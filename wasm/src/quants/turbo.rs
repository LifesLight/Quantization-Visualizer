use crate::math_utils::*;
use crate::quants::{InspectorData, QuantMeta, QuantizeOutput, Settings, TurboBlockMeta};

/// Quantization technique combining Lloyd-Max, Optional FWHT, and Optional QJL Error correction.
pub fn quantize(floats: &[f32], settings: &Settings) -> QuantizeOutput {
    let (t_bits, t_bsize, use_wht, use_qjl, seed) = (
        settings.turbo_bits,
        settings.turbo_block_size,
        settings.use_wht,
        settings.use_qjl,
        settings.turbo_sign_seed,
    );
    let centroids = get_lloyd_max_centroids(t_bits, "normal");
    let bpw = t_bits as f32 + (if use_qjl { 1.0 } else { 0.0 }) + (16.0 / t_bsize as f32);

    let mut q_floats = vec![0.0; floats.len()];
    let mut t_floats = if use_wht {
        Some(vec![0.0; floats.len()])
    } else {
        None
    };
    let mut t_q_floats = if use_wht {
        Some(vec![0.0; floats.len()])
    } else {
        None
    };
    let mut blocks = Vec::new();

    // Pre-allocate working buffers for the hot loop
    let max_pad_len = t_bsize.next_power_of_two();
    let mut chunk_padded = vec![0.0; max_pad_len];
    let mut chunk_q = vec![0.0; max_pad_len];
    let mut residuals = vec![0.0; max_pad_len];
    let mut chunk_out = vec![0.0; max_pad_len];

    for i in (0..floats.len()).step_by(t_bsize) {
        let actual_len = (floats.len() - i).min(t_bsize);
        let chunk = &floats[i..i + actual_len];

        let mut pad_len = 1;
        while pad_len < actual_len {
            pad_len *= 2;
        }

        chunk_padded[..actual_len].copy_from_slice(chunk);
        chunk_padded[actual_len..pad_len].fill(0.0);

        if use_wht {
            for j in 0..pad_len {
                chunk_padded[j] *= get_sign_flip(i + j, seed);
            }
            fwht_f32_inplace(&mut chunk_padded[..pad_len]);
        }

        let sum_sq: f32 = chunk_padded[..pad_len].iter().map(|&x| x * x).sum();
        let rms = fp16(if sum_sq == 0.0 {
            1e-5
        } else {
            (sum_sq / pad_len as f32).sqrt()
        });

        for j in 0..pad_len {
            let best_c = snap_to_codebook(chunk_padded[j] / rms, &centroids);
            chunk_q[j] = best_c * rms;
            residuals[j] = chunk_padded[j] - chunk_q[j];
        }

        let mut mean_abs_res = 0.0;
        if use_qjl {
            let abs_res_sum: f32 = residuals[..pad_len].iter().map(|&x| x.abs()).sum();
            mean_abs_res = fp16(abs_res_sum / pad_len as f32);
            for j in 0..pad_len {
                chunk_q[j] += if residuals[j] >= 0.0 { 1.0 } else { -1.0 } * mean_abs_res;
            }
        }

        chunk_out[..pad_len].copy_from_slice(&chunk_q[..pad_len]);

        if use_wht {
            fwht_f32_inplace(&mut chunk_out[..pad_len]);
            for j in 0..pad_len {
                chunk_out[j] *= get_sign_flip(i + j, seed);
            }
        }

        for j in 0..actual_len {
            q_floats[i + j] = chunk_out[j];
            if use_wht {
                t_floats.as_mut().unwrap()[i + j] = chunk_padded[j];
                t_q_floats.as_mut().unwrap()[i + j] = chunk_q[j];
            }
        }

        let (mse, mae) = get_err_stats(chunk, &chunk_out[..actual_len]);
        blocks.push(TurboBlockMeta {
            idx: i / t_bsize,
            size: actual_len,
            scale: rms,
            qjl_scale: mean_abs_res,
            mse,
            mae,
        });
    }

    QuantizeOutput {
        q_floats, t_floats, t_q_floats, bpw,
        formula_html: format!("<span>Weight = {}[ ( <span class=\"eq-pill\">LloydMax<span class=\"bits\">{}b</span></span> &times; <span class=\"eq-pill\">RMS_Scale<span class=\"bits\">16b</span></span> ){} ]</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Every {} weights share one FP16 RMS scale. {}</span>", if use_wht { "<span class=\"eq-pill\" title=\"Diagonal Random Sign Array\">D</span> &times; <span class=\"eq-pill\" title=\"Orthogonal Fast Walsh-Hadamard Transform\">FWHT</span> &times; " } else { "" }, t_bits, if use_qjl { " + <span class=\"eq-pill\">QJL_1bit<span class=\"bits\">1b</span></span>" } else { "" }, t_bsize, if use_wht && use_qjl { "SRHT forces Gaussian distribution; QJL adds 1-bit bias correction." } else if use_wht { "SRHT rotates features into a Gaussian distribution." } else if use_qjl { "QJL adds 1-bit bias correction to raw values." } else { "Applying static Lloyd-Max to raw distribution." }),
        block_size: t_bsize, super_block_size: 0, meta: QuantMeta::Turbo(blocks),
    }
}

pub fn format_inspector(
    idx: usize,
    active: &[f32],
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::Turbo(blocks) = &out.meta else {
        return InspectorData::default();
    };
    let b_idx = idx / settings.turbo_block_size;
    let mut data = InspectorData::default();

    if let Some(bm) = blocks.get(b_idx) {
        let t_val = if settings.use_wht {
            out.t_floats.as_ref().unwrap()[idx]
        } else {
            active[idx]
        };
        let centroids = get_lloyd_max_centroids(settings.turbo_bits, "normal");
        let best_c = snap_to_codebook(t_val / bm.scale, &centroids);

        let inner = format!(
            "C({:.2}) &times; {:.2}{}",
            best_c,
            bm.scale,
            if settings.use_qjl {
                format!(
                    " {} QJL",
                    if t_val - best_c * bm.scale >= 0.0 {
                        "+"
                    } else {
                        "-"
                    }
                )
            } else {
                "".into()
            }
        );

        data.math_str = Some(if settings.use_wht {
            format!(
                "D({}) &times; FWHT( {} )[{}]",
                if get_sign_flip(idx, settings.turbo_sign_seed) > 0.0 {
                    "+1"
                } else {
                    "-1"
                },
                inner,
                idx % settings.turbo_block_size
            )
        } else {
            inner
        });

        data.block_idx = Some(b_idx);
        data.mse = Some(bm.mse);
        data.mae = Some(bm.mae);
        data.scale = Some(bm.scale);
        data.qjl_scale = Some(bm.qjl_scale);
    }
    data
}
