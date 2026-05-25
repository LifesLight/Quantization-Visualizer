pub mod algos;
pub mod lut; // From previous folder state, already implemented

use crate::math_utils::*;
use crate::quants::{
    ImportanceResult, InspectorData, IqBlockMeta, QuantMeta, QuantizeOutput, Settings,
};

pub fn quantize(
    floats: &[f32],
    importance: Option<&ImportanceResult>,
    settings: &Settings,
) -> QuantizeOutput {
    let n = floats.len();
    let mut q_floats = vec![0.0; n];
    let mut blocks = Vec::new();

    let (block_size, bpw, formula_html) = match settings.iq_type.as_str() {
        "iq1_s" => (
            algos::iq1_s::block_size(),
            algos::iq1_s::bpw(),
            algos::iq1_s::formula_html(),
        ),
        "iq1_m" => (
            algos::iq1_m::block_size(),
            algos::iq1_m::bpw(),
            algos::iq1_m::formula_html(),
        ),
        "iq2_xxs" => (
            algos::iq2_xxs::block_size(),
            algos::iq2_xxs::bpw(),
            algos::iq2_xxs::formula_html(),
        ),
        "iq2_xs" => (
            algos::iq2_xs::block_size(),
            algos::iq2_xs::bpw(),
            algos::iq2_xs::formula_html(),
        ),
        "iq2_s" => (
            algos::iq2_s::block_size(),
            algos::iq2_s::bpw(),
            algos::iq2_s::formula_html(),
        ),
        "iq3_xxs" => (
            algos::iq3_xxs::block_size(),
            algos::iq3_xxs::bpw(),
            algos::iq3_xxs::formula_html(),
        ),
        "iq3_s" => (
            algos::iq3_s::block_size(),
            algos::iq3_s::bpw(),
            algos::iq3_s::formula_html(),
        ),
        "iq4_xs" => (
            algos::iq4_xs::block_size(),
            algos::iq4_xs::bpw(),
            algos::iq4_xs::formula_html(),
        ),
        "iq4_nl" => (
            algos::iq4_nl::block_size(),
            algos::iq4_nl::bpw(),
            algos::iq4_nl::formula_html(),
        ),
        _ => (
            algos::iq2_xxs::block_size(),
            algos::iq2_xxs::bpw(),
            algos::iq2_xxs::formula_html(),
        ),
    };

    let iters = settings.iq_scale_iters as i32;
    let max_pad_len = block_size;
    let mut chunk_padded = vec![0.0; max_pad_len];
    let mut weights = vec![0.0; max_pad_len];

    for b in (0..n).step_by(block_size) {
        let chunk_len = (n - b).min(block_size);
        let chunk = &floats[b..b + chunk_len];

        chunk_padded[..chunk_len].copy_from_slice(chunk);
        chunk_padded[chunk_len..max_pad_len].fill(0.0);

        let mut sum_sq = 0.0;
        for &v in chunk {
            sum_sq += (v * v) as f64;
        }
        let variance = 2.0 * sum_sq / chunk_len.max(1) as f64;

        if let Some(imp) = importance {
            for i in 0..chunk_len {
                let m_i = imp.raw[b + i] as f64;
                weights[i] = (m_i * (variance + (chunk[i] * chunk[i]) as f64).sqrt()) as f32;
            }
        } else {
            for i in 0..chunk_len {
                weights[i] = chunk[i] * chunk[i];
            }
        }
        weights[chunk_len..max_pad_len].fill(0.0);

        let (q_block, block_scale, scales, aux8, grids, signs) = match settings.iq_type.as_str() {
            "iq1_s" => algos::iq1_s::quantize_block(&chunk_padded, &weights, iters),
            "iq1_m" => algos::iq1_m::quantize_block(&chunk_padded, &weights, iters),
            "iq2_xxs" => algos::iq2_xxs::quantize_block(&chunk_padded, &weights, iters),
            "iq2_xs" => algos::iq2_xs::quantize_block(&chunk_padded, &weights, iters),
            "iq2_s" => algos::iq2_s::quantize_block(&chunk_padded, &weights, iters),
            "iq3_xxs" => algos::iq3_xxs::quantize_block(&chunk_padded, &weights, iters),
            "iq3_s" => algos::iq3_s::quantize_block(&chunk_padded, &weights, iters),
            "iq4_xs" => algos::iq4_xs::quantize_block(&chunk_padded, &weights, iters),
            "iq4_nl" => algos::iq4_nl::quantize_block(&chunk_padded, &weights, iters),
            _ => algos::iq2_xxs::quantize_block(&chunk_padded, &weights, iters),
        };

        for i in 0..chunk_len {
            q_floats[b + i] = q_block[i];
        }

        let (mse, mae) = get_err_stats(chunk, &q_block[..chunk_len]);
        blocks.push(IqBlockMeta {
            idx: b / block_size,
            size: chunk_len,
            block_scale,
            mse,
            mae,
            scales,
            aux8,
            grids,
            signs,
        });
    }

    QuantizeOutput {
        q_floats,
        t_floats: None,
        t_q_floats: None,
        bpw,
        formula_html: formula_html.to_string(),
        block_size,
        super_block_size: 0,
        meta: QuantMeta::Iq(blocks),
    }
}

pub fn format_inspector(
    idx: usize,
    active: &[f32],
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::Iq(blocks) = &out.meta else {
        return InspectorData::default();
    };
    let block_size = out.block_size;
    let b_idx = idx / block_size;
    let mut data = InspectorData::default();

    if let Some(bm) = blocks.get(b_idx) {
        let q_val = out.q_floats[idx];

        let sign_char = if q_val < 0.0 { "-" } else { "+" };
        data.math_str = Some(format!("{} {:.4} (Grid mapped)", sign_char, q_val.abs()));

        let iq_html = match settings.iq_type.as_str() {
            "iq1_s" => algos::iq1_s::format_inspector(idx, active, &out.q_floats, bm),
            "iq1_m" => algos::iq1_m::format_inspector(idx, active, &out.q_floats, bm),
            "iq2_xxs" => algos::iq2_xxs::format_inspector(idx, active, &out.q_floats, bm),
            "iq2_xs" => algos::iq2_xs::format_inspector(idx, active, &out.q_floats, bm),
            "iq2_s" => algos::iq2_s::format_inspector(idx, active, &out.q_floats, bm),
            "iq3_xxs" => algos::iq3_xxs::format_inspector(idx, active, &out.q_floats, bm),
            "iq3_s" => algos::iq3_s::format_inspector(idx, active, &out.q_floats, bm),
            "iq4_xs" => algos::iq4_xs::format_inspector(idx, active, &out.q_floats, bm),
            "iq4_nl" => algos::iq4_nl::format_inspector(idx, active, &out.q_floats, bm),
            _ => None,
        };

        data.iq_html = iq_html;
        data.block_idx = Some(bm.idx);
        data.scale = Some(bm.block_scale);
        data.mse = Some(bm.mse);
        data.mae = Some(bm.mae);
    }
    data
}
