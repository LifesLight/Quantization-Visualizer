use crate::math_utils::*;
use crate::quants::{AsymBlockMeta, InspectorData, QuantMeta, QuantizeOutput, Settings};

/// Applies asymmetrical block quantization (includes both scale and offset limits).
pub fn quantize(
    floats: &[f32],
    _importance: Option<&[f32]>,
    settings: &Settings,
) -> QuantizeOutput {
    let weight_bits = settings.weight_bits;
    let block_size = settings.block_size;
    let base_precision = 16.0;
    let bpw = weight_bits as f32 + (2.0 * base_precision / block_size as f32);

    let mut q_floats = vec![0.0; floats.len()];
    let mut blocks = Vec::new();

    for i in (0..floats.len()).step_by(block_size) {
        let chunk_len = (floats.len() - i).min(block_size);
        let chunk = &floats[i..i + chunk_len];

        let mut min = chunk[0];
        let mut max = chunk[0];
        for &v in chunk {
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
        }

        let mut scale = fp16((max - min) / ((1 << weight_bits) as f32 - 1.0));
        if scale == 0.0 {
            scale = 1e-5;
        }
        let offset = fp16(min);

        let mut chunk_q = vec![0.0; chunk_len];
        for j in 0..chunk_len {
            let q = ((chunk[j] - offset) / scale)
                .round()
                .clamp(0.0, (1 << weight_bits) as f32 - 1.0);
            let q_val = q * scale + offset;
            chunk_q[j] = q_val;
            q_floats[i + j] = q_val;
        }

        let (mse, mae) = get_err_stats(chunk, &chunk_q);
        blocks.push(AsymBlockMeta {
            idx: i / block_size,
            size: chunk_len,
            scale,
            min: offset,
            mse,
            mae,
        });
    }

    QuantizeOutput {
        q_floats, t_floats: None, t_q_floats: None, bpw,
        formula_html: format!("<span>Weight = <span class=\"eq-pill\">Q_Weight<span class=\"bits\">{}b</span></span> &times; <span class=\"eq-pill\">Scale<span class=\"bits\">16b</span></span> + <span class=\"eq-pill\">Min<span class=\"bits\">16b</span></span></span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Every {} weights share one FP16 scale and one FP16 offset.</span>", weight_bits, block_size),
        block_size, super_block_size: 0, imp_size: block_size, meta: QuantMeta::Asym(blocks),
    }
}

pub fn format_inspector(
    idx: usize,
    _active: &[f32],
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::Asym(blocks) = &out.meta else {
        return InspectorData::default();
    };
    let b_idx = idx / settings.block_size;
    let mut data = InspectorData::default();

    if let Some(bm) = blocks.get(b_idx) {
        let q = ((out.q_floats[idx] - bm.min) / bm.scale).round() as i32;
        data.math_str = Some(format!("{} &times; {:.4} + {:.4}", q, bm.scale, bm.min));
        data.block_idx = Some(bm.idx);
        data.mse = Some(bm.mse);
        data.mae = Some(bm.mae);
        data.scale = Some(bm.scale);
        data.min = Some(bm.min);
    }
    data
}
