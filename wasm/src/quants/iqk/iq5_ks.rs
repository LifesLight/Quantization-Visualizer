//! IQ5_KS: f32 row scale, then block_iq5_ks { u8 scales[8]; u8 qs[128]; u8 qh[32] } -> 168 bytes.
//! IQ5_K levels per 32 weights, scale bytes like IQ4_KS (shift bit + even 7 bit value).

use super::common::*;
use super::iq4_ks::row_scale;
use super::iq5_k::{simple, table};
use super::lut::IQ5NL_VALUES;
use super::{formula, KDecode, KType};

pub static TYPE: KType = KType {
    name: "IQ5_KS",
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

const BYTES: usize = 168;
const QS: usize = 8;
const QH: usize = 136;

/// Port of quantize_row_iq5_ks_impl (AVX2 path).
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const BS: usize = 32;
    const NTRY: i32 = 5;
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; 4 + nblock * BYTES];
    let mut all_scales = vec![0.0f32; nblock * 8];
    let mut weight = [0.0f32; BS];
    let sigma2_of = |xbl: &[f32]| sum_sq(xbl) * (2.0 / QK_K as f32);

    let mut amax_scale = 0.0f32;
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = sigma2_of(xbl);
        for ib in 0..8 {
            let w = qw.map(|qw| &qw[QK_K * ibl + BS * ib..QK_K * ibl + BS * ib + BS]);
            let (d, shifted) = simple().find_best_scale(
                32,
                &xbl[BS * ib..BS * ib + BS],
                w,
                sigma2,
                NTRY,
                false,
                1.0,
            );
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
            for j in 0..BS {
                weight[j] = match qw {
                    Some(qw) => qw[ibl * QK_K + ib * BS + j] * (sigma2 + xb[j] * xb[j]).sqrt(),
                    None => xb[j] * xb[j],
                };
            }
            for j in 0..BS {
                let idx = best_index_iq5nl(bv, idl * xb[j]);
                y[QS + BS * (ib / 2) + j] |= ((idx & 0xf) << (4 * (ib % 2))) as u8;
                y[QH + j] |= ((idx >> 4) << ib) as u8;
                let q = (bv[idx] as i32 * l) as f32;
                sumqx += weight[j] * q * xb[j];
                sumq2 += weight[j] * q * q;
            }
        }
    }
    if sumq2 > 0.0 {
        out[..4].copy_from_slice(&(1.0 * sumqx / sumq2).to_le_bytes());
    }
    out
}

fn index(blk: &[u8], j: usize) -> usize {
    let ib = j / 32;
    let k = j % 32;
    let lo = (blk[QS + 32 * (ib / 2) + k] >> (4 * (ib % 2))) & 0xf;
    let hi = (blk[QH + k] >> ib) & 1;
    (lo | hi << 4) as usize
}

/// Port of dequantize_row_iq5_ks.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = row_scale(b);
    for (ibl, blk) in b[4..].chunks(BYTES).enumerate() {
        for j in 0..QK_K {
            let ib = j / 32;
            let dl = d * ((blk[ib] & 254) as i32 - 127) as f32;
            y[ibl * QK_K + j] = dl * table(blk[ib] & 1 != 0)[index(blk, j)] as f32;
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
    let idx = index(blk, j);
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
    .level(idx, 5, value, &IQ5NL_VALUES, 32)
    .math(format!(
        "{:.6}&middot;({}&minus;127) &times; {}",
        d, f, value
    ))
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">32b</span></span> &times; (<span class=\"eq-pill\">s<span class=\"bits\">7b</span></span> &minus; 127) &times; <span class=\"eq-pill\">iq5nl[sh][q]<span class=\"bits\">1b+5b</span></span>",
        "IQ5_K levels with shifted copy and 7-bit scale per 32, one FP32 scale per row.",
        "Same 32 levels and shifted copy as IQ5_K, per 32 weights, with the scale byte of IQ4_KS (shift bit plus an odd scale from -127 to 127). The whole row shares one FP32 scale. 5.25 bpw plus 32 bits per row.",
    )
}
