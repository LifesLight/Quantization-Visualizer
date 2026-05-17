use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn wasm_fwht(data: &[f64]) -> Vec<f64> {
    let n = data.len();
    let mut p2 = 1;
    while p2 < n {
        p2 *= 2;
    }

    let mut res = vec![0.0; p2];
    res[..n].copy_from_slice(data);

    let mut h = 1;
    while h < p2 {
        let mut i = 0;
        while i < p2 {
            for j in i..i + h {
                let x = res[j];
                let y = res[j + h];
                res[j] = x + y;
                res[j + h] = x - y;
            }
            i += h * 2;
        }
        h *= 2;
    }

    let scale = 1.0 / (p2 as f64).sqrt();
    for x in &mut res {
        *x *= scale;
    }

    res.truncate(n);
    res
}

#[wasm_bindgen]
pub fn wasm_get_lloyd_max_centroids(bits: u32, dist: &str) -> Vec<f64> {
    let levels = 1_usize << bits;
    let mut centroids = vec![0.0; levels];
    for i in 0..levels {
        centroids[i] = -3.0 + (6.0 * (i as f64 + 0.5)) / (levels as f64);
    }

    let is_laplace = dist == "laplace";
    let pdf = |x: f64| -> f64 {
        if is_laplace {
            0.5 * (-x.abs()).exp()
        } else {
            (-x * x / 2.0).exp()
        }
    };

    let steps = 200;
    for _ in 0..100 {
        let mut thresholds = vec![0.0; levels + 1];
        thresholds[0] = -10.0;
        for i in 0..levels - 1 {
            thresholds[i + 1] = (centroids[i] + centroids[i + 1]) / 2.0;
        }
        thresholds[levels] = 10.0;

        for i in 0..levels {
            let mut num = 0.0;
            let mut den = 0.0;
            let t_start = thresholds[i];
            let t_end = thresholds[i + 1];
            let dt = (t_end - t_start) / (steps as f64);

            for j in 0..steps {
                let x = t_start + (j as f64 + 0.5) * dt;
                let p = pdf(x);
                num += x * p;
                den += p;
            }
            if den > 1e-9 {
                centroids[i] = num / den;
            }
        }
    }
    centroids
}
