//! Rotation around any quant: the quant sees R·x per block, the weights come back as Rᵀ·q.
//! R is orthogonal and block diagonal, so the total squared error is the same in both views.

use crate::quants::{get_rotation_block_size, Settings};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// Plain Walsh-Hadamard, H/√p.
    Hadamard,
    /// Random ±1 signs, then Walsh-Hadamard (QuIP#, QuaRot).
    RandomHadamard,
    /// Q of the QR decomposition of a Gaussian matrix (TurboQuant).
    Orthogonal,
}

/// Random orthogonal blocks are d³ to build and d² per vector, larger ones get clamped.
pub const MAX_ORTHOGONAL: usize = 1024;

pub struct Rotation {
    pub kind: Kind,
    pub size: usize,
    pub seed: u32,
    /// Sign of every position in a block, RandomHadamard only.
    signs: Vec<f32>,
}

impl Rotation {
    pub fn from_settings(settings: &Settings) -> Option<Rotation> {
        let kind = match settings.rotation_type.as_str() {
            "hadamard" => Kind::Hadamard,
            "rht" => Kind::RandomHadamard,
            "orthogonal" => Kind::Orthogonal,
            _ => return None,
        };
        let mut size = match settings.rotation_size {
            0 => get_rotation_block_size(&settings.q_type, settings),
            s => s,
        }
        .max(2);
        if kind == Kind::Orthogonal {
            size = size.min(MAX_ORTHOGONAL);
        }
        let signs = if kind == Kind::RandomHadamard {
            let mut rng = Rng(settings.rotation_seed as u64);
            (0..size)
                .map(|_| if rng.next() >> 63 == 0 { 1.0 } else { -1.0 })
                .collect()
        } else {
            Vec::new()
        };
        Some(Rotation {
            kind,
            size,
            seed: settings.rotation_seed,
            signs,
        })
    }

    pub fn short_name(&self) -> &'static str {
        match self.kind {
            Kind::Hadamard => "Hadamard",
            Kind::RandomHadamard => "RHT",
            Kind::Orthogonal => "Orthogonal",
        }
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            Kind::Hadamard => "Hadamard",
            Kind::RandomHadamard => "Randomized Hadamard",
            Kind::Orthogonal => "Random orthogonal",
        }
    }

    /// Pieces that get their own transform: (start, len, offset in the block).
    /// Hadamard needs powers of two, so a shorter last block is split into them (96 -> 64 + 32).
    fn pieces(&self, n: usize) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        for b in (0..n).step_by(self.size) {
            let len = (n - b).min(self.size);
            if self.kind == Kind::Orthogonal {
                out.push((b, len, 0));
                continue;
            }
            let mut off = 0;
            while off < len {
                let p = 1usize << (usize::BITS - 1 - (len - off).leading_zeros());
                out.push((b + off, p, off));
                off += p;
            }
        }
        out
    }

    /// Number of coefficients that make up weight i.
    pub fn piece_len(&self, i: usize, n: usize) -> usize {
        self.pieces(n)
            .into_iter()
            .find(|&(s, len, _)| i >= s && i < s + len)
            .map_or(0, |(_, len, _)| len)
    }

    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        let mut y = x.to_vec();
        for (s, len, off) in self.pieces(x.len()) {
            let v = &mut y[s..s + len];
            if self.kind == Kind::Orthogonal {
                let q = orthogonal(len, self.seed);
                let src = v.to_vec();
                for k in 0..len {
                    let row = &q[k * len..(k + 1) * len];
                    v[k] = row
                        .iter()
                        .zip(&src)
                        .map(|(&a, &b)| a as f64 * b as f64)
                        .sum::<f64>() as f32;
                }
                continue;
            }
            if self.kind == Kind::RandomHadamard {
                for j in 0..len {
                    v[j] *= self.signs[off + j];
                }
            }
            fwht(v);
        }
        y
    }

    pub fn inverse(&self, y: &[f32]) -> Vec<f32> {
        let mut x = y.to_vec();
        for (s, len, off) in self.pieces(y.len()) {
            let v = &mut x[s..s + len];
            if self.kind == Kind::Orthogonal {
                let q = orthogonal(len, self.seed);
                let mut acc = vec![0.0f64; len];
                for k in 0..len {
                    let c = v[k] as f64;
                    for (a, &r) in acc.iter_mut().zip(&q[k * len..(k + 1) * len]) {
                        *a += c * r as f64;
                    }
                }
                for (v, a) in v.iter_mut().zip(acc) {
                    *v = a as f32;
                }
                continue;
            }
            fwht(v);
            if self.kind == Kind::RandomHadamard {
                for j in 0..len {
                    v[j] *= self.signs[off + j];
                }
            }
        }
        x
    }

    /// Importance of the rotated coefficients, w'_k = Σ_j R_kj²·w_j. That's the expected
    /// weighted error if the errors of the coefficients are uncorrelated. Hadamard has
    /// R_kj² = 1/p, so every coefficient gets the mean of its piece.
    pub fn importance(&self, w: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0f32; w.len()];
        for (s, len, _) in self.pieces(w.len()) {
            let src = &w[s..s + len];
            if self.kind == Kind::Orthogonal {
                let q = orthogonal(len, self.seed);
                for k in 0..len {
                    let row = &q[k * len..(k + 1) * len];
                    out[s + k] = row
                        .iter()
                        .zip(src)
                        .map(|(&r, &x)| r as f64 * r as f64 * x as f64)
                        .sum::<f64>() as f32;
                }
                continue;
            }
            let mean = src.iter().map(|&x| x as f64).sum::<f64>() / len as f64;
            out[s..s + len].fill(mean as f32);
        }
        out
    }
}

/// In-place orthonormal Walsh-Hadamard transform, len must be a power of two. Self inverse.
fn fwht(v: &mut [f32]) {
    let n = v.len();
    let mut a: Vec<f64> = v.iter().map(|&x| x as f64).collect();
    let mut h = 1;
    while h < n {
        for i in (0..n).step_by(2 * h) {
            for j in i..i + h {
                let (x, y) = (a[j], a[j + h]);
                a[j] = x + y;
                a[j + h] = x - y;
            }
        }
        h *= 2;
    }
    let scale = 1.0 / (n as f64).sqrt();
    for (v, a) in v.iter_mut().zip(a) {
        *v = (a * scale) as f32;
    }
}

thread_local! {
    /// The full block size and the size of the last block are what's needed at a time.
    static ORTHO: RefCell<Vec<(usize, u32, Rc<Vec<f32>>)>> = RefCell::new(Vec::new());
}

/// Seeded random orthogonal d×d matrix, row-major.
pub fn orthogonal(d: usize, seed: u32) -> Rc<Vec<f32>> {
    ORTHO.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((_, _, m)) = c.iter().find(|(cd, cs, _)| *cd == d && *cs == seed) {
            return m.clone();
        }
        let m = Rc::new(random_rotation(d, &mut Rng(seed as u64)));
        if c.len() >= 2 {
            c.remove(0);
        }
        c.push((d, seed, m.clone()));
        m
    })
}

/// Q of the QR decomposition of a Gaussian matrix. Modified Gram-Schmidt keeps diag(R) positive,
/// which makes Q uniformly distributed over rotations.
fn random_rotation(d: usize, rng: &mut Rng) -> Vec<f32> {
    let mut q = vec![0.0f64; d * d];
    rng.gauss_fill(&mut q);
    for k in 0..d {
        let (done, rest) = q.split_at_mut(k * d);
        let v = &mut rest[..d];
        for i in 0..k {
            let qi = &done[i * d..(i + 1) * d];
            let dot: f64 = qi.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
            for (x, &a) in v.iter_mut().zip(qi) {
                *x -= dot * a;
            }
        }
        let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
    q.iter().map(|&x| x as f32).collect()
}

/// SplitMix64, only needs to be reproducible.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    /// Uniform in (0, 1].
    fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) + 1) as f64 / (1u64 << 53) as f64
    }

    /// Box-Muller, two normals per pair of uniforms.
    pub fn gauss_fill(&mut self, out: &mut [f64]) {
        for pair in out.chunks_mut(2) {
            let r = (-2.0 * self.uniform().ln()).sqrt();
            let t = 2.0 * std::f64::consts::PI * self.uniform();
            pair[0] = r * t.cos();
            if pair.len() > 1 {
                pair[1] = r * t.sin();
            }
        }
    }
}
