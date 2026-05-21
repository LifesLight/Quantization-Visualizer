use crate::math_utils::*;
use crate::quants::{
    ImportanceResult, InspectorData, KBlockMeta, KSuperMeta, QuantMeta, QuantizeOutput, Settings,
};

/// Replicates the GGUF `K-Quant` hierarchical (super-block & sub-block) scale aggregation logic.
pub fn quantize(
    floats: &[f32],
    importance: Option<&ImportanceResult>,
    settings: &Settings,
) -> QuantizeOutput {
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
                let mut best_scale = if (max - min) / qmax_weight == 0.0 {
                    1e-5
                } else {
                    (max - min) / qmax_weight
                };
                let mut best_min = min;

                // local subblock refinement using element-level importance weights
                if let Some(imp) = importance {
                    let mut min_err = f64::INFINITY;
                    let range = max - min;
                    for k_s in 0..=10 {
                        let factor_s = 0.5 + 0.1 * (k_s as f32);
                        let s_cand = (best_scale * factor_s).max(1e-5);
                        for j_m in 0..=10 {
                            let factor_m = -0.2 + 0.04 * (j_m as f32);
                            let m_cand = (best_min + range * factor_m).min(0.0);

                            let mut err = 0.0_f64;
                            for (l, &v) in chunk.iter().enumerate() {
                                let w = imp.raw[s + i + l] as f64;
                                let q =
                                    ((v - m_cand) / s_cand).round().clamp(0.0, qmax_weight) as f64;
                                let diff = (v as f64) - (q * (s_cand as f64) + (m_cand as f64));
                                err += w * diff * diff;
                            }
                            if err < min_err {
                                min_err = err;
                                best_scale = s_cand;
                                best_min = m_cand;
                            }
                        }
                    }
                }

                sub_scales.push(best_scale);
                sub_mins.push(best_min);
            }

            // aggregate subblock weights
            let mut sub_weights = None;
            if let Some(imp) = importance {
                let mut w_list = Vec::new();
                for i in (0..super_chunk_len).step_by(sub_size) {
                    let chunk_len = (super_chunk_len - i).min(sub_size);
                    let mut w_sum = 0.0_f32;
                    for l in 0..chunk_len {
                        w_sum += imp.raw[s + i + l];
                    }
                    w_list.push(w_sum.max(1e-5));
                }
                sub_weights = Some(w_list);
            }

            // search for optimal super_scale
            let mut best_super_scale =
                fp16(sub_scales.iter().cloned().fold(0.0_f32, f32::max) / qmax_sub).max(1e-5);
            if let Some(ref w) = sub_weights {
                let mut min_err = f64::INFINITY;
                let base_sc = best_super_scale;
                for k_sc in 0..=50 {
                    let factor = 0.5 + 0.02 * (k_sc as f32);
                    let sc_cand = fp16(base_sc * factor).max(1e-5);
                    let mut err = 0.0_f64;
                    for (j, &s_val) in sub_scales.iter().enumerate() {
                        let q = ((s_val / sc_cand).round()).clamp(0.0, qmax_sub);
                        let diff = (s_val - q * sc_cand) as f64;
                        err += (w[j] as f64) * diff * diff;
                    }
                    if err < min_err {
                        min_err = err;
                        best_super_scale = sc_cand;
                    }
                }
            }
            super_scale = best_super_scale;

            // search for optimal super_min_scale
            let mut best_super_min_scale =
                fp16(sub_mins.iter().map(|&x| -x).fold(0.0_f32, f32::max) / qmax_sub).max(1e-5);
            if let Some(ref w) = sub_weights {
                let mut min_err = f64::INFINITY;
                let base_sm = best_super_min_scale;
                for k_sm in 0..=50 {
                    let factor = 0.5 + 0.02 * (k_sm as f32);
                    let sm_cand = fp16(base_sm * factor).max(1e-5);
                    let mut err = 0.0_f64;
                    for (j, &m_val) in sub_mins.iter().enumerate() {
                        let q = (((-m_val) / sm_cand).round()).clamp(0.0, qmax_sub);
                        let diff = ((-m_val) - q * sm_cand) as f64;
                        err += (w[j] as f64) * diff * diff;
                    }
                    if err < min_err {
                        min_err = err;
                        best_super_min_scale = sm_cand;
                    }
                }
            }
            super_min_scale = best_super_min_scale;

            for k in 0..sub_scales.len() {
                q_sub_scales.push(
                    ((sub_scales[k] / super_scale).round()).clamp(0.0, qmax_sub) * super_scale,
                );
                q_sub_mins.push(
                    ((-sub_mins[k] / super_min_scale).round()).clamp(0.0, qmax_sub)
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
                    .clamp(0.0, qmax_weight);
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
                let mut best_scale = if weight_bits == 1 {
                    max_abs.max(1e-5)
                } else {
                    (max_abs / (max_q - 1.0)).max(1e-5)
                };

                // local subblock refinement using element-level importance weights
                if let Some(imp) = importance {
                    let mut min_err = f64::INFINITY;
                    for k_s in 0..=10 {
                        let factor_s = 0.5 + 0.1 * (k_s as f32);
                        let s_cand = (best_scale * factor_s).max(1e-5);

                        let mut err = 0.0_f64;
                        for (l, &v) in chunk.iter().enumerate() {
                            let w = imp.raw[s + i + l] as f64;
                            let diff = if weight_bits == 1 {
                                let q = if v >= 0.0 { 1.0 } else { -1.0 };
                                (v as f64) - (q * (s_cand as f64))
                            } else {
                                let q =
                                    (v / s_cand + max_q).round().clamp(0.0, (max_q * 2.0) - 1.0);
                                (v as f64) - ((q - max_q) as f64 * (s_cand as f64))
                            };
                            err += w * diff * diff;
                        }
                        if err < min_err {
                            min_err = err;
                            best_scale = s_cand;
                        }
                    }
                }

                sub_scales.push(best_scale);
            }

            // aggregate subblock weights
            let mut sub_weights = None;
            if let Some(imp) = importance {
                let mut w_list = Vec::new();
                for i in (0..super_chunk_len).step_by(sub_size) {
                    let chunk_len = (super_chunk_len - i).min(sub_size);
                    let mut w_sum = 0.0_f32;
                    for l in 0..chunk_len {
                        w_sum += imp.raw[s + i + l];
                    }
                    w_list.push(w_sum.max(1e-5));
                }
                sub_weights = Some(w_list);
            }

            // search for optimal super_scale
            let mut best_super_scale =
                fp16(sub_scales.iter().cloned().fold(0.0_f32, f32::max) / qmax_sub).max(1e-5);
            if let Some(ref w) = sub_weights {
                let mut min_err = f64::INFINITY;
                let base_sc = best_super_scale;
                for k_sc in 0..=50 {
                    let factor = 0.5 + 0.02 * (k_sc as f32);
                    let sc_cand = fp16(base_sc * factor).max(1e-5);
                    let mut err = 0.0_f64;
                    for (j, &s_val) in sub_scales.iter().enumerate() {
                        let q = ((s_val / sc_cand).round()).clamp(0.0, qmax_sub);
                        let diff = (s_val - q * sc_cand) as f64;
                        err += (w[j] as f64) * diff * diff;
                    }
                    if err < min_err {
                        min_err = err;
                        best_super_scale = sc_cand;
                    }
                }
            }
            super_scale = best_super_scale;

            for k in 0..sub_scales.len() {
                q_sub_scales.push(
                    ((sub_scales[k] / super_scale).round()).clamp(0.0, qmax_sub) * super_scale,
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
                        .clamp(0.0, (max_q * 2.0) - 1.0);
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
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::KQuant(blocks, supers) = &out.meta else {
        return InspectorData::default();
    };
    let (b_idx, s_idx) = (idx / settings.sub_size, idx / settings.sb_size);
    let mut data = InspectorData::default();

    if let (Some(bm), Some(sm)) = (blocks.get(b_idx), supers.get(s_idx)) {
        let v = active[idx];
        if settings.has_offset {
            let qs = if bm.q_scale == 0.0 { 1e-5 } else { bm.q_scale };
            let qmax_weight = (1 << settings.weight_bits) as f32 - 1.0;
            let q = ((v + bm.q_min) / qs).round().clamp(0.0, qmax_weight);
            data.math_str = Some(format!(
                "{} &times; ({} &times; {:.4}) - ({} &times; {:.4})",
                q,
                (bm.q_scale / sm.super_scale).round(),
                sm.super_scale,
                (bm.q_min / sm.super_min_scale).round(),
                sm.super_min_scale
            ));
        } else {
            if settings.weight_bits == 1 {
                data.math_str = Some(format!(
                    "{} &times; ({} &times; {:.4})",
                    if v >= 0.0 { 1 } else { -1 },
                    (bm.q_scale / sm.super_scale).round(),
                    sm.super_scale
                ));
            } else {
                let max_q = 2.0_f32.powi(settings.weight_bits as i32 - 1);
                let qs = if bm.q_scale == 0.0 { 1e-5 } else { bm.q_scale };
                let q = (v / qs + max_q).round().clamp(0.0, (max_q * 2.0) - 1.0);
                data.math_str = Some(format!(
                    "{} &times; ({} &times; {:.4})",
                    q - max_q,
                    (bm.q_scale / sm.super_scale).round(),
                    sm.super_scale
                ));
            }
        }
        data.block_idx = Some(b_idx);
        data.mse = Some(bm.mse);
        data.mae = Some(bm.mae);
        data.scale = Some(bm.q_scale);
        if settings.has_offset {
            data.min = Some(bm.q_min);
        }
        data.super_idx = Some(s_idx);
        data.super_scale = Some(sm.super_scale);
        if settings.has_offset {
            data.super_min = Some(sm.super_min_scale);
        }
        data.super_mse = Some(sm.mse);
        data.super_mae = Some(sm.mae);
    }
    data
}
