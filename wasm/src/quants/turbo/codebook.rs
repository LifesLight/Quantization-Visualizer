//! Lloyd-Max levels for one coordinate of a random point on the unit sphere in d dimensions,
//! f(x) ∝ (1 − x²)^((d−3)/2) on [−1, 1] (Lemma 1 of the paper). Close to N(0, 1/d) for large d.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Codebook {
    pub levels: Vec<f32>,
    /// Midpoints between neighbouring levels.
    pub bounds: Vec<f32>,
    /// d·E[(x − c)²], the expected ‖x − x̃‖² of a unit vector.
    pub distortion: f64,
}

impl Codebook {
    pub fn index(&self, y: f32) -> usize {
        self.bounds.partition_point(|&b| b < y)
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<(usize, u32), Rc<Codebook>>> = RefCell::new(HashMap::new());
}

pub fn get(d: usize, bits: u32) -> Rc<Codebook> {
    CACHE.with(|c| {
        c.borrow_mut()
            .entry((d, bits))
            .or_insert_with(|| Rc::new(lloyd_max(d, bits)))
            .clone()
    })
}

const BINS: usize = 1 << 16;

/// Continuous 1-D k-means on f. The density is integrated on a fine grid (constant per bin,
/// exact inside a bin), so levels and bounds move smoothly between iterations.
fn lloyd_max(d: usize, bits: u32) -> Codebook {
    let n = 1usize << bits;
    let sigma = 1.0 / (d as f64).sqrt();
    // past 16 sigma the density is below e^-120
    let half = (16.0 * sigma).min(1.0);
    let h = 2.0 * half / BINS as f64;
    let e = (d as f64 - 3.0).max(0.0) / 2.0;

    let mut dens = vec![0.0f64; BINS];
    let mut m = [
        vec![0.0f64; BINS + 1],
        vec![0.0f64; BINS + 1],
        vec![0.0f64; BINS + 1],
    ];
    for i in 0..BINS {
        let a = -half + i as f64 * h;
        let b = a + h;
        let x = a + h / 2.0;
        let p = (e * (1.0 - x * x).ln()).exp();
        dens[i] = p;
        m[0][i + 1] = m[0][i] + p * (b - a);
        m[1][i + 1] = m[1][i] + p * (b * b - a * a) / 2.0;
        m[2][i + 1] = m[2][i] + p * (b * b * b - a * a * a) / 3.0;
    }
    // ∫ x^k f from -half to t, k = 0, 1, 2
    let cum = |t: f64| -> [f64; 3] {
        let t = t.clamp(-half, half);
        let k = (((t + half) / h) as usize).min(BINS - 1);
        let a = -half + k as f64 * h;
        let p = dens[k];
        [
            m[0][k] + p * (t - a),
            m[1][k] + p * (t * t - a * a) / 2.0,
            m[2][k] + p * (t * t * t - a * a * a) / 3.0,
        ]
    };

    let mut c: Vec<f64> = (0..n)
        .map(|i| (-3.0 + 6.0 * (i as f64 + 0.5) / n as f64) * sigma)
        .collect();
    if n == 1 {
        c[0] = 0.0;
    }
    let mut bounds = vec![0.0f64; n - 1];
    for _ in 0..if n > 1 { 20000 } else { 0 } {
        for i in 0..n - 1 {
            bounds[i] = (c[i] + c[i + 1]) / 2.0;
        }
        let mut moved = 0.0f64;
        let mut lo = cum(-half);
        for i in 0..n {
            let hi = cum(if i + 1 < n { bounds[i] } else { half });
            let mass = hi[0] - lo[0];
            if mass > 0.0 {
                let nc = (hi[1] - lo[1]) / mass;
                moved = moved.max((nc - c[i]).abs());
                c[i] = nc;
            }
            lo = hi;
        }
        if moved < 1e-13 * sigma {
            break;
        }
    }
    for i in 0..n - 1 {
        bounds[i] = (c[i] + c[i + 1]) / 2.0;
    }

    let mut err = 0.0f64;
    let mut lo = cum(-half);
    for i in 0..n {
        let hi = cum(if i + 1 < n { bounds[i] } else { half });
        err += (hi[2] - lo[2]) - 2.0 * c[i] * (hi[1] - lo[1]) + c[i] * c[i] * (hi[0] - lo[0]);
        lo = hi;
    }

    Codebook {
        levels: c.iter().map(|&x| x as f32).collect(),
        bounds: bounds.iter().map(|&x| x as f32).collect(),
        distortion: d as f64 * err / m[0][BINS],
    }
}
