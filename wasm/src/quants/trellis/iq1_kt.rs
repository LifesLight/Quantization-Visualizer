//! IQ1_KT: f32 row scale, then block_iq1_kt { u8 sh[8]; u8 ql[32]; u8 qh[16] } -> 56 bytes per 256.
//! Groups of 8 store a 13 bit index: 8 bits in ql, 4 in qh, the 13th in the top of sh.
//! The low nibble of sh is a 4 bit index into iq4k_values as block scale.

use super::codebook::{self, KtCodebook};
use super::common::*;
use super::KtType;

pub static TYPE: KtType = KtType {
    name: "IQ1_KT",
    group_size: 8,
    bytes: 56,
    copy_pad: false,
    quantize,
    dequantize,
    decode,
    block_scales,
    formula_html,
};

const BYTES: usize = 56;
const SH: usize = 0;
const QL: usize = 8;
const QH: usize = 40;
const FUDGE: f32 = 1.07;

fn group_index(b: &[u8], g: usize) -> u32 {
    let ib = g / 4;
    let ig = g % 4;
    let mut idx =
        b[QL + g] as u32 | (((b[QH + (ib % 4) * 4 + ig] as u32) << (8 - 4 * (ib / 4))) & 0xf00);
    idx |= ((b[SH + ib] as u32) << (8 - ig)) & 0x1000;
    idx
}

/// Port of quantize_row_iq1_kt_impl for one (padded) row.
pub fn quantize(x: &[f32], n_per_row: usize, qw: Option<&[f32]>) -> KtRow {
    let cb: &KtCodebook<8> = codebook::iq1kt();
    let nblock = x.len() / QK_K;
    let mut blocks = vec![0u8; nblock * BYTES];
    let mut weights = vec![0.0f32; nblock * QK_K];
    let mut scales = vec![0.0f32; nblock * 8];
    let mut all_idx = vec![0u32; nblock * 32];
    let mut best_idx = [0u32; 8];

    set_weights(2.0, x, n_per_row, qw, &mut weights);
    let amax_row = max_abs(&x[..n_per_row]);

    let mut amax_scale = 0.0f32;
    let mut max_scale = 0.0f32;
    for ibl in 0..nblock {
        for ib in 0..8 {
            let off = ibl * QK_K + ib * 32;
            let xb = &x[off..off + 32];
            let weight = &weights[off..off + 32];
            let si = ibl * 8 + ib;
            let amax = max_abs(xb);
            if amax < 1e-16 {
                scales[si] = 0.0;
                all_idx[off / 8..off / 8 + 4].fill(0);
                continue;
            }
            let mut scale_0 = 90.0f32.max(124.0 * amax / amax_row);
            cb.find_best_match(amax / scale_0, xb, weight, &mut best_idx[0..4]);
            let (dp, mut score_p) = cb.find_best_scale(xb, weight, &best_idx[0..4]);
            cb.find_best_match(-amax / scale_0, xb, weight, &mut best_idx[4..8]);
            let (dm, score_m) = cb.find_best_scale(xb, weight, &best_idx[4..8]);

            let first = if score_p > score_m {
                scales[si] = dp;
                0
            } else {
                scales[si] = dm;
                score_p = score_m;
                4
            };
            all_idx[off / 8..off / 8 + 4].copy_from_slice(&best_idx[first..first + 4]);

            scale_0 -= 8.0;
            cb.find_best_match(amax / scale_0, xb, weight, &mut best_idx[0..4]);
            let (dp1, score_p1) = cb.find_best_scale(xb, weight, &best_idx[0..4]);
            cb.find_best_match(-amax / scale_0, xb, weight, &mut best_idx[4..8]);
            let (dm1, score_m1) = cb.find_best_scale(xb, weight, &best_idx[4..8]);

            if score_p1 > score_p || score_m1 > score_p {
                let first = if score_p1 > score_m1 {
                    scales[si] = dp1;
                    0
                } else {
                    scales[si] = dm1;
                    4
                };
                all_idx[off / 8..off / 8 + 4].copy_from_slice(&best_idx[first..first + 4]);
            }

            let abs_scale = scales[si].abs();
            if abs_scale > amax_scale {
                amax_scale = abs_scale;
                max_scale = scales[si];
            }
        }
    }

    if max_scale == 0.0 {
        return KtRow { d: 0.0, blocks };
    }

    let mut d = max_scale / IQ4K_VALUES[0] as f32;
    let mut best = 0.0f32;
    for itry in -9..=9 {
        let id = (itry + IQ4K_VALUES[0] as i32) as f32 / max_scale;
        let mut sumqx = 0.0f32;
        let mut sumq2 = 0.0f32;
        for ibl in 0..nblock {
            for ib in 0..8 {
                let off = ibl * QK_K + ib * 32;
                let ls = best_index_iq4nl(id * scales[ibl * 8 + ib]);
                let dl = IQ4K_VALUES[ls] as f32;
                for ig in 0..4 {
                    let qb = cb.group(all_idx[off / 8 + ig]);
                    for j in 0..8 {
                        let jj = off + ig * 8 + j;
                        let q = dl * qb[j];
                        sumqx += weights[jj] * x[jj] * q;
                        sumq2 += weights[jj] * q * q;
                    }
                }
            }
        }
        if sumq2 > 0.0 && sumqx * sumqx > best * sumq2 {
            d = sumqx / sumq2;
            best = d * sumqx;
        }
    }

    let id = if d != 0.0 { 1.0 / d } else { 0.0 };
    for ibl in 0..nblock {
        for ib in 0..8 {
            blocks[ibl * BYTES + SH + ib] = best_index_iq4nl(id * scales[ibl * 8 + ib]) as u8;
        }
    }

    let mut row_d = FUDGE * d;
    if d == 0.0 {
        return KtRow { d: row_d, blocks };
    }

    let mut sumqx = 0.0f32;
    let mut sumq2 = 0.0f32;
    for ibl in 0..nblock {
        let b = &mut blocks[ibl * BYTES..(ibl + 1) * BYTES];
        for ib in 0..8 {
            let off = ibl * QK_K + ib * 32;
            let xb = &x[off..off + 32];
            let weight = &weights[off..off + 32];
            let ls = IQ4K_VALUES[(b[SH + ib] & 0xf) as usize] as i32;
            let dl = d * ls as f32;
            cb.find_best_match(dl, xb, weight, &mut best_idx[0..4]);

            let prev = &mut all_idx[off / 8..off / 8 + 4];
            let mut mse1 = 0.0f32;
            let mut mse2 = 0.0f32;
            for ig in 0..4 {
                let q1 = cb.group(prev[ig]);
                let q2 = cb.group(best_idx[ig]);
                for j in 0..8 {
                    let jj = ig * 8 + j;
                    let diff1 = xb[jj] - dl * q1[j];
                    let diff2 = xb[jj] - dl * q2[j];
                    mse1 += weight[jj] * diff1 * diff1;
                    mse2 += weight[jj] * diff2 * diff2;
                }
            }
            if mse1 < mse2 {
                best_idx[0..4].copy_from_slice(prev);
            } else {
                prev.copy_from_slice(&best_idx[0..4]);
            }

            for j in 0..4 {
                let idx = best_idx[j];
                b[QL + ib * 4 + j] = (idx & 0xff) as u8;
                b[QH + (ib % 4) * 4 + j] |= (((idx >> 8) & 0xf) << (4 * (ib / 4))) as u8;
                b[SH + ib] |= ((idx >> 12) << (4 + j)) as u8;
                let ql = cb.group(idx);
                for k in 0..8 {
                    let q = ql[k] * ls as f32;
                    sumqx += weight[8 * j + k] * xb[8 * j + k] * q;
                    sumq2 += weight[8 * j + k] * q * q;
                }
            }
        }
    }
    if sumq2 > 0.0 {
        d = sumqx / sumq2;
        row_d = d * FUDGE;
    }
    KtRow { d: row_d, blocks }
}

/// Port of dequantize_row_iq1_kt.
pub fn dequantize(row: &KtRow, y: &mut [f32]) {
    let mut v = [0.0f32; 8];
    for (ibl, b) in row.blocks.chunks(BYTES).enumerate() {
        for ib in 0..8 {
            let sl = row.d * IQ4K_VALUES[(b[SH + ib] & 0xf) as usize] as f32;
            for ig in 0..4 {
                gen_group(group_index(b, ib * 4 + ig), 4096, &mut v);
                for j in 0..8 {
                    y[ibl * QK_K + ib * 32 + ig * 8 + j] = sl * v[j];
                }
            }
        }
    }
}

pub fn block_scales(row: &KtRow, ibl: usize) -> Vec<f32> {
    let b = &row.blocks[ibl * BYTES..(ibl + 1) * BYTES];
    (0..8)
        .map(|ib| row.d * IQ4K_VALUES[(b[SH + ib] & 0xf) as usize] as f32)
        .collect()
}

pub fn decode(row: &KtRow, i: usize) -> KtDecode {
    let b = &row.blocks[(i / QK_K) * BYTES..(i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let field = (b[SH + j / 32] & 0xf) as usize;
    let idx = group_index(b, j / 8);
    let lane = j % 8;
    let value = codebook::state_value(codebook::lane_state(idx, 4096, lane));
    KtDecode {
        d: row.d,
        d_label: "d",
        field: field as i32,
        dl: row.d * IQ4K_VALUES[field] as f32,
        dl_expr: "iq4k[{s}]",
        idx,
        index_bits: 13,
        offset: 4096,
        lane,
        value,
        neg: None,
        math: format!(
            "{:.5} &times; {} &times; {}",
            row.d, IQ4K_VALUES[field], value
        ),
    }
}

pub fn formula_html() -> String {
    super::formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">32b</span></span> &times; <span class=\"eq-pill\">iq4k[s]<span class=\"bits\">4b</span></span> &times; <span class=\"eq-pill\">gen(idx)<sub>j</sub><span class=\"bits\">13b</span></span>",
        "8-element groups, 13-bit computed-codebook seeds, 4-bit scale per 32.",
        "Each 13-bit index plus 4096 seeds an LCG, every lane is the sum of 4 random 6-bit bytes minus 126, roughly Gaussian. No stored codebook, no Viterbi path, every group index is picked independently. Search runs over 256 clusters of the 8192 points. Non-linear 4-bit scale per 32 (iq4k values), one FP32 scale per row with a 1.07 fudge factor. 1.75 bpw plus 32 bits per row.",
    )
}
