//! IQ3_KT: f32 row scale, then block_iq3_kt { u8 scales[4]; u16 ql[32]; u8 qh[32] } -> 100 bytes.
//! The codebook holds absolute values, the sign of every weight is a bit in qh (bit ib of
//! qh[j] for weight j of block ib). Blocks of 32 have a plain 4 bit scale.

use super::codebook::{self, KtCodebook};
use super::common::*;
use super::KtType;

pub static TYPE: KtType = KtType {
    name: "IQ3_KT",
    group_size: 8,
    bytes: 100,
    copy_pad: false,
    quantize,
    dequantize,
    decode,
    block_scales,
    formula_html,
};

const BYTES: usize = 100;
const QL: usize = 4;
const QH: usize = 68;
/// dequantize_row_iq3_kt and the inference kernels both scale the row by 1.01.
const RUNTIME_SCALE: f32 = 1.01;

fn scale_field(b: &[u8], ib: usize) -> usize {
    ((b[ib % 4] >> (4 * (ib / 4))) & 0xf) as usize
}

/// Port of quantize_row_iq3_kt_impl for one (padded) row.
pub fn quantize(x: &[f32], n_per_row: usize, qw: Option<&[f32]>) -> KtRow {
    const STEP: f32 = 8.0;
    let cb: &KtCodebook<8> = codebook::iq3kt();
    let nblock = x.len() / QK_K;
    let mut blocks = vec![0u8; nblock * BYTES];

    let amax_row = max_abs(&x[..n_per_row]);
    if amax_row == 0.0 {
        return KtRow { d: 0.0, blocks };
    }

    let mut weights = vec![0.0f32; nblock * QK_K];
    let mut scales = vec![0.0f32; nblock * 8];
    let mut qtmp = vec![0.0f32; nblock * QK_K];
    let mut best_idx = [0u32; 8];
    let mut xaux = [0.0f32; 32];

    set_weights(2.0, x, n_per_row, qw, &mut weights);

    let mut amax_scale = 0.0f32;
    let mut max_scale = 0.0f32;
    for ibl in 0..nblock {
        for ib in 0..8 {
            let off = ibl * QK_K + ib * 32;
            let weight = &weights[off..off + 32];
            let mut amax = 0.0f32;
            for j in 0..32 {
                let ax = x[off + j].abs();
                xaux[j] = ax;
                if amax < ax {
                    amax = ax;
                }
            }
            if amax < 1e-16 {
                scales[ibl * 8 + ib] = 0.0;
                continue;
            }
            let scale_0 = 84.0f32.max(123.0 * amax / amax_row);
            let mut best = 0.0f32;
            for itry in -3..=3 {
                cb.find_best_match(
                    amax / (scale_0 + STEP * itry as f32),
                    &xaux,
                    weight,
                    &mut best_idx[0..4],
                );
                let (d, score) = cb.find_best_scale(&xaux, weight, &best_idx[0..4]);
                if score > best {
                    best = score;
                    scales[ibl * 8 + ib] = d;
                    best_idx.copy_within(0..4, 4);
                }
            }
            let xt = &mut qtmp[off..off + 32];
            for ig in 0..4 {
                xt[8 * ig..8 * ig + 8].copy_from_slice(cb.group(best_idx[4 + ig]));
            }
            let abs_scale = scales[ibl * 8 + ib].abs();
            if abs_scale > amax_scale {
                amax_scale = abs_scale;
                max_scale = scales[ibl * 8 + ib];
            }
        }
    }

    let mut d = max_scale / 15.0;
    let mut best = 0.0f32;
    for itry in -9..=9 {
        let id = (itry as f32 * 0.2 + 15.0) / max_scale;
        let mut sumqx = 0.0f32;
        let mut sumq2 = 0.0f32;
        for ibl in 0..nblock {
            for ib in 0..8 {
                let off = ibl * QK_K + ib * 32;
                let ls = nearest_int(id * scales[ibl * 8 + ib]).clamp(0, 15);
                let dl = ls as f32;
                for j in 0..32 {
                    let q = dl * qtmp[off + j];
                    sumqx += weights[off + j] * x[off + j].abs() * q;
                    sumq2 += weights[off + j] * q * q;
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
            let ls1 = nearest_int(id * scales[ibl * 8 + ib]).clamp(0, 15);
            let ls2 = nearest_int(id * scales[ibl * 8 + ib + 4]).clamp(0, 15);
            blocks[ibl * BYTES + ib] = (ls1 | (ls2 << 4)) as u8;
        }
    }

    let mut row_d = d;
    let mut sumqx = 0.0f32;
    let mut sumq2 = 0.0f32;
    for ibl in 0..nblock {
        let b = &mut blocks[ibl * BYTES..(ibl + 1) * BYTES];
        b[QH..QH + 16].fill(0);
        for ib in 0..8 {
            let off = ibl * QK_K + ib * 32;
            let weight = &weights[off..off + 32];
            for j in 0..32 {
                xaux[j] = x[off + j].abs();
                if x[off + j] < 0.0 {
                    b[QH + j] |= 1 << ib;
                }
            }
            let ls = scale_field(b, ib) as i32;
            let dl = d * ls as f32;
            cb.find_best_match(dl, &xaux, weight, &mut best_idx[0..4]);
            for j in 0..4 {
                put_u16(b, QL + 2 * (ib * 4 + j), best_idx[j] as u16);
                let ql = cb.group(best_idx[j]);
                for k in 0..8 {
                    let q = ql[k] * ls as f32;
                    sumqx += weight[8 * j + k] * xaux[8 * j + k] * q;
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

/// Port of dequantize_row_iq3_kt.
pub fn dequantize(row: &KtRow, y: &mut [f32]) {
    let d = row.d * RUNTIME_SCALE;
    let mut v = [0.0f32; 8];
    for (ibl, b) in row.blocks.chunks(BYTES).enumerate() {
        for ib in 0..8 {
            let sl = d * scale_field(b, ib) as f32;
            for ig in 0..4 {
                gen_group(get_u16(b, QL + 2 * (ib * 4 + ig)) as u32, 4096, &mut v);
                for j in 0..8 {
                    let pos = ig * 8 + j;
                    let val = sl * v[j].abs();
                    y[ibl * QK_K + ib * 32 + pos] = if b[QH + pos] & (1 << ib) != 0 {
                        -val
                    } else {
                        val
                    };
                }
            }
        }
    }
}

pub fn block_scales(row: &KtRow, ibl: usize) -> Vec<f32> {
    let b = &row.blocks[ibl * BYTES..(ibl + 1) * BYTES];
    let d = row.d * RUNTIME_SCALE;
    (0..8).map(|ib| d * scale_field(b, ib) as f32).collect()
}

pub fn decode(row: &KtRow, i: usize) -> KtDecode {
    let b = &row.blocks[(i / QK_K) * BYTES..(i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let ib = j / 32;
    let field = scale_field(b, ib);
    let d = row.d * RUNTIME_SCALE;
    let idx = get_u16(b, QL + 2 * (j / 8)) as u32;
    let lane = j % 8;
    let value = codebook::state_value(codebook::lane_state(idx, 4096, lane)).abs();
    let base = (j % 32) / 8 * 8;
    let mut neg = 0u8;
    for k in 0..8 {
        if b[QH + base + k] & (1 << ib) != 0 {
            neg |= 1 << k;
        }
    }
    KtDecode {
        d,
        d_label: "d&middot;1.01",
        field: field as i32,
        dl: d * field as f32,
        dl_expr: "{s}",
        idx,
        index_bits: 16,
        offset: 4096,
        lane,
        value,
        neg: Some(neg),
        math: format!(
            "{:.5}&middot;1.01 &times; {} &times; {} &times; {}1",
            row.d,
            field,
            value,
            if neg & (1 << lane) != 0 {
                "&minus;"
            } else {
                "+"
            }
        ),
    }
}

pub fn formula_html() -> String {
    super::formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">32b</span></span> &times; 1.01 &times; <span class=\"eq-pill\">s<span class=\"bits\">4b</span></span> &times; <span class=\"eq-pill\">|gen(idx)|<sub>j</sub><span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">&plusmn;<span class=\"bits\">1b</span></span>",
        "8-element groups, 16-bit seeds, separate signs, 4-bit scale per 32.",
        "Same LCG codebook as IQ2_KT but with absolute values, signs are stored separately for every weight. Search runs over 256 clusters split at the per-dimension median. Linear 4-bit scale per 32, one FP32 scale per row, times 1.01 at decode. 3.125 bpw plus 32 bits per row.",
    )
}
