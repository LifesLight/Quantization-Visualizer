//! IQ3_K: block_iq3_k { fp16 d; u16 extra; u16 scales_h; u8 scales_l[8]; u8 qs[64]; u8 qh[32] }
//! -> 110 bytes per 256. 8 non-linear levels or their shifted copy per 16 weights, odd 4 bit
//! scale (2s+1) with a separate sign bit in scales_h.

use super::common::*;
use super::lut::{IQ3NL_VALUES, K_SIMPLE_INDEX_IQ3NL};
use super::{formula, KDecode, KType};
use std::sync::OnceLock;

pub static TYPE: KType = KType {
    name: "IQ3_K",
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

const BYTES: usize = 110;
const SCALES_H: usize = 4;
const SCALES_L: usize = 6;
const QS: usize = 14;
const QH: usize = 78;
const FUDGE: f32 = 1.01;

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

/// Port of quantize_row_iq3_k_impl (AVX2 path).
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const NTRY: i32 = 3;
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; nblock * BYTES];
    let mut weight = [0.0f32; 16];
    for ibl in 0..nblock {
        let y = &mut out[ibl * BYTES..(ibl + 1) * BYTES];
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = 1.5 * sum_sq(xbl) / QK_K as f32;
        let mut extra = 0u16;
        let mut scales = [0.0f32; 16];
        let mut max_abs_scale = 0.0f32;
        for ib in 0..16 {
            let w = qw.map(|qw| &qw[QK_K * ibl + 16 * ib..QK_K * ibl + 16 * ib + 16]);
            let (d, shifted) = simple().find_best_scale(
                8,
                &xbl[16 * ib..16 * ib + 16],
                w,
                sigma2,
                NTRY,
                true,
                2.0,
            );
            scales[ib] = d;
            if shifted {
                extra |= 1 << ib;
            }
            max_abs_scale = max_abs_scale.max(d.abs());
        }
        if max_abs_scale == 0.0 {
            continue;
        }
        let d = max_abs_scale / 31.0;
        put_u16(y, 2, extra);
        let id = 1.0 / d;
        let mut sumqx = 0.0f32;
        let mut sumq2 = 0.0f32;
        for ib in 0..16 {
            let mut ls = nearest_int(0.5 * (id * scales[ib].abs() - 1.0)).clamp(0, 15);
            y[SCALES_L + ib / 2] |= (ls << (4 * (ib % 2))) as u8;
            if scales[ib] < 0.0 {
                let sh = get_u16(y, SCALES_H) | (1 << ib);
                put_u16(y, SCALES_H, sh);
            }
            ls = (2 * ls + 1) * if scales[ib] < 0.0 { -1 } else { 1 };
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
                let ibest = best_index_iq3nl(bv, idl * xb[j]);
                y[QS + 32 * (ib32 / 4) + off + j] |= ((ibest & 3) << (2 * (ib32 % 4))) as u8;
                y[QH + 32 * (ib32 / 8) + off + j] |= ((ibest >> 2) << (ib32 % 8)) as u8;
                let q = (bv[ibest] as i32 * ls) as f32;
                sumqx += weight[j] * q * xb[j];
                sumq2 += weight[j] * q * q;
            }
        }
        put_u16(
            y,
            0,
            f16_bits(FUDGE * if sumq2 > 0.0 { sumqx / sumq2 } else { d }),
        );
    }
    out
}

fn scale(blk: &[u8], ib: usize) -> (i32, bool) {
    let f = ((blk[SCALES_L + ib / 2] >> (4 * (ib % 2))) & 0xf) as i32;
    (f, get_u16(blk, SCALES_H) & (1 << ib) != 0)
}

fn dl_of(d: f32, f: i32, neg: bool) -> f32 {
    d * ((2 * f + 1) * if neg { -1 } else { 1 }) as f32
}

/// Port of dequantize_row_iq3_k.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    for (ibl, blk) in b.chunks(BYTES).enumerate() {
        let d = f16_val(get_u16(blk, 0));
        let extra = get_u16(blk, 2);
        for ib32 in 0..8 {
            let (f1, n1) = scale(blk, 2 * ib32);
            let (f2, n2) = scale(blk, 2 * ib32 + 1);
            let (dl1, dl2) = (dl_of(d, f1, n1), dl_of(d, f2, n2));
            let v1 = table(extra & (1 << (2 * ib32)) != 0);
            let v2 = table(extra & (1 << (2 * ib32 + 1)) != 0);
            let (sl, shh) = (2 * (ib32 % 4), ib32 % 8);
            let qs = QS + 32 * (ib32 / 4);
            let o = ibl * QK_K + 32 * ib32;
            for j in 0..16 {
                let i1 = ((blk[qs + j] >> sl) & 3) | (((blk[QH + j] >> shh) & 1) << 2);
                let i2 = ((blk[qs + j + 16] >> sl) & 3) | (((blk[QH + j + 16] >> shh) & 1) << 2);
                y[o + j] = dl1 * v1[i1 as usize] as f32;
                y[o + j + 16] = dl2 * v2[i2 as usize] as f32;
            }
        }
    }
}

pub fn block_scales(b: &[u8], ibl: usize) -> Vec<(f32, bool)> {
    let blk = &b[ibl * BYTES..(ibl + 1) * BYTES];
    let d = f16_val(get_u16(blk, 0));
    let extra = get_u16(blk, 2);
    (0..16)
        .map(|ib| {
            let (f, neg) = scale(blk, ib);
            (dl_of(d, f, neg), extra & (1 << ib) != 0)
        })
        .collect()
}

pub fn decode(b: &[u8], i: usize) -> KDecode {
    let blk = &b[(i / QK_K) * BYTES..(i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let ib = j / 16;
    let ib32 = j / 32;
    let d = f16_val(get_u16(blk, 0));
    let shifted = get_u16(blk, 2) & (1 << ib) != 0;
    let (f, neg) = scale(blk, ib);
    let k = j % 32;
    let idx = (((blk[QS + 32 * (ib32 / 4) + k] >> (2 * (ib32 % 4))) & 3)
        | (((blk[QH + k] >> (ib32 % 8)) & 1) << 2)) as usize;
    let value = table(shifted)[idx] as f32;
    let sign = if neg { "&minus;" } else { "" };
    KDecode::new(
        d,
        false,
        f,
        4,
        dl_of(d, f, neg),
        if neg {
            "&minus;(2&middot;{s}+1)"
        } else {
            "(2&middot;{s}+1)"
        },
        shifted,
    )
    .level(idx, 3, value, &IQ3NL_VALUES, 8)
    .math(format!(
        "{:.5}&middot;{}{} &times; {}",
        d,
        sign,
        2 * f + 1,
        value
    ))
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">&plusmn;(2s+1)<span class=\"bits\">1b+4b</span></span> &times; <span class=\"eq-pill\">iq3nl[sh][q]<span class=\"bits\">1b+3b</span></span>",
        "8 non-linear levels, shifted copy and odd signed scale per 16, FP16 scale per 256.",
        "Levels {-63, -40, -23, -10, 1, 13, 28, 47}, every 16 weights pick that table or the copy shifted by +4 (1 bit in extra). The scale per 16 is odd, 2s+1 with s in 4 bits, and its sign is a separate bit. One FP16 scale per 256 with a 1.01 fudge. 3.4375 bpw.",
    )
}
