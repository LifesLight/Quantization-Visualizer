use crate::math_utils::*;
use crate::quants::{InspectorData, MxfpBlockMeta, QuantMeta, QuantizeOutput, Settings};

pub fn quantize(floats: &[f32], settings: &Settings) -> QuantizeOutput {
    let (bits, max_fmt, is_fp8, cb) = match settings.mxfp_format.as_str() {
        "mxfp6_e2m3" => {
            let mut c = vec![];
            for e in 0..4 {
                for m in 0..8 {
                    c.push(if e == 0 {
                        m as f32 / 8.0
                    } else {
                        2.0_f32.powi(e - 1) * (1.0 + m as f32 / 8.0)
                    });
                }
            }
            (6, 7.5, false, c)
        }
        "mxfp6_e3m2" => {
            let mut c = vec![];
            for e in 0..8 {
                for m in 0..4 {
                    c.push(if e == 0 {
                        2.0_f32.powi(-2) * (m as f32 / 4.0)
                    } else {
                        2.0_f32.powi(e - 3) * (1.0 + m as f32 / 4.0)
                    });
                }
            }
            (6, 28.0, false, c)
        }
        "mxfp8_e4m3" => (8, 448.0, true, vec![]),
        "mxfp8_e5m2" => (8, 57344.0, true, vec![]),
        _ => (4, 6.0, false, vec![0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0]),
    };

    let block_size = 32;
    let bpw = bits as f32 + (8.0 / block_size as f32);
    let mut q_floats = vec![0.0; floats.len()];
    let mut blocks = Vec::new();

    for i in (0..floats.len()).step_by(block_size) {
        let chunk_len = (floats.len() - i).min(block_size);
        let chunk = &floats[i..i + chunk_len];
        let mut max_abs = 0.0_f32;
        for &v in chunk {
            if v.abs() > max_abs {
                max_abs = v.abs();
            }
        }

        let mut e = -127;
        if max_abs > 0.0 {
            e = ((max_abs / max_fmt).log2().ceil() as i32).clamp(-127, 127);
        }
        let s = 2.0_f32.powi(e);

        let mut chunk_q = vec![0.0; chunk_len];
        for j in 0..chunk_len {
            let scaled = chunk[j] / s;
            let q_val = if is_fp8 {
                if settings.mxfp_format == "mxfp8_e5m2" {
                    fp8_e5m2(scaled)
                } else {
                    fp8_e4m3(scaled)
                }
            } else {
                snap_to_codebook(scaled, &cb)
            };
            chunk_q[j] = q_val * s;
            q_floats[i + j] = chunk_q[j];
        }

        let (mse, mae) = get_err_stats(chunk, &chunk_q);
        blocks.push(MxfpBlockMeta {
            idx: i / block_size,
            size: chunk_len,
            scale_e: e,
            scale: s,
            mse,
            mae,
        });
    }

    QuantizeOutput {
        q_floats, t_floats: None, t_q_floats: None, bpw,
        formula_html: format!("<span>Weight = <span class=\"eq-pill\">Micro_Value<span class=\"bits\">{}b</span></span> &times; <span class=\"eq-pill\" title=\"8-Bit E8M0 scaling factor\">2^E<span class=\"bits\">8b</span></span></span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">OCP {}: Every 32 weights share one block scale.</span>", bits, settings.mxfp_format.to_uppercase().replace('_', " ")),
        block_size, super_block_size: 0, meta: QuantMeta::Mxfp(blocks),
    }
}

pub fn format_inspector(
    idx: usize,
    active: &[f32],
    _out: &QuantizeOutput,
    blocks: &[MxfpBlockMeta],
    _settings: &Settings,
) -> InspectorData {
    let b_idx = idx / 32;
    let mut math_str = String::new();
    if let Some(bm) = blocks.get(b_idx) {
        let q_val = active[idx] / bm.scale;
        let s_disp = if q_val < 0.0 || active[idx] < 0.0 {
            "-"
        } else {
            "+"
        };
        math_str = format!(
            "{}{:.4} &times; 2<sup>{}</sup>",
            s_disp,
            q_val.abs(),
            bm.scale_e
        );
    }
    InspectorData {
        math_str,
        block_html: blocks.get(b_idx).map(|bm| format!("<div class=\"data-row\"><span>MSE:</span> <span class=\"val-hl\">{:.6}</span></div><div class=\"data-row\"><span>MAE:</span> <span>{:.6}</span></div><div class=\"data-row\" style=\"margin-top:4px\"><span>Block Scale (E8M0):</span> <span>2<sup>{}</sup> ({:.2e})</span></div>", bm.mse, bm.mae, bm.scale_e, bm.scale)).unwrap_or_default(),
        block_idx_str: format!("[{}]", b_idx),
        super_html: "".into(), super_idx_str: "".into(),
    }
}
