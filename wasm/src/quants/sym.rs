use crate::math_utils::*;
use crate::quants::{InspectorData, QuantMeta, QuantizeOutput, Settings, SymBlockMeta};

/// Applies standard symmetrical block quantization (e.g., NF4-style or INT8).
pub fn quantize(
    floats: &[f32],
    _importance: Option<&[f32]>,
    settings: &Settings,
) -> QuantizeOutput {
    let weight_bits = settings.weight_bits;
    let block_size = settings.block_size;
    let base_precision = 16.0;
    let bpw = weight_bits as f32 + (base_precision / block_size as f32);

    let mut q_floats = vec![0.0; floats.len()];
    let mut blocks = Vec::new();

    for i in (0..floats.len()).step_by(block_size) {
        let chunk_len = (floats.len() - i).min(block_size);
        let chunk = &floats[i..i + chunk_len];

        let scale;
        let mut chunk_q = vec![0.0; chunk_len];

        if weight_bits == 1 {
            let mut max_abs = 0.0_f32;
            for &v in chunk {
                if v.abs() > max_abs {
                    max_abs = v.abs();
                }
            }
            scale = fp16(if max_abs == 0.0 { 1e-5 } else { max_abs });
            for j in 0..chunk_len {
                let q_val = if chunk[j] >= 0.0 { 1.0 } else { -1.0 } * scale;
                chunk_q[j] = q_val;
                q_floats[i + j] = q_val;
            }
        } else {
            let max_q = 2.0_f32.powi(weight_bits as i32 - 1);
            let mut max_val = chunk[0];
            for &v in chunk {
                if v.abs() > max_val.abs() {
                    max_val = v;
                }
            }
            scale = fp16(if (max_val / -max_q) == 0.0 {
                1e-5
            } else {
                max_val / -max_q
            });
            for j in 0..chunk_len {
                let q = ((chunk[j] / scale + max_q).round()).clamp(0.0, (max_q * 2.0) - 1.0);
                let q_val = (q - max_q) * scale;
                chunk_q[j] = q_val;
                q_floats[i + j] = q_val;
            }
        }

        let (mse, mae) = get_err_stats(chunk, &chunk_q);
        blocks.push(SymBlockMeta {
            idx: i / block_size,
            size: chunk_len,
            scale,
            mse,
            mae,
        });
    }

    QuantizeOutput {
        q_floats, t_floats: None, t_q_floats: None, bpw,
        formula_html: format!("<span>Weight = <span class=\"eq-pill\">Q_Weight<span class=\"bits\">{}b</span></span> &times; <span class=\"eq-pill\">Scale<span class=\"bits\">16b</span></span></span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Every {} weights share one FP16 scale.</span>", weight_bits, block_size),
        block_size, super_block_size: 0, imp_size: block_size, meta: QuantMeta::Sym(blocks),
    }
}

pub fn format_inspector(
    idx: usize,
    _active: &[f32],
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::Sym(blocks) = &out.meta else {
        return InspectorData::default();
    };
    let b_idx = idx / settings.block_size;
    let mut data = InspectorData::default();

    if let Some(bm) = blocks.get(b_idx) {
        let chunk_v = out.q_floats[idx];
        data.math_str = Some(if settings.weight_bits == 1 {
            let sign = if chunk_v >= 0.0 { 1 } else { -1 };
            format!("{} &times; {:.4}", sign, bm.scale)
        } else {
            format!(
                "{} &times; {:.4}",
                (chunk_v / bm.scale).round() as i32,
                bm.scale
            )
        });
        data.block_idx = Some(bm.idx);
        data.mse = Some(bm.mse);
        data.mae = Some(bm.mae);
        data.scale = Some(bm.scale);
    }
    data
}
