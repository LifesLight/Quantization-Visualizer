//! IQ4_KS: f32 row scale, then block_iq4_ks { u8 scales[8]; u8 qs[128] } -> 136 bytes per 256.
//! IQ4_K levels per 32 weights. Each scale byte holds the shift bit (bit 0) and an even 7 bit
//! value, the block scale is (byte & 254) - 127.

use super::common::*;
use super::iq4_k::{simple, table};
use super::lut::IQ4K_VALUES;
use super::{formula, KDecode, KType};

pub static TYPE: KType = KType {
    name: "IQ4_KS",
    block_bytes: BYTES,
    row_meta: 4,
    sub_size: 32,
    imatrix_block: 32,
    quantize,
    dequantize,
    decode,
    block_scales,
    formula_html,
};

const BYTES: usize = 136;
const QS: usize = 8;

/// Port of quantize_row_iq4_k_impl_bs32 (AVX2 path).
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const BS: usize = 32;
    const NTRY: i32 = 7;
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; 4 + nblock * BYTES];
    let mut all_scales = vec![0.0f32; nblock * 8];
    let mut weight = [0.0f32; BS];
    let sigma2_of = |xbl: &[f32]| sum_sq(xbl) * (2.0 / QK_K as f32);
    let set_weights = |weight: &mut [f32; BS], ibl: usize, ib: usize, sigma2: f32| {
        let xb = &x[ibl * QK_K + ib * BS..ibl * QK_K + ib * BS + BS];
        for j in 0..BS {
            weight[j] = match qw {
                Some(qw) => qw[ibl * QK_K + ib * BS + j] * (sigma2 + xb[j] * xb[j]).sqrt(),
                None => xb[j] * xb[j],
            };
        }
    };

    let mut amax_scale = 0.0f32;
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = sigma2_of(xbl);
        for ib in 0..8 {
            set_weights(&mut weight, ibl, ib, sigma2);
            let (d, shifted) = simple().search(16, &xbl[ib * BS..ib * BS + BS], &weight, NTRY, 1.0);
            if shifted {
                out[4 + ibl * BYTES + ib] = 0x01;
            }
            all_scales[ibl * 8 + ib] = d;
            amax_scale = amax_scale.max(d.abs());
        }
    }
    let d = amax_scale / 127.0;
    out[..4].copy_from_slice(&(1.0 * d).to_le_bytes());
    if d == 0.0 {
        return out;
    }
    let id = 1.0 / d;
    let mut sumqx = 0.0f32;
    let mut sumq2 = 0.0f32;
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = sigma2_of(xbl);
        let y = &mut out[4 + ibl * BYTES..4 + (ibl + 1) * BYTES];
        for ib in 0..8 {
            let bv = table(y[ib] & 1 != 0);
            let mut l = nearest_int(0.5 * (id * all_scales[ibl * 8 + ib] + 127.0));
            l = l.clamp(0, 127) << 1;
            y[ib] |= l as u8;
            l -= 127;
            let dl = d * l as f32;
            let idl = if dl != 0.0 { 1.0 / dl } else { 0.0 };
            let xb = &xbl[ib * BS..ib * BS + BS];
            set_weights(&mut weight, ibl, ib, sigma2);
            for j in 0..BS / 2 {
                let i1 = best_index_iq4nl(bv, idl * xb[j]);
                let i2 = best_index_iq4nl(bv, idl * xb[j + BS / 2]);
                y[QS + ib * (BS / 2) + j] = (i1 | (i2 << 4)) as u8;
                let (w1, w2) = (weight[j], weight[j + BS / 2]);
                let q1 = (bv[i1] as i32 * l) as f32;
                let q2 = (bv[i2] as i32 * l) as f32;
                sumqx += w1 * q1 * xb[j] + w2 * q2 * xb[j + BS / 2];
                sumq2 += w1 * q1 * q1 + w2 * q2 * q2;
            }
        }
    }
    if sumq2 > 0.0 {
        out[..4].copy_from_slice(&(1.0 * sumqx / sumq2).to_le_bytes());
    }
    out
}

pub fn row_scale(b: &[u8]) -> f32 {
    f32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Port of dequantize_row_iq4_ks.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = row_scale(b);
    for (ibl, blk) in b[4..].chunks(BYTES).enumerate() {
        for ib in 0..8 {
            let dl = d * ((blk[ib] & 254) as i32 - 127) as f32;
            let v = table(blk[ib] & 1 != 0);
            let o = ibl * QK_K + 32 * ib;
            for j in 0..16 {
                let q = blk[QS + 16 * ib + j];
                y[o + j] = dl * v[(q & 0xf) as usize] as f32;
                y[o + j + 16] = dl * v[(q >> 4) as usize] as f32;
            }
        }
    }
}

pub fn block_scales(b: &[u8], ibl: usize) -> Vec<(f32, bool)> {
    let d = row_scale(b);
    let blk = &b[4 + ibl * BYTES..4 + (ibl + 1) * BYTES];
    (0..8)
        .map(|ib| (d * ((blk[ib] & 254) as i32 - 127) as f32, blk[ib] & 1 != 0))
        .collect()
}

pub fn decode(b: &[u8], i: usize) -> KDecode {
    let d = row_scale(b);
    let blk = &b[4 + (i / QK_K) * BYTES..4 + (i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let ib = j / 32;
    let shifted = blk[ib] & 1 != 0;
    let q = blk[QS + 16 * ib + j % 16];
    let idx = if j % 32 < 16 { q & 0xf } else { q >> 4 } as usize;
    let value = table(shifted)[idx] as f32;
    let f = (blk[ib] & 254) as i32;
    KDecode::new(
        d,
        true,
        f,
        7,
        d * (f - 127) as f32,
        "({s}&minus;127)",
        shifted,
    )
    .level(idx, 4, value, &IQ4K_VALUES, 16)
    .math(format!(
        "{:.6}&middot;({}&minus;127) &times; {}",
        d, f, value
    ))
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">32b</span></span> &times; (<span class=\"eq-pill\">s<span class=\"bits\">7b</span></span> &minus; 127) &times; <span class=\"eq-pill\">iq4k[sh][q]<span class=\"bits\">1b+4b</span></span>",
        "IQ4_K levels with shifted copy and 7-bit scale per 32, one FP32 scale per row.",
        "Same 16 levels and shifted copy as IQ4_K, per 32 weights. Each block has one byte: the shift bit and an even 7-bit value, so the block scale is an odd number from -127 to 127. No per-256 scale, the whole row shares one FP32 scale. 4.25 bpw plus 32 bits per row.",
    )
}
