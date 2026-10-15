//! IQ4_K: block_iq4_k { fp16 d; u16 extra; u8 scales_h[4]; u8 scales_l[8]; u8 qs[128] }
//! -> 144 bytes per 256. IQ4_NL style 16 levels or the copy shifted by +4 per 16 weights,
//! signed 6 bit scale per 16.

use super::common::*;
use super::lut::{IQ4K_VALUES, K_SIMPLE_INDEX_IQ4NL};
use super::{formula, KDecode, KType};
use std::sync::OnceLock;

pub static TYPE: KType = KType {
    name: "IQ4_K",
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

const BYTES: usize = 144;
const SCALES_H: usize = 4;
const SCALES_L: usize = 8;
const QS: usize = 16;

pub fn simple() -> &'static Simple {
    static S: OnceLock<Simple> = OnceLock::new();
    S.get_or_init(|| Simple::new(&IQ4K_VALUES, &K_SIMPLE_INDEX_IQ4NL, 240))
}

pub fn table(shifted: bool) -> &'static [i8] {
    if shifted {
        &IQ4K_VALUES[16..]
    } else {
        &IQ4K_VALUES[..16]
    }
}

/// Port of quantize_row_iq4_k_impl_bs16 (AVX2 path), one super-block at a time.
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const BS: usize = 16;
    const NTRY: i32 = 7;
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; nblock * BYTES];
    let mut weight = [0.0f32; BS];
    let mut l = [0u8; QK_K];
    for ibl in 0..nblock {
        let y = &mut out[ibl * BYTES..(ibl + 1) * BYTES];
        let xs = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let qws = qw.map(|qw| &qw[ibl * QK_K..(ibl + 1) * QK_K]);
        let sigma2 = sum_sq(xs) * (2.0 / QK_K as f32);
        let set_weights = |weight: &mut [f32; BS], ib: usize| {
            let xb = &xs[ib * BS..ib * BS + BS];
            for j in 0..BS {
                weight[j] = match qws {
                    Some(q) => q[ib * BS + j] * (sigma2 + xb[j] * xb[j]).sqrt(),
                    None => xb[j] * xb[j],
                };
            }
        };

        let mut scales = [0.0f32; QK_K / BS];
        let (mut max_scale, mut amax_scale) = (0.0f32, 0.0f32);
        let mut extra = 0u16;
        for ib in 0..QK_K / BS {
            set_weights(&mut weight, ib);
            let (d, shifted) = simple().search(16, &xs[ib * BS..ib * BS + BS], &weight, NTRY, 1.0);
            if shifted {
                extra |= 1 << ib;
            }
            scales[ib] = d;
            if d.abs() > amax_scale {
                amax_scale = d.abs();
                max_scale = d;
            }
        }
        let d = -max_scale / 32.0;
        put_u16(y, 0, f16_bits(d));
        put_u16(y, 2, extra);
        let id = if d != 0.0 { 1.0 / d } else { 0.0 };
        let mut sumqx = 0.0f32;
        let mut sumq2 = 0.0f32;
        for ib in 0..QK_K / BS {
            let bv = table(extra & (1 << ib) != 0);
            let mut ls = nearest_int(id * scales[ib]).clamp(-32, 31);
            let dl = d * ls as f32;
            let idl = if dl != 0.0 { 1.0 / dl } else { 0.0 };
            let xb = &xs[ib * BS..ib * BS + BS];
            set_weights(&mut weight, ib);
            for j in 0..BS {
                let lj = best_index_iq4nl(bv, idl * xb[j]);
                l[ib * BS + j] = lj as u8;
                let q = (bv[lj] as i32 * ls) as f32;
                sumqx += weight[j] * q * xb[j];
                sumq2 += weight[j] * q * q;
            }
            ls += 32;
            let (l_l, l_h) = ((ls & 0xf) as u8, (ls >> 4) as u16);
            if ib % 2 == 0 {
                y[SCALES_L + ib / 2] = l_l;
            } else {
                y[SCALES_L + ib / 2] |= l_l << 4;
            }
            let o = SCALES_H + 2 * (ib / 8);
            let sh = get_u16(y, o) | (l_h << (2 * (ib % 8)));
            put_u16(y, o, sh);
        }
        if sumq2 > 0.0 {
            put_u16(y, 0, f16_bits(1.0 * sumqx / sumq2));
        }
        for i in 0..QK_K / 32 {
            for j in 0..16 {
                y[QS + 16 * i + j] = l[32 * i + j] | (l[32 * i + 16 + j] << 4);
            }
        }
    }
    out
}

fn field(blk: &[u8], ib: usize) -> i32 {
    let lo = (blk[SCALES_L + ib / 2] >> (4 * (ib % 2))) & 0xf;
    let hi = (blk[SCALES_H + ib / 4] >> (2 * (ib % 4))) & 3;
    lo as i32 | (hi as i32) << 4
}

/// Port of dequantize_row_iq4_k.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    for (ibl, blk) in b.chunks(BYTES).enumerate() {
        let d = f16_val(get_u16(blk, 0));
        let extra = get_u16(blk, 2);
        for ib in 0..8 {
            let dl1 = d * (field(blk, 2 * ib) - 32) as f32;
            let dl2 = d * (field(blk, 2 * ib + 1) - 32) as f32;
            let v1 = table(extra & (1 << (2 * ib)) != 0);
            let v2 = table(extra & (1 << (2 * ib + 1)) != 0);
            let o = ibl * QK_K + 32 * ib;
            for j in 0..16 {
                let q = blk[QS + 16 * ib + j];
                y[o + j] = dl1 * v1[(q & 0xf) as usize] as f32;
                y[o + j + 16] = dl2 * v2[(q >> 4) as usize] as f32;
            }
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
    let q = blk[QS + 16 * (j / 32) + j % 16];
    let idx = if (j / 16) % 2 == 0 { q & 0xf } else { q >> 4 } as usize;
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
    .level(idx, 4, value, &IQ4K_VALUES, 16)
    .math(format!(
        "{:.5}&middot;({}&minus;32) &times; {}",
        d, f, value
    ))
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (<span class=\"eq-pill\">s<span class=\"bits\">6b</span></span> &minus; 32) &times; <span class=\"eq-pill\">iq4k[sh][q]<span class=\"bits\">1b+4b</span></span>",
        "16 non-linear levels, shifted copy and 6-bit scale per 16, FP16 scale per 256.",
        "The IQ4_NL table plus a copy shifted by +4: every 16 weights pick one (1 bit in extra), which fills the gaps between the levels where the block needs it. Signed 6-bit scale per 16, one FP16 scale per 256. 4.5 bpw.",
    )
}
