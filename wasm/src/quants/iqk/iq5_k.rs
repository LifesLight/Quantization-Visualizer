//! IQ5_K: block_iq5_k { fp16 d; u16 extra; u8 scales_h[4]; u8 scales_l[8]; u8 qs[128]; u8 qh[32] }
//! -> 176 bytes per 256. 32 non-linear levels or the copy shifted by +2 per 16 weights, signed
//! 6 bit scale per 16.

use super::common::*;
use super::lut::{IQ5NL_VALUES, K_SIMPLE_INDEX_IQ5NL};
use super::{formula, KDecode, KType};
use std::sync::OnceLock;

pub static TYPE: KType = KType {
    name: "IQ5_K",
    block_bytes: BYTES,
    row_meta: 0,
    sub_size: 16,
    imatrix_block: 16,
    quantize,
    dequantize,
    decode,
    block_scales,
    formula_html,
};

const BYTES: usize = 176;
const SCALES_H: usize = 4;
const SCALES_L: usize = 8;
const QS: usize = 16;
const QH: usize = 144;

pub fn simple() -> &'static Simple {
    static S: OnceLock<Simple> = OnceLock::new();
    S.get_or_init(|| Simple::new(&IQ5NL_VALUES, &K_SIMPLE_INDEX_IQ5NL, 240))
}

pub fn table(shifted: bool) -> &'static [i8] {
    if shifted {
        &IQ5NL_VALUES[32..]
    } else {
        &IQ5NL_VALUES[..32]
    }
}

/// Port of quantize_row_iq5_k_impl (AVX2 path).
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const NTRY: i32 = 5;
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; nblock * BYTES];
    let mut weight = [0.0f32; 16];
    for ibl in 0..nblock {
        let y = &mut out[ibl * BYTES..(ibl + 1) * BYTES];
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = 2.0 * sum_sq(xbl) / QK_K as f32;
        let mut scales = [0.0f32; 16];
        let (mut max_scale, mut max_abs_scale) = (0.0f32, 0.0f32);
        let mut extra = 0u16;
        for ib in 0..16 {
            let w = qw.map(|qw| &qw[QK_K * ibl + 16 * ib..QK_K * ibl + 16 * ib + 16]);
            let (d, shifted) = simple().find_best_scale(
                32,
                &xbl[16 * ib..16 * ib + 16],
                w,
                sigma2,
                NTRY,
                false,
                1.0,
            );
            scales[ib] = d;
            if shifted {
                extra |= 1 << ib;
            }
            if d.abs() > max_abs_scale {
                max_abs_scale = d.abs();
                max_scale = d;
            }
        }
        if max_abs_scale == 0.0 {
            continue;
        }
        let d = -max_scale / 32.0;
        put_u16(y, 0, f16_bits(d));
        put_u16(y, 2, extra);
        let id = 1.0 / d;
        let mut sumqx = 0.0f32;
        let mut sumq2 = 0.0f32;
        for ib in 0..16 {
            let ls = nearest_int(id * scales[ib]).clamp(-32, 31);
            let uls = ls + 32;
            y[SCALES_L + ib / 2] |= ((uls & 0xf) << (4 * (ib % 2))) as u8;
            y[SCALES_H + ib / 4] |= ((uls >> 4) << (2 * (ib % 4))) as u8;
            let dl = d * ls as f32;
            if dl == 0.0 {
                continue;
            }
            let bv = table(extra & (1 << ib) != 0);
            let xb = &xbl[16 * ib..16 * ib + 16];
            for j in 0..16 {
                weight[j] = match qw {
                    Some(qw) => qw[ibl * QK_K + ib * 16 + j] * (sigma2 + xb[j] * xb[j]).sqrt(),
                    None => 0.25 * sigma2 + xb[j] * xb[j],
                };
            }
            let idl = 1.0 / dl;
            let ib32 = ib / 2;
            let off = 16 * (ib % 2);
            for j in 0..16 {
                let ibest = best_index_iq5nl(bv, idl * xb[j]);
                y[QS + 32 * (ib32 / 2) + off + j] |= ((ibest & 0xf) << (4 * (ib32 % 2))) as u8;
                y[QH + 32 * (ib32 / 8) + off + j] |= ((ibest >> 4) << (ib32 % 8)) as u8;
                let q = (bv[ibest] as i32 * ls) as f32;
                sumqx += weight[j] * q * xb[j];
                sumq2 += weight[j] * q * q;
            }
        }
        if sumq2 > 0.0 {
            put_u16(y, 0, f16_bits(1.0 * sumqx / sumq2));
        }
    }
    out
}

fn field(blk: &[u8], ib: usize) -> i32 {
    let lo = (blk[SCALES_L + ib / 2] >> (4 * (ib % 2))) & 0xf;
    let hi = (blk[SCALES_H + ib / 4] >> (2 * (ib % 4))) & 3;
    lo as i32 | (hi as i32) << 4
}

/// Index of weight j of a super-block: 4 low bits in qs, the 5th in qh.
fn index(blk: &[u8], j: usize) -> usize {
    let ib32 = j / 32;
    let k = j % 32;
    let lo = (blk[QS + 32 * (ib32 / 2) + k] >> (4 * (ib32 % 2))) & 0xf;
    let hi = (blk[QH + k] >> (ib32 % 8)) & 1;
    (lo | hi << 4) as usize
}

/// Port of dequantize_row_iq5_k.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    for (ibl, blk) in b.chunks(BYTES).enumerate() {
        let d = f16_val(get_u16(blk, 0));
        let extra = get_u16(blk, 2);
        for j in 0..QK_K {
            let ib = j / 16;
            let dl = d * (field(blk, ib) - 32) as f32;
            y[ibl * QK_K + j] = dl * table(extra & (1 << ib) != 0)[index(blk, j)] as f32;
        }
    }
}

pub fn block_scales(b: &[u8], ibl: usize) -> Vec<(f32, bool)> {
    let blk = &b[ibl * BYTES..(ibl + 1) * BYTES];
    let d = f16_val(get_u16(blk, 0));
    let extra = get_u16(blk, 2);
    (0..16)
        .map(|ib| (d * (field(blk, ib) - 32) as f32, extra & (1 << ib) != 0))
        .collect()
}

pub fn decode(b: &[u8], i: usize) -> KDecode {
    let blk = &b[(i / QK_K) * BYTES..(i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let ib = j / 16;
    let d = f16_val(get_u16(blk, 0));
    let shifted = get_u16(blk, 2) & (1 << ib) != 0;
    let idx = index(blk, j);
    let value = table(shifted)[idx] as f32;
    let f = field(blk, ib);
    KDecode::new(
        d,
        false,
        f,
        6,
        d * (f - 32) as f32,
        "({s}&minus;32)",
        shifted,
    )
    .level(idx, 5, value, &IQ5NL_VALUES, 32)
    .math(format!(
        "{:.5}&middot;({}&minus;32) &times; {}",
        d, f, value
    ))
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (<span class=\"eq-pill\">s<span class=\"bits\">6b</span></span> &minus; 32) &times; <span class=\"eq-pill\">iq5nl[sh][q]<span class=\"bits\">1b+5b</span></span>",
        "32 non-linear levels, shifted copy and 6-bit scale per 16, FP16 scale per 256.",
        "A 32-level table, denser near zero, plus a copy shifted by +2: every 16 weights pick one (1 bit in extra). Signed 6-bit scale per 16, one FP16 scale per 256. 5.5 bpw.",
    )
}
