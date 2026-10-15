//! IQ3_KS: fp16 row scale, then block_iq3_ks { u16 extra; u8 scales[4]; u8 qs[64]; u8 qh[32] }
//! -> 102 bytes. IQ3_K levels per 32 weights, signed 5 bit scale (4 bits in scales, the 5th in
//! the low byte of extra), shift bits in the high byte of extra.

use super::common::*;
use super::lut::{IQ3NL_VALUES, K_SIMPLE_INDEX_IQ3NL};
use super::{formula, KDecode, KType};
use std::sync::OnceLock;

pub static TYPE: KType = KType {
    name: "IQ3_KS",
    block_bytes: BYTES,
    row_meta: 2,
    sub_size: 32,
    imatrix_block: 32,
    quantize,
    dequantize,
    decode,
    block_scales,
    formula_html,
};

const BYTES: usize = 102;
const SCALES: usize = 2;
const QS: usize = 6;
const QH: usize = 70;

fn simple() -> &'static Simple {
    static S: OnceLock<Simple> = OnceLock::new();
    S.get_or_init(|| Simple::new(&IQ3NL_VALUES, &K_SIMPLE_INDEX_IQ3NL, 127))
}

fn table(shifted: bool) -> &'static [i8] {
    if shifted {
        &IQ3NL_VALUES[8..]
    } else {
        &IQ3NL_VALUES[..8]
    }
}

/// Port of quantize_row_iq3_ks_impl (AVX2 path).
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const BS: usize = 32;
    const NTRY: i32 = 5;
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; 2 + nblock * BYTES];
    let mut all_scales = vec![0.0f32; nblock * 8];
    let mut weight = [0.0f32; BS];
    let sigma2_of = |xbl: &[f32]| sum_sq(xbl) * (2.0 / QK_K as f32);

    let (mut amax_scale, mut max_scale) = (0.0f32, 0.0f32);
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = sigma2_of(xbl);
        let mut extra = 0u16;
        for ib in 0..8 {
            let w = qw.map(|qw| &qw[QK_K * ibl + BS * ib..QK_K * ibl + BS * ib + BS]);
            let (d, shifted) = simple().find_best_scale(
                8,
                &xbl[BS * ib..BS * ib + BS],
                w,
                sigma2,
                NTRY,
                false,
                1.0,
            );
            if shifted {
                extra |= 1 << (8 + ib);
            }
            all_scales[ibl * 8 + ib] = d;
            if d.abs() > amax_scale {
                amax_scale = d.abs();
                max_scale = d;
            }
        }
        put_u16(&mut out, 2 + ibl * BYTES, extra);
    }
    let d = -max_scale / 16.0;
    put_u16(&mut out, 0, f16_bits(d));
    if d == 0.0 {
        return out;
    }
    let id = 1.0 / d;
    let mut sumqx = 0.0f32;
    let mut sumq2 = 0.0f32;
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = sigma2_of(xbl);
        let y = &mut out[2 + ibl * BYTES..2 + (ibl + 1) * BYTES];
        for ib in 0..8 {
            let bv = table((get_u16(y, 0) >> (8 + ib)) & 1 != 0);
            let l = nearest_int(id * all_scales[ibl * 8 + ib]).clamp(-16, 15);
            let ul = (l + 16) as u8;
            y[SCALES + ib % 4] |= (ul & 0xf) << (4 * (ib / 4));
            let e = get_u16(y, 0) | (((ul >> 4) as u16) << ib);
            put_u16(y, 0, e);
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
                let i = best_index_iq3nl(bv, idl * xb[j]);
                y[QS + (ib / 4) * BS + j] |= ((i & 3) << (2 * (ib % 4))) as u8;
                y[QH + (ib / 8) * BS + j] |= ((i >> 2) << (ib % 8)) as u8;
                let q = (bv[i] as i32 * l) as f32;
                sumqx += weight[j] * q * xb[j];
                sumq2 += weight[j] * q * q;
            }
        }
    }
    if sumq2 > 0.0 {
        put_u16(&mut out, 0, f16_bits(1.0 * sumqx / sumq2));
    }
    out
}

fn field(blk: &[u8], ib: usize) -> i32 {
    let lo = (blk[SCALES + ib % 4] >> (4 * (ib / 4))) & 0xf;
    let hi = (get_u16(blk, 0) >> ib) & 1;
    lo as i32 | (hi as i32) << 4
}

/// Port of dequantize_row_iq3_ks.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(get_u16(b, 0));
    for (ibl, blk) in b[2..].chunks(BYTES).enumerate() {
        let extra = get_u16(blk, 0);
        for i128 in 0..2 {
            for ib in 0..4 {
                let k = 4 * i128 + ib;
                let dl = d * (field(blk, k) - 16) as f32;
                let v = table((extra >> (8 + k)) & 1 != 0);
                let o = ibl * QK_K + 32 * k;
                for j in 0..32 {
                    let q = ((blk[QS + 32 * i128 + j] >> (2 * ib)) & 3)
                        | (((blk[QH + j] >> k) & 1) << 2);
                    y[o + j] = dl * v[q as usize] as f32;
                }
            }
        }
    }
}

pub fn block_scales(b: &[u8], ibl: usize) -> Vec<(f32, bool)> {
    let d = f16_val(get_u16(b, 0));
    let blk = &b[2 + ibl * BYTES..2 + (ibl + 1) * BYTES];
    let extra = get_u16(blk, 0);
    (0..8)
        .map(|ib| {
            (
                d * (field(blk, ib) - 16) as f32,
                (extra >> (8 + ib)) & 1 != 0,
            )
        })
        .collect()
}

pub fn decode(b: &[u8], i: usize) -> KDecode {
    let d = f16_val(get_u16(b, 0));
    let blk = &b[2 + (i / QK_K) * BYTES..2 + (i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let ib = j / 32;
    let k = j % 32;
    let shifted = (get_u16(blk, 0) >> (8 + ib)) & 1 != 0;
    let idx = (((blk[QS + 32 * (ib / 4) + k] >> (2 * (ib % 4))) & 3)
        | (((blk[QH + k] >> ib) & 1) << 2)) as usize;
    let value = table(shifted)[idx] as f32;
    let f = field(blk, ib);
    KDecode::new(
        d,
        true,
        f,
        5,
        d * (f - 16) as f32,
        "({s}&minus;16)",
        shifted,
    )
    .level(idx, 3, value, &IQ3NL_VALUES, 8)
    .math(format!(
        "{:.5}&middot;({}&minus;16) &times; {}",
        d, f, value
    ))
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (<span class=\"eq-pill\">s<span class=\"bits\">5b</span></span> &minus; 16) &times; <span class=\"eq-pill\">iq3nl[sh][q]<span class=\"bits\">1b+3b</span></span>",
        "IQ3_K levels with shifted copy and 5-bit scale per 32, one FP16 scale per row.",
        "Same 8 levels and shifted copy as IQ3_K, but per 32 weights with a plain signed 5-bit scale, and the scales of the whole row share one FP16 scale instead of one per 256. 3.1875 bpw plus 16 bits per row.",
    )
}
