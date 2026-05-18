use crate::math_utils::*;
use crate::quants::{InspectorData, Nvfp4BlockMeta, QuantMeta, QuantizeOutput, Settings};

pub fn quantize(floats: &[f32], _settings: &Settings) -> QuantizeOutput {
    let block_size = 16;
    let bpw = 4.0 + (8.0 / 16.0) + (32.0 / floats.len().max(1) as f32);
    let mut q_floats = vec![0.0; floats.len()];
    let mut blocks = Vec::new();
    let cb = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

    let mut max_abs_global = 0.0_f32;
    for &v in floats {
        if v.abs() > max_abs_global {
            max_abs_global = v.abs();
        }
    }
    let global_scale = fp32(if max_abs_global > 0.0 {
        max_abs_global / 6.0
    } else {
        1.0
    });

    for i in (0..floats.len()).step_by(block_size) {
        let chunk_len = (floats.len() - i).min(block_size);
        let chunk = &floats[i..i + chunk_len];

        let mut max_abs_norm = 0.0_f32;
        for &v in chunk {
            if (v / global_scale).abs() > max_abs_norm {
                max_abs_norm = (v / global_scale).abs();
            }
        }

        let mut scale_e4m3 = fp8_e4m3(max_abs_norm / 6.0);
        if scale_e4m3 == 0.0 {
            scale_e4m3 = 0.001953;
        }

        let mut chunk_q = vec![0.0; chunk_len];
        for j in 0..chunk_len {
            let norm_val = (chunk[j] / global_scale) / scale_e4m3;
            let sign = if norm_val >= 0.0 { 1.0 } else { -1.0 };
            let best = snap_to_codebook(norm_val.abs(), &cb);
            let q_val = sign * best * scale_e4m3 * global_scale;
            chunk_q[j] = q_val;
            q_floats[i + j] = q_val;
        }

        let (mse, mae) = get_err_stats(chunk, &chunk_q);
        blocks.push(Nvfp4BlockMeta {
            idx: i / block_size,
            size: chunk_len,
            scale: scale_e4m3,
            mse,
            mae,
        });
    }

    QuantizeOutput {
        q_floats, t_floats: None, t_q_floats: None, bpw,
        formula_html: "<span>Weight = <span class=\"eq-pill\">Global_FP32<span class=\"bits\">32b</span></span> &times; ( <span class=\"eq-pill\">NVFP4_Value<span class=\"bits\">4b</span></span> &times; <span class=\"eq-pill\">FP8_Scale<span class=\"bits\">8b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">NVFP4: 16 weights share one FP8 scale. All weights share one global FP32 scale.</span>".into(),
        block_size, super_block_size: floats.len(), meta: QuantMeta::Nvfp4(blocks, global_scale, 0.0, 0.0), // global err populated in backend
    }
}

pub fn format_inspector(
    idx: usize,
    active: &[f32],
    _out: &QuantizeOutput,
    blocks: &[Nvfp4BlockMeta],
    gs: f32,
    gmse: f64,
    gmae: f64,
    _settings: &Settings,
) -> InspectorData {
    let b_idx = idx / 16;
    let mut math_str = String::new();
    if let Some(bm) = blocks.get(b_idx) {
        let norm_val = (active[idx] / gs) / bm.scale;
        let sign = if norm_val >= 0.0 { 1.0 } else { -1.0 };
        let cb = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];
        let best = snap_to_codebook(norm_val.abs(), &cb);
        let s_disp = if best == 0.0 || sign >= 0.0 { "+" } else { "-" };
        math_str = format!(
            "{}{:.1} &times; {:.4} &times; {:.4}",
            s_disp, best, bm.scale, gs
        );
    }

    InspectorData {
        math_str,
        block_html: blocks.get(b_idx).map(|bm| format!("<div class=\"data-row\"><span>MSE:</span> <span class=\"val-hl\">{:.6}</span></div><div class=\"data-row\"><span>MAE:</span> <span>{:.6}</span></div><div class=\"data-row\" style=\"margin-top:4px\"><span>Block Scale (FP8):</span> <span>{:.5}</span></div>", bm.mse, bm.mae, bm.scale)).unwrap_or_default(),
        block_idx_str: format!("[{}]", b_idx),
        super_html: format!("<div class=\"data-row\"><span>Global MSE:</span> <span class=\"val-hl\">{:.6}</span></div><div class=\"data-row\"><span>Global MAE:</span> <span>{:.6}</span></div><div class=\"data-row\" style=\"margin-top:4px\"><span>Global Scale (FP32):</span> <span>{:.6}</span></div>", gmse, gmae, gs),
        super_idx_str: "[Global]".into(),
    }
}
