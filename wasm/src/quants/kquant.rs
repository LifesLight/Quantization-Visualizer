use crate::math_utils::*;
use crate::quants::{InspectorData, KBlockMeta, KSuperMeta, QuantMeta, QuantizeOutput, Settings};

pub fn quantize(floats: &[f32], settings: &Settings) -> QuantizeOutput {
    let (weight_bits, sb_size, sub_size, sub_bits, has_offset) = (
        settings.weight_bits,
        settings.sb_size,
        settings.sub_size,
        settings.sub_bits,
        settings.has_offset,
    );
    let bpw = weight_bits as f32
        + ((if has_offset { 2 } else { 1 }) * sub_bits) as f32 / sub_size as f32
        + ((if has_offset { 2 } else { 1 }) as f32 * 16.0) / sb_size as f32;

    let mut q_floats = vec![0.0; floats.len()];
    let mut block_meta = Vec::new();
    let mut super_meta = Vec::new();

    for s in (0..floats.len()).step_by(sb_size) {
        let super_chunk_len = (floats.len() - s).min(sb_size);
        let super_chunk = &floats[s..s + super_chunk_len];

        let (mut sub_scales, mut sub_mins, mut q_sub_scales, mut q_sub_mins) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut super_chunk_q = vec![0.0; super_chunk_len];
        let super_scale;
        let mut super_min_scale = 1e-5_f32;

        if has_offset {
            let qmax_weight = (1 << weight_bits) as f32 - 1.0;
            let qmax_sub = (1 << sub_bits) as f32 - 1.0;
            for i in (0..super_chunk_len).step_by(sub_size) {
                let chunk = &super_chunk[i..(i + sub_size).min(super_chunk_len)];
                let (mut max, mut min) = (chunk[0], chunk[0].min(0.0));
                for &v in chunk {
                    if v > max {
                        max = v;
                    }
                    if v < min {
                        min = v;
                    }
                }
                sub_scales.push(if (max - min) / qmax_weight == 0.0 {
                    1e-5
                } else {
                    (max - min) / qmax_weight
                });
                sub_mins.push(min);
            }
            super_scale =
                fp16(sub_scales.iter().cloned().fold(0.0_f32, f32::max) / qmax_sub).max(1e-5);
            super_min_scale =
                fp16(sub_mins.iter().map(|&x| -x).fold(0.0_f32, f32::max) / qmax_sub).max(1e-5);

            for k in 0..sub_scales.len() {
                q_sub_scales.push(
                    ((sub_scales[k] / super_scale).round())
                        .max(0.0)
                        .min(qmax_sub)
                        * super_scale,
                );
                q_sub_mins.push(
                    ((-sub_mins[k] / super_min_scale).round())
                        .max(0.0)
                        .min(qmax_sub)
                        * super_min_scale,
                );
            }
            for i in 0..super_chunk_len {
                let sub_idx = i / sub_size;
                let qs = if q_sub_scales[sub_idx] == 0.0 {
                    1e-5
                } else {
                    q_sub_scales[sub_idx]
                };
                let q = ((super_chunk[i] + q_sub_mins[sub_idx]) / qs)
                    .round()
                    .max(0.0)
                    .min(qmax_weight);
                super_chunk_q[i] = q * q_sub_scales[sub_idx] - q_sub_mins[sub_idx];
                q_floats[s + i] = super_chunk_q[i];
            }
        } else {
            let max_q = 2.0_f32.powi(weight_bits as i32 - 1);
            let qmax_sub = (1 << sub_bits) as f32 - 1.0;
            for i in (0..super_chunk_len).step_by(sub_size) {
                let chunk = &super_chunk[i..(i + sub_size).min(super_chunk_len)];
                let mut max_abs = 0.0_f32;
                for &v in chunk {
                    if v.abs() > max_abs {
                        max_abs = v.abs();
                    }
                }
                sub_scales.push(if weight_bits == 1 {
                    max_abs.max(1e-5)
                } else {
                    (max_abs / (max_q - 1.0)).max(1e-5)
                });
            }
            super_scale =
                fp16(sub_scales.iter().cloned().fold(0.0_f32, f32::max) / qmax_sub).max(1e-5);
            for k in 0..sub_scales.len() {
                q_sub_scales.push(
                    ((sub_scales[k] / super_scale).round())
                        .max(0.0)
                        .min(qmax_sub)
                        * super_scale,
                );
            }

            for i in 0..super_chunk_len {
                let sub_idx = i / sub_size;
                let qs = if q_sub_scales[sub_idx] == 0.0 {
                    1e-5
                } else {
                    q_sub_scales[sub_idx]
                };
                if weight_bits == 1 {
                    super_chunk_q[i] = if super_chunk[i] >= 0.0 { 1.0 } else { -1.0 } * qs;
                } else {
                    let q = (super_chunk[i] / qs + max_q)
                        .round()
                        .max(0.0)
                        .min((max_q * 2.0) - 1.0);
                    super_chunk_q[i] = (q - max_q) * qs;
                }
                q_floats[s + i] = super_chunk_q[i];
            }
        }

        let (smse, smae) = get_err_stats(super_chunk, &super_chunk_q);
        super_meta.push(KSuperMeta {
            idx: s / sb_size,
            size: super_chunk_len,
            super_scale,
            super_min_scale: if has_offset { super_min_scale } else { 0.0 },
            mse: smse,
            mae: smae,
        });

        for i in (0..super_chunk_len).step_by(sub_size) {
            let chunk_len = (super_chunk_len - i).min(sub_size);
            let (mse, mae) = get_err_stats(
                &super_chunk[i..i + chunk_len],
                &super_chunk_q[i..i + chunk_len],
            );
            let sub_idx = i / sub_size;
            block_meta.push(KBlockMeta {
                idx: block_meta.len(),
                sb_idx: s / sb_size,
                size: chunk_len,
                q_scale: q_sub_scales[sub_idx],
                q_min: if has_offset { q_sub_mins[sub_idx] } else { 0.0 },
                mse,
                mae,
            });
        }
    }

    QuantizeOutput {
        q_floats,
        t_floats: None,
        t_q_floats: None,
        bpw,
        formula_html: if has_offset {
            format!("<span>Weight = <span class=\"eq-pill\">Q_Weight<span class=\"bits\">{}b</span></span> &times; ( <span class=\"eq-pill\">SubScale_Int<span class=\"bits\">{}b</span></span> &times; <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> ) - ( <span class=\"eq-pill\">SubMin_Int<span class=\"bits\">{}b</span></span> &times; <span class=\"eq-pill\">SuperMinScale<span class=\"bits\">16b</span></span> )</span>", weight_bits, sub_bits, sub_bits)
        } else {
            format!("<span>Weight = <span class=\"eq-pill\">Q_Weight<span class=\"bits\">{}b</span></span> &times; ( <span class=\"eq-pill\">SubScale_Int<span class=\"bits\">{}b</span></span> &times; <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> )</span>", weight_bits, sub_bits)
        },
        block_size: sub_size,
        super_block_size: sb_size,
        meta: QuantMeta::KQuant(block_meta, super_meta),
    }
}

pub fn format_inspector(
    idx: usize,
    active: &[f32],
    _out: &QuantizeOutput,
    blocks: &[KBlockMeta],
    supers: &[KSuperMeta],
    settings: &Settings,
) -> InspectorData {
    let (b_idx, s_idx) = (idx / settings.sub_size, idx / settings.sb_size);
    let mut math_str = String::new();
    if let (Some(bm), Some(sm)) = (blocks.get(b_idx), supers.get(s_idx)) {
        let v = active[idx];
        if settings.has_offset {
            let qs = if bm.q_scale == 0.0 { 1e-5 } else { bm.q_scale };
            let qmax_weight = (1 << settings.weight_bits) as f32 - 1.0;
            let q = ((v + bm.q_min) / qs).round().max(0.0).min(qmax_weight);
            math_str = format!(
                "{} &times; ({} &times; {:.4}) - ({} &times; {:.4})",
                q,
                (bm.q_scale / sm.super_scale).round(),
                sm.super_scale,
                (bm.q_min / sm.super_min_scale).round(),
                sm.super_min_scale
            );
        } else {
            if settings.weight_bits == 1 {
                math_str = format!(
                    "{} &times; ({} &times; {:.4})",
                    if v >= 0.0 { 1 } else { -1 },
                    (bm.q_scale / sm.super_scale).round(),
                    sm.super_scale
                );
            } else {
                let max_q = 2.0_f32.powi(settings.weight_bits as i32 - 1);
                let qs = if bm.q_scale == 0.0 { 1e-5 } else { bm.q_scale };
                let q = (v / qs + max_q).round().max(0.0).min((max_q * 2.0) - 1.0);
                math_str = format!(
                    "{} &times; ({} &times; {:.4})",
                    q - max_q,
                    (bm.q_scale / sm.super_scale).round(),
                    sm.super_scale
                );
            }
        }
    }

    InspectorData {
        math_str,
        block_html: blocks.get(b_idx).map(|bm| format!("<div class=\"data-row\"><span>MSE:</span> <span class=\"val-hl\">{:.6}</span></div><div class=\"data-row\"><span>MAE:</span> <span>{:.6}</span></div><div class=\"data-row\" style=\"margin-top:4px\"><span>SubScale (derived):</span> <span>{:.5}</span></div>{}", bm.mse, bm.mae, bm.q_scale, if settings.has_offset { format!("<div class=\"data-row\"><span>SubMin (derived):</span> <span>{:.5}</span></div>", bm.q_min) } else { "".to_string() })).unwrap_or_default(),
        block_idx_str: format!("[{}]", b_idx),
        super_html: supers.get(s_idx).map(|sm| format!("<div class=\"data-row\"><span>Super MSE:</span> <span class=\"val-hl\">{:.6}</span></div><div class=\"data-row\"><span>Super MAE:</span> <span>{:.6}</span></div><div class=\"data-row\" style=\"margin-top:4px\"><span>SuperScale (FP16):</span> <span>{:.6}</span></div>{}", sm.mse, sm.mae, sm.super_scale, if settings.has_offset { format!("<div class=\"data-row\"><span>SuperMinScale (FP16):</span> <span>{:.6}</span></div>", sm.super_min_scale) } else { "".to_string() })).unwrap_or_default(),
        super_idx_str: format!("[{}]", s_idx),
    }
}
