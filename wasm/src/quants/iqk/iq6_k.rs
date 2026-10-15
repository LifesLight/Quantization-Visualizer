//! IQ6_K: block_iq6_k { fp16 d; u16 extra; i8 scales[16]; u8 qs[128]; u8 qh[64] } -> 212 bytes.
//! 64 non-linear levels or the copy shifted by +1 per 16 weights, 8 bit scale per 16. The search
//! uses the integer table, decoding evaluates the cubic the table was rounded from.

use super::common::*;
use super::lut::{IQ6NL_VALUES, K_SIMPLE_INDEX_IQ6NL};
use super::{formula, KDecode, KType};
use std::sync::OnceLock;

pub static TYPE: KType = KType {
    name: "IQ6_K",
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

const BYTES: usize = 212;
const SCALES: usize = 4;
const QS: usize = 20;
const QH: usize = 148;
const A: f32 = -127.0;
const B: f32 = 6.2568;
const C: f32 = 0.11218;
const D: f32 = 0.0011972;

fn simple() -> &'static Simple {
    static S: OnceLock<Simple> = OnceLock::new();
    S.get_or_init(|| Simple::new(&IQ6NL_VALUES, &K_SIMPLE_INDEX_IQ6NL, 248))
}

/// The tables quantize_iq6_k hands over: the first half of iq6nl_values and that plus 1.
fn values() -> &'static [f32; 128] {
    static V: OnceLock<[f32; 128]> = OnceLock::new();
    V.get_or_init(|| {
        let mut v = [0.0f32; 128];
        for i in 0..64 {
            v[i] = IQ6NL_VALUES[i] as f32;
            v[i + 64] = v[i] + 1.0;
        }
        v
    })
}

/// What dequantize_row_iq6_k decodes index q to.
pub fn cubic(q: usize, shifted: bool) -> f32 {
    let q = q as f32;
    A + q * (B + q * (-C + q * D)) + if shifted { 1.0 } else { 0.0 }
}

/// Port of quantize_row_iq6_k_impl (AVX2 path).
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const NTRY: i32 = 5;
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; nblock * BYTES];
    let mut weight = [0.0f32; 16];
    let vals = values();
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
                64,
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
        let d = -max_scale / 127.0;
        put_u16(y, 0, f16_bits(d));
        put_u16(y, 2, extra);
        let id = 1.0 / d;
        let mut sumqx = 0.0f32;
        let mut sumq2 = 0.0f32;
        for ib in 0..16 {
            let ls = nearest_int(id * scales[ib]).clamp(-127, 127);
            y[SCALES + ib] |= ls as i8 as u8;
            let dl = d * ls as f32;
            if dl == 0.0 {
                continue;
            }
            let bv = if extra & (1 << ib) != 0 {
                &vals[64..]
            } else {
                &vals[..64]
            };
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
                let ibest = best_index_iq6nl(bv, idl * xb[j]);
                y[QS + 32 * (ib32 / 2) + off + j] |= ((ibest & 0xf) << (4 * (ib32 % 2))) as u8;
                y[QH + 32 * (ib32 / 4) + off + j] |= ((ibest >> 4) << (2 * (ib32 % 4))) as u8;
                let q = bv[ibest] * ls as f32;
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

fn index(blk: &[u8], j: usize) -> usize {
    let ib32 = j / 32;
    let k = j % 32;
    let lo = (blk[QS + 32 * (ib32 / 2) + k] >> (4 * (ib32 % 2))) & 0xf;
    let hi = (blk[QH + 32 * (ib32 / 4) + k] >> (2 * (ib32 % 4))) & 3;
    (lo | hi << 4) as usize
}

/// Port of dequantize_row_iq6_k.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    for (ibl, blk) in b.chunks(BYTES).enumerate() {
        let d = f16_val(get_u16(blk, 0));
        let extra = get_u16(blk, 2);
        for j in 0..QK_K {
            let ib = j / 16;
            let dl = d * blk[SCALES + ib] as i8 as f32;
            y[ibl * QK_K + j] = dl * cubic(index(blk, j), extra & (1 << ib) != 0);
        }
    }
}

pub fn block_scales(b: &[u8], ibl: usize) -> Vec<(f32, bool)> {
    let blk = &b[ibl * BYTES..(ibl + 1) * BYTES];
    let d = f16_val(get_u16(blk, 0));
    let extra = get_u16(blk, 2);
    (0..16)
        .map(|ib| (d * blk[SCALES + ib] as i8 as f32, extra & (1 << ib) != 0))
        .collect()
}

pub fn decode(b: &[u8], i: usize) -> KDecode {
    let blk = &b[(i / QK_K) * BYTES..(i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let ib = j / 16;
    let d = f16_val(get_u16(blk, 0));
    let shifted = get_u16(blk, 2) & (1 << ib) != 0;
    let idx = index(blk, j);
    let value = cubic(idx, shifted);
    let f = blk[SCALES + ib] as i8 as i32;
    let base: Vec<f32> = (0..64).map(|q| cubic(q, false)).collect();
    let up: Vec<f32> = (0..64).map(|q| cubic(q, true)).collect();
    let sign = if f < 0 { "&minus;" } else { "" };
    let fmt = |v: f32| format!("{}{:.3}", if v < 0.0 { "&minus;" } else { "" }, v.abs());
    KDecode::new(
        d,
        false,
        f.abs(),
        8,
        d * f as f32,
        if f < 0 { "&minus;{s}" } else { "{s}" },
        shifted,
    )
    .levels(idx, 6, value, base, up)
    .math(format!(
        "{:.5}&middot;{}{} &times; {}",
        d,
        sign,
        f.abs(),
        fmt(value)
    ))
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">s<span class=\"bits\">8b</span></span> &times; <span class=\"eq-pill\">cubic(q) + sh<span class=\"bits\">6b+1b</span></span>",
        "64 levels from a cubic, +1 shifted copy and 8-bit scale per 16, FP16 per 256.",
        "Index q (6 bits) decodes to -127 + 6.2568q - 0.11218q&sup2; + 0.0011972q&sup3;, plus 1 if the block uses the shifted table. The quantizer searches with that cubic rounded to integers, so decoded values sit up to half a step off the ones it chose. Signed 8-bit scale per 16, one FP16 scale per 256. 6.625 bpw.",
    )
}
