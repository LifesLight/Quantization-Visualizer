//! Pieces shared by the KT row quantizers (ik_llama.cpp, iqk_quantize.cpp).

use super::codebook::{lane_state, state_value};

pub use crate::quants::iq::algos::nearest_int;

pub const QK_K: usize = 256;

/// First half of iq4k_values, the non-linear 4 bit scale table of IQ1_KT and IQ2_KT.
pub const IQ4K_VALUES: [i8; 16] = [
    -127, -104, -83, -65, -49, -35, -22, -10, 1, 13, 25, 38, 53, 69, 89, 113,
];

#[rustfmt::skip]
static IQ4NL_INDEX: [i8; 241] = [
     0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, 16, 16,  1,  1,  1,  1,  1,  1,  1,  1,  1,  1,  1,
     1,  1,  1,  1,  1,  1,  1,  1,  1, 17, 17,  2,  2,  2,  2,  2,  2,  2,  2,  2,  2,  2,  2,  2,
     2,  2,  2,  2,  2, 18,  3,  3,  3,  3,  3,  3,  3,  3,  3,  3,  3,  3,  3,  3,  3,  3, 19,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4, 20,  5,  5,  5,  5,  5,  5,  5,  5,  5,  5,
     5,  5, 21, 21,  6,  6,  6,  6,  6,  6,  6,  6,  6,  6,  6, 22,  7,  7,  7,  7,  7,  7,  7,  7,
     7,  7, 23, 23,  8,  8,  8,  8,  8,  8,  8,  8,  8,  8, 24,  9,  9,  9,  9,  9,  9,  9,  9,  9,
     9,  9, 25, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 26, 26, 11, 11, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 27, 27, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 28, 13, 13, 13,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 29, 14, 14, 14, 14, 14, 14, 14, 14, 14,
    14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 30, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15,];

/// One quantized row: the f32 row scale as stored, then the super-blocks.
pub struct KtRow {
    pub d: f32,
    pub blocks: Vec<u8>,
}

/// Everything the inspector needs to explain how one weight was decoded.
pub struct KtDecode {
    /// Row scale as used at inference.
    pub d: f32,
    pub d_label: &'static str,
    /// Stored block scale field and the multiplier it decodes to.
    pub field: i32,
    pub dl: f32,
    pub dl_expr: &'static str,
    pub idx: u32,
    pub index_bits: u32,
    pub offset: u32,
    pub lane: usize,
    pub value: f32,
    /// IQ3_KT only: the sign bits of the 8 weights of the group.
    pub neg: Option<u8>,
    pub math: String,
}

/// Port of best_index_iq4nl (iqk version, table lookup with a midpoint check).
pub fn best_index_iq4nl(x: f32) -> usize {
    let ix = x as i32 - IQ4K_VALUES[0] as i32;
    if ix < 0 || ix >= 241 {
        return if ix < 0 { 0 } else { 15 };
    }
    let ix = IQ4NL_INDEX[ix as usize] as usize;
    if ix < 16 {
        ix
    } else if x - (IQ4K_VALUES[ix - 16] as f32) < (IQ4K_VALUES[ix - 15] as f32) - x {
        ix - 16
    } else {
        ix - 15
    }
}

/// Port of QuantizerIQKT::set_weights. Only the first n_per_row weights are real,
/// the rest of the last super-block gets weight 0.
pub fn set_weights(
    sigma2_scale: f32,
    x: &[f32],
    n_per_row: usize,
    imatrix: Option<&[f32]>,
    w: &mut [f32],
) {
    const EPS2: f32 = 1e-14;
    const WEIGHT: f32 = 1e-4;
    let nblock = x.len() / QK_K;
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..ibl * QK_K + QK_K];
        let wbl = &mut w[ibl * QK_K..ibl * QK_K + QK_K];
        let n = QK_K.min(n_per_row.saturating_sub(ibl * QK_K));
        wbl[n..].fill(0.0);
        let mut sumx2 = 0.0f32;
        for j in 0..n {
            sumx2 += xbl[j] * xbl[j];
        }
        if sumx2 < EPS2 * n as f32 {
            wbl[..n].fill(WEIGHT);
            continue;
        }
        let sigma2 = sigma2_scale * sumx2 / n as f32;
        match imatrix {
            Some(im) => {
                for ib in 0..n / 32 {
                    let qw = &im[ibl * QK_K + ib * 32..];
                    let xb = &xbl[ib * 32..ib * 32 + 32];
                    let wb = &mut wbl[ib * 32..ib * 32 + 32];
                    let mut sumwx = 0.0f32;
                    let mut sumw2 = 0.0f32;
                    let mut sumx2 = 0.0f32;
                    for j in 0..32 {
                        wb[j] = qw[j] * (sigma2 + xb[j] * xb[j]).sqrt();
                        sumwx += wb[j] * xb[j].abs();
                        sumw2 += wb[j] * wb[j];
                        sumx2 += xb[j] * xb[j];
                    }
                    if sumx2 < EPS2 || sumw2 < EPS2 || sumwx < EPS2 {
                        wb.fill(WEIGHT);
                    }
                }
            }
            None => {
                for j in 0..n {
                    wbl[j] = 0.25 * sigma2 + xbl[j] * xbl[j];
                }
            }
        }
    }
}

/// Generated values of one group straight from the LCG, no codebook needed.
pub fn gen_group(idx: u32, offset: u32, out: &mut [f32]) {
    for (lane, v) in out.iter_mut().enumerate() {
        *v = state_value(lane_state(idx, offset, lane));
    }
}

pub fn max_abs(x: &[f32]) -> f32 {
    let mut m = 0.0f32;
    for &v in x {
        let a = v.abs();
        if m < a {
            m = a;
        }
    }
    m
}

pub fn get_u16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

pub fn put_u16(b: &mut [u8], off: usize, v: u16) {
    b[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

pub fn get_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

pub fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
