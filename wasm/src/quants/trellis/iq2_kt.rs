//! IQ2_KT: f32 row scale, then block_iq2_kt { u8 scales[4]; u16 ql[32] } -> 68 bytes per 256.
//! Groups of 8 weights store a 16 bit index into the computed codebook, blocks of 32 a 4 bit
//! index into iq4k_values as scale.

use super::codebook::{self, KtCodebook};
use super::common::*;
use super::KtType;

pub static TYPE: KtType = KtType {
    name: "IQ2_KT",
    group_size: 8,
    bytes: 68,
    copy_pad: true,
    quantize,
    dequantize,
    decode,
    block_scales,
    formula_html,
};

const BYTES: usize = 68;
const QL: usize = 4;
/// Applied to the row scale at inference (CPU gemm and CUDA), dequantize_row_iq2_kt leaves it out.
const RUNTIME_SCALE: f32 = 1.05;

fn scale_field(b: &[u8], ib: usize) -> usize {
    ((b[ib % 4] >> (4 * (ib / 4))) & 0xf) as usize
}

/// Port of quantize_row_iq2_kt_impl for one (padded) row.
pub fn quantize(x: &[f32], n_per_row: usize, qw: Option<&[f32]>) -> KtRow {
    let cb: &KtCodebook<8> = codebook::iq2kt();
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
            let amax = max_abs(xb);
            if amax < 1e-16 {
                scales[ibl * 8 + ib] = 0.0;
                all_idx[off / 8..off / 8 + 4].fill(0);
                continue;
            }
            let scale_0 = 90.0f32.max(124.0 * amax / amax_row);
            cb.find_best_match(amax / scale_0, xb, weight, &mut best_idx[0..4]);
            let (dp, score_p) = cb.find_best_scale(xb, weight, &best_idx[0..4]);
            cb.find_best_match(-amax / scale_0, xb, weight, &mut best_idx[4..8]);
            let (dm, score_m) = cb.find_best_scale(xb, weight, &best_idx[4..8]);

            let first = if score_p > score_m {
                scales[ibl * 8 + ib] = dp;
                0
            } else {
                scales[ibl * 8 + ib] = dm;
                4
            };
            all_idx[off / 8..off / 8 + 4].copy_from_slice(&best_idx[first..first + 4]);

            let abs_scale = scales[ibl * 8 + ib].abs();
            if abs_scale > amax_scale {
                amax_scale = abs_scale;
                max_scale = scales[ibl * 8 + ib];
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
        for ib in 0..4 {
            let ls1 = best_index_iq4nl(id * scales[ibl * 8 + ib]);
            let ls2 = best_index_iq4nl(id * scales[ibl * 8 + ib + 4]);
            blocks[ibl * BYTES + ib] = (ls1 | (ls2 << 4)) as u8;
        }
    }

    let mut row_d = d;
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
            let ls = IQ4K_VALUES[scale_field(b, ib)] as i32;
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
                put_u16(b, QL + 2 * (ib * 4 + j), best_idx[j] as u16);
                let ql = cb.group(best_idx[j]);
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
        row_d = d;
    }
    KtRow { d: row_d, blocks }
}

/// Dequantize the way inference does (row scale times 1.05).
pub fn dequantize(row: &KtRow, y: &mut [f32]) {
    let mut v = [0.0f32; 8];
    for (ibl, b) in row.blocks.chunks(BYTES).enumerate() {
        for g in 0..32 {
            let dl = row.d * IQ4K_VALUES[scale_field(b, g / 4)] as f32 * RUNTIME_SCALE;
            gen_group(get_u16(b, QL + 2 * g) as u32, 4096, &mut v);
            for j in 0..8 {
                y[ibl * QK_K + 8 * g + j] = dl * v[j];
            }
        }
    }
}

pub fn block_scales(row: &KtRow, ibl: usize) -> Vec<f32> {
    let b = &row.blocks[ibl * BYTES..(ibl + 1) * BYTES];
    (0..8)
        .map(|ib| row.d * IQ4K_VALUES[scale_field(b, ib)] as f32 * RUNTIME_SCALE)
        .collect()
}

pub fn decode(row: &KtRow, i: usize) -> KtDecode {
    let b = &row.blocks[(i / QK_K) * BYTES..(i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let field = scale_field(b, j / 32);
    let d = row.d * RUNTIME_SCALE;
    let dl = row.d * IQ4K_VALUES[field] as f32 * RUNTIME_SCALE;
    let idx = get_u16(b, QL + 2 * (j / 8)) as u32;
    let lane = j % 8;
    let value = state_value_of(idx, 4096, lane);
    KtDecode {
        d,
        d_label: "d&middot;1.05",
        field: field as i32,
        dl,
        dl_expr: "iq4k[{s}]",
        idx,
        index_bits: 16,
        offset: 4096,
        lane,
        value,
        neg: None,
        math: format!(
            "{:.5}&middot;1.05 &times; {} &times; {}",
            row.d, IQ4K_VALUES[field], value
        ),
    }
}

fn state_value_of(idx: u32, offset: u32, lane: usize) -> f32 {
    codebook::state_value(codebook::lane_state(idx, offset, lane))
}

pub fn formula_html() -> String {
    super::formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">32b</span></span> &times; 1.05 &times; <span class=\"eq-pill\">iq4k[s]<span class=\"bits\">4b</span></span> &times; <span class=\"eq-pill\">gen(idx)<sub>j</sub><span class=\"bits\">16b</span></span>",
        "8-element groups, 16-bit computed-codebook seeds, 4-bit scale per 32.",
        "Each 16-bit index plus 4096 seeds an LCG, every lane is the sum of 4 random 6-bit bytes minus 126, roughly Gaussian. No stored codebook, no Viterbi path, every group index is picked independently. Search runs over 256 clusters of the 65536 points. Non-linear 4-bit scale per 32 (iq4k values), one FP32 scale per row. Inference multiplies the row scale by 1.05, dequantize_row_iq2_kt does not. 2.125 bpw plus 32 bits per row.",
    )
}
