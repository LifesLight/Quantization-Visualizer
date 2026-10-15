//! Shared pieces of the IQ*_K quantizers: table lookups, ik's make_qx_quants, the imatrix
//! clean-up of QHelper and the AVX2 scale search that x86 builds of iqk_quantize.cpp use.

use super::lut;

pub use crate::quants::iq::algos::{
    f16_bits, f16_val, get_u16, get_u32, nearest_int, put_u16, put_u32, sort_pairs,
};

pub const QK_K: usize = 256;

/// ggml's hsum_float_8.
pub fn hsum8(v: &[f32; 8]) -> f32 {
    ((v[0] + v[4]) + (v[2] + v[6])) + ((v[1] + v[5]) + (v[3] + v[7]))
}

/// (int)x on x86: truncation, out of range and NaN give i32::MIN.
pub fn cvtt(v: f32) -> i32 {
    if v >= -2147483648.0 && v < 2147483648.0 {
        v as i32
    } else {
        i32::MIN
    }
}

pub fn best_index_iq2nl(values: &[i8], x: f32) -> usize {
    let idx = if x < values[1] as f32 {
        0
    } else if x > values[2] as f32 {
        2
    } else {
        1
    };
    if x - (values[idx] as f32) < (values[idx + 1] as f32) - x {
        idx
    } else {
        idx + 1
    }
}

/// best_index_iq3nl / iq4nl / iq5nl, `n` levels, index table of `len` entries.
fn best_index_nl(values: &[i8], x: f32, index: &[i8], len: i32, n: i32) -> usize {
    let ix = cvtt(x).wrapping_sub(values[0] as i32);
    if ix < 0 || ix >= len {
        return if ix < 0 { 0 } else { (n - 1) as usize };
    }
    let ix = index[ix as usize] as i32;
    if ix < n {
        return ix as usize;
    }
    let (lo, hi) = ((ix - n) as usize, (ix - n + 1) as usize);
    if x - (values[lo] as f32) < (values[hi] as f32) - x {
        lo
    } else {
        hi
    }
}

pub fn best_index_iq3nl(values: &[i8], x: f32) -> usize {
    best_index_nl(values, x, &lut::IQ3NL_INDEX, 111, 8)
}

pub fn best_index_iq4nl(values: &[i8], x: f32) -> usize {
    best_index_nl(values, x, &lut::IQ4NL_INDEX, 241, 16)
}

pub fn best_index_iq5nl(values: &[i8], x: f32) -> usize {
    best_index_nl(values, x, &lut::IQ5NL_INDEX, 247, 32)
}

/// IQ6 works on float values, the difference to values[0] is taken before truncating.
pub fn best_index_iq6nl(values: &[f32], x: f32) -> usize {
    let ix = cvtt(x - values[0]);
    if ix < 0 || ix >= 249 {
        return if ix < 0 { 0 } else { 63 };
    }
    let ix = lut::IQ6NL_INDEX[ix as usize] as usize;
    if ix < 64 {
        return ix;
    }
    if x - values[ix - 64] < values[ix - 63] - x {
        ix - 64
    } else {
        ix - 63
    }
}

/// make_qx_quants from iqk_quantize.cpp (not the ggml-quants.c one), always weighted.
pub fn make_qx_quants(nmax: i32, x: &[f32], l: &mut [i8], qw: &[f32]) -> f32 {
    let n = x.len();
    let mut max = 0.0f32;
    let mut amax = 0.0f32;
    for &v in x {
        let ax = v.abs();
        if ax > amax {
            amax = ax;
            max = v;
        }
    }
    if amax == 0.0 {
        l.fill(0);
        return 0.0;
    }
    let quant = |iscale: f32, i: usize| nearest_int(iscale * x[i]).clamp(-nmax, nmax - 1);
    let mut iscale = -(nmax as f32) / max;
    let mut sumlx = 0.0f32;
    let mut suml2 = 0.0f32;
    for i in 0..n {
        let li = quant(iscale, i);
        l[i] = (li + nmax) as i8;
        sumlx += qw[i] * x[i] * li as f32;
        suml2 += qw[i] * li as f32 * li as f32;
    }
    let mut scale = if suml2 != 0.0 { sumlx / suml2 } else { 0.0 };
    let mut best = scale * sumlx;
    for is in -9..=9 {
        if is == 0 {
            continue;
        }
        iscale = -(nmax as f32 + 0.1 * is as f32) / max;
        sumlx = 0.0;
        suml2 = 0.0;
        for i in 0..n {
            let li = quant(iscale, i);
            sumlx += qw[i] * x[i] * li as f32;
            suml2 += qw[i] * li as f32 * li as f32;
        }
        if suml2 > 0.0 && sumlx * sumlx > best * suml2 {
            for i in 0..n {
                l[i] = (nmax + quant(iscale, i)) as i8;
            }
            scale = sumlx / suml2;
            best = scale * sumlx;
        }
    }
    scale
}

/// QHelper::row_weights: the imatrix of a block whose weights, values or overlap are all tiny
/// is replaced by 1e-9.
pub fn row_weights(imatrix: &[f32], x: &[f32], block_size: usize) -> Vec<f32> {
    const EPS: f32 = 1e-9;
    let eps2 = EPS * EPS;
    let mut w = imatrix.to_vec();
    for (wb, xb) in w.chunks_mut(block_size).zip(x.chunks(block_size)) {
        let (mut sumw2, mut sumx2, mut sumwx) = (0.0f32, 0.0f32, 0.0f32);
        for (&wj, &xj) in wb.iter().zip(xb) {
            sumw2 += wj * wj;
            sumx2 += xj * xj;
            sumwx += wj * xj.abs();
        }
        let lim = block_size as f32 * eps2;
        if !(sumw2 > lim && sumx2 > lim && sumwx > lim) {
            wb.fill(EPS);
        }
    }
    w
}

pub fn max_abs_signed(xb: &[f32]) -> (f32, f32) {
    let (mut amax, mut max) = (0.0f32, 0.0f32);
    for &v in xb {
        let ax = v.abs();
        if ax > amax {
            amax = ax;
            max = v;
        }
    }
    (amax, max)
}

pub fn sum_sq(x: &[f32]) -> f32 {
    let mut s = 0.0f32;
    for &v in x {
        s += v * v;
    }
    s
}

/// The reference reads one past its IQ6 level table there (f_values[128], undefined). In the
/// gcc -O2 and -O3 AVX2 builds that stack slot holds the bits 0xfe00. Only nearly empty blocks
/// care, there -127 or 0.0 give different bytes.
pub const OOB_IQ6: f32 = f32::from_bits(0xfe00);

/// The tables the AVX2 search needs: levels as floats and the coarse index table.
pub struct Simple {
    pub values: &'static [i8],
    pub f_values: Vec<f32>,
    pub k_simple: &'static [i8],
    pub nmap: i32,
}

impl Simple {
    pub fn new(values: &'static [i8], k_simple: &'static [i8], nmap: i32) -> Simple {
        Simple {
            values,
            f_values: values.iter().map(|&v| v as f32).collect(),
            k_simple,
            nmap,
        }
    }

    /// One lane of best_index_avx2: coarse index from the truncated value, then the closer of
    /// two neighbours. `off` selects the shifted half of the table.
    fn level(&self, off: usize, v: f32) -> f32 {
        let idx = cvtt(v)
            .wrapping_sub(self.values[off] as i32)
            .max(0)
            .min(self.nmap);
        let t0 = self.k_simple[idx as usize] as u8 as usize;
        let c0 = self.f_values[off + t0];
        // IQ6: the top of the coarse table points one past the level table. For the shifted
        // half that read is past the end of the reference's array (undefined), see OOB_IQ6.
        let c1 = self.f_values.get(off + t0 + 1).copied().unwrap_or(OOB_IQ6);
        if v - c0 < c1 - v {
            c0
        } else {
            c1
        }
    }

    /// evaluate_block_avx2 / evaluate_block_iq4k: best scale for d and -d, lane-wise FMA sums.
    pub fn evaluate(
        &self,
        off: usize,
        id: f32,
        vx: &[f32],
        vw: &[f32],
        d: &mut f32,
        best: &mut f32,
    ) -> bool {
        let mut sqx_p = [0.0f32; 8];
        let mut sq2_p = [0.0f32; 8];
        let mut sqx_m = [0.0f32; 8];
        let mut sq2_m = [0.0f32; 8];
        for (j, (&x, &w)) in vx.iter().zip(vw).enumerate() {
            let l = j % 8;
            let v = id * x;
            let qp = self.level(off, v);
            let qm = self.level(off, -v);
            let wp = w * qp;
            let wm = w * qm;
            sqx_p[l] = wp.mul_add(x, sqx_p[l]);
            sq2_p[l] = wp.mul_add(qp, sq2_p[l]);
            sqx_m[l] = wm.mul_add(x, sqx_m[l]);
            sq2_m[l] = wm.mul_add(qm, sq2_m[l]);
        }
        let (sqx_p, sq2_p) = (hsum8(&sqx_p), hsum8(&sq2_p));
        let (sqx_m, sq2_m) = (hsum8(&sqx_m), hsum8(&sq2_m));
        let mut updated = false;
        if sq2_p > 0.0 && sqx_p * sqx_p > sq2_p * *best {
            *d = sqx_p / sq2_p;
            *best = *d * sqx_p;
            updated = true;
        }
        if sq2_m > 0.0 && sqx_m * sqx_m > sq2_m * *best {
            *d = sqx_m / sq2_m;
            *best = *d * sqx_m;
            updated = true;
        }
        updated
    }

    /// find_best_scale_avx2: (d, shifted) of one block, nval levels per table half.
    #[allow(clippy::too_many_arguments)]
    pub fn find_best_scale(
        &self,
        nval: usize,
        xb: &[f32],
        imatrix: Option<&[f32]>,
        sigma2: f32,
        ntry: i32,
        use_sigma2: bool,
        step: f32,
    ) -> (f32, bool) {
        let vw: Vec<f32> = match imatrix {
            Some(qw) => xb
                .iter()
                .zip(qw)
                .map(|(&x, &w)| w * x.mul_add(x, sigma2).sqrt())
                .collect(),
            None if use_sigma2 => {
                let s = 0.25 * sigma2;
                xb.iter().map(|&x| x.mul_add(x, s)).collect()
            }
            None => xb.iter().map(|&x| x * x).collect(),
        };
        self.search(nval, xb, &vw, ntry, step)
    }

    /// The scale search itself, for callers that bring their own weights (IQ4_K, IQ4_KS).
    pub fn search(&self, nval: usize, xb: &[f32], vw: &[f32], ntry: i32, step: f32) -> (f32, bool) {
        let (amax, max) = max_abs_signed(xb);
        if amax < 1e-16 {
            return (0.0, false);
        }
        let v0 = self.values[0] as f32;
        let mut d = if ntry > 0 { -max / v0 } else { max / v0 };
        let mut best = 0.0f32;
        self.evaluate(0, 1.0 / d, xb, vw, &mut d, &mut best);
        let mut shifted = false;
        for itry in -ntry..=ntry {
            let id = (step * itry as f32 + v0) / max;
            if self.evaluate(0, id, xb, vw, &mut d, &mut best) {
                shifted = false;
            }
            let id = (step * itry as f32 + self.values[nval] as f32) / max;
            if self.evaluate(nval, id, xb, vw, &mut d, &mut best) {
                shifted = true;
            }
        }
        (d, shifted)
    }
}
