//! IQ4_KT: f32 row scale, then block_iq4_kt { u32 qs[32] } -> 128 bytes per 256.
//! qs[0..8] is one word per block of 32: bit 0 picks the codebook bank, bits 1..7 hold ls + 64,
//! bits 8..31 the top 3 index bits of its 8 groups. Then 64 bytes of low index bits and
//! 32 bytes with 4 more bits per group, 15 bits per group of 4 weights in total.

use super::codebook::{self, KtCodebook};
use super::common::*;
use super::KtType;

pub static TYPE: KtType = KtType {
    name: "IQ4_KT",
    group_size: 4,
    bytes: 128,
    copy_pad: false,
    quantize,
    dequantize,
    decode,
    block_scales,
    formula_html,
};

const BYTES: usize = 128;
const QL: usize = 32;
const QH: usize = 96;
const NTRY: i32 = 2;

fn bank_offset(shb: u32) -> u32 {
    if shb & 1 != 0 {
        32768 + 4096
    } else {
        4096
    }
}

fn ls_of(shb: u32) -> i32 {
    ((shb & 0xff) >> 1) as i32 - 64
}

fn group_index(b: &[u8], g: usize) -> u32 {
    let shb = get_u32(b, 4 * (g / 8));
    b[QL + g] as u32
        | (((b[QH + g % 32] as u32) << (8 - 4 * (g / 32))) & 0xf00)
        | (((shb >> (8 + 3 * (g % 8))) & 7) << 12)
}

/// Port of quantize_row_iq4_kt_impl for one (padded) row.
pub fn quantize(x: &[f32], n_per_row: usize, qw: Option<&[f32]>) -> KtRow {
    let q1: &KtCodebook<4> = codebook::iq4kt(0);
    let q2: &KtCodebook<4> = codebook::iq4kt(1);
    let nblock = x.len() / QK_K;
    let mut blocks = vec![0u8; nblock * BYTES];
    let mut weights = vec![0.0f32; nblock * QK_K];

    set_weights(2.0, x, n_per_row, qw, &mut weights);
    let amax_row = max_abs(&x[..n_per_row]);
    if amax_row == 0.0 {
        return KtRow { d: 0.0, blocks };
    }

    let mut scales = vec![0.0f32; nblock * 8];
    let mut best_idx = [0u32; 8];
    let mut amax_scale = 0.0f32;
    let mut max_scale = 0.0f32;

    for ibl in 0..nblock {
        for ib in 0..8 {
            let off = ibl * QK_K + ib * 32;
            let xaux = &x[off..off + 32];
            let weight = &weights[off..off + 32];
            let si = ibl * 8 + ib;
            let amax = max_abs(xaux);
            if amax < 1e-16 {
                scales[si] = 0.0;
                continue;
            }
            let mut best = 0.0f32;
            let scale_0 = 90.0f32.max(124.0 * amax / amax_row);
            for itry in -NTRY..=NTRY {
                let s = 8.0 * itry as f32 + scale_0;
                q1.find_best_match(amax / s, xaux, weight, &mut best_idx);
                let (dp, score_p) = q1.find_best_scale(xaux, weight, &best_idx);
                if score_p > best {
                    best = score_p;
                    scales[si] = dp;
                }
                q1.find_best_match(-amax / s, xaux, weight, &mut best_idx);
                let (dm, score_m) = q1.find_best_scale(xaux, weight, &best_idx);
                if score_m > best {
                    best = score_m;
                    scales[si] = dm;
                }
            }

            let mut flag = 0u32;
            q2.find_best_match(scales[si], xaux, weight, &mut best_idx);
            let (d, score) = q2.find_best_scale(xaux, weight, &best_idx);
            if score > best {
                best = score;
                scales[si] = d;
                flag = 1;
            }
            for itry in -NTRY..=NTRY {
                let s = 8.0 * itry as f32 + scale_0;
                q2.find_best_match(amax / s, xaux, weight, &mut best_idx);
                let (dp, score_p) = q2.find_best_scale(xaux, weight, &best_idx);
                if score_p > best {
                    best = score_p;
                    scales[si] = dp;
                    flag = 1;
                }
                q2.find_best_match(-amax / s, xaux, weight, &mut best_idx);
                let (dm, score_m) = q2.find_best_scale(xaux, weight, &best_idx);
                if score_m > best {
                    best = score_m;
                    scales[si] = dm;
                    flag = 1;
                }
            }
            put_u32(&mut blocks[ibl * BYTES..], 4 * ib, flag);

            let abs_scale = scales[si].abs();
            if abs_scale > amax_scale {
                amax_scale = abs_scale;
                max_scale = scales[si];
            }
        }
    }

    let mut d = -max_scale / 64.0;
    let mut row_d = d;
    if d == 0.0 {
        return KtRow { d: row_d, blocks };
    }

    let id = 1.0 / d;
    let mut sumqx = 0.0f32;
    let mut sumq2 = 0.0f32;
    for ibl in 0..nblock {
        let b = &mut blocks[ibl * BYTES..(ibl + 1) * BYTES];
        b[QH..QH + 32].fill(0);
        for ib in 0..8 {
            let off = ibl * QK_K + ib * 32;
            let xaux = &x[off..off + 32];
            let weight = &weights[off..off + 32];
            let mut shb = get_u32(b, 4 * ib);
            let cb = if shb & 1 != 0 { q2 } else { q1 };
            let ls = nearest_int(id * scales[ibl * 8 + ib]).min(63);
            shb = (shb & !0xff) | ((((ls + 64) << 1) as u32 | (shb & 1)) & 0xff);
            let dl = d * ls as f32;
            cb.find_best_match(dl, xaux, weight, &mut best_idx);
            for j in 0..8 {
                let g = 8 * ib + j;
                let idx = best_idx[j];
                shb |= (idx >> 12) << (8 + 3 * j);
                b[QL + g] = (idx & 255) as u8;
                b[QH + g % 32] |= (((idx >> 8) & 0xf) << (4 * (g / 32))) as u8;
                let ql = cb.group(idx);
                for k in 0..4 {
                    let q = ql[k] * ls as f32;
                    sumqx += weight[4 * j + k] * xaux[4 * j + k] * q;
                    sumq2 += weight[4 * j + k] * q * q;
                }
            }
            put_u32(b, 4 * ib, shb);
        }
    }
    if sumq2 > 0.0 {
        d = sumqx / sumq2;
        row_d = d;
    }
    KtRow { d: row_d, blocks }
}

/// Port of dequantize_row_iq4_kt.
pub fn dequantize(row: &KtRow, y: &mut [f32]) {
    let mut v = [0.0f32; 4];
    for (ibl, b) in row.blocks.chunks(BYTES).enumerate() {
        for ib in 0..8 {
            let shb = get_u32(b, 4 * ib);
            let sl = row.d * ls_of(shb) as f32;
            for ig in 0..8 {
                gen_group(group_index(b, 8 * ib + ig), bank_offset(shb), &mut v);
                for j in 0..4 {
                    y[ibl * QK_K + ib * 32 + 4 * ig + j] = sl * v[j];
                }
            }
        }
    }
}

pub fn block_scales(row: &KtRow, ibl: usize) -> Vec<f32> {
    let b = &row.blocks[ibl * BYTES..(ibl + 1) * BYTES];
    (0..8)
        .map(|ib| row.d * ls_of(get_u32(b, 4 * ib)) as f32)
        .collect()
}

pub fn decode(row: &KtRow, i: usize) -> KtDecode {
    let b = &row.blocks[(i / QK_K) * BYTES..(i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let shb = get_u32(b, 4 * (j / 32));
    let ls = ls_of(shb);
    let idx = group_index(b, j / 4);
    let offset = bank_offset(shb);
    let lane = j % 4;
    let value = codebook::state_value(codebook::lane_state(idx, offset, lane));
    KtDecode {
        d: row.d,
        d_label: "d",
        field: ls + 64,
        dl: row.d * ls as f32,
        dl_expr: "({s}&minus;64)",
        idx,
        index_bits: 15,
        offset,
        lane,
        value,
        neg: None,
        math: format!("{:.5} &times; {} &times; {}", row.d, ls, value),
    }
}

pub fn formula_html() -> String {
    super::formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">32b</span></span> &times; (<span class=\"eq-pill\">s<span class=\"bits\">7b</span></span> &minus; 64) &times; <span class=\"eq-pill\">gen(idx + bank)<sub>j</sub><span class=\"bits\">15b+1b</span></span>",
        "4-element groups, 15-bit seeds, 2 banks, signed 7-bit scale per 32.",
        "Each 15-bit index seeds the LCG with offset 4096, or 36864 when the block's bank bit is set, so every 32-block picks one of two 32768-entry codebooks. Search runs over 625 clusters per bank. Signed 7-bit scale per 32 (stored as ls + 64), one FP32 scale per row. 4.0 bpw plus 32 bits per row.",
    )
}
