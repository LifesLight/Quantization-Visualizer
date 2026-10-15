//! IQ quants from llama.cpp. Every type quantizes into its real block layout and is
//! decoded with the reference dequantizer, so what's shown is exactly what llama.cpp stores.

pub mod algos;
pub mod grid;
#[rustfmt::skip]
pub mod lut;
mod view;

use algos::IqType;

use crate::math_utils::*;
use crate::quants::{
    ImportanceResult, InspectorData, IqBlockMeta, QuantMeta, QuantizeOutput, Settings,
};

fn iq_type(name: &str) -> &'static IqType {
    match name {
        "iq1_s" => &algos::iq1_s::TYPE,
        "iq1_m" => &algos::iq1_m::TYPE,
        "iq2_xxs" => &algos::iq2_xxs::TYPE,
        "iq2_xs" => &algos::iq2_xs::TYPE,
        "iq2_s" => &algos::iq2_s::TYPE,
        "iq3_xxs" => &algos::iq3_xxs::TYPE,
        "iq3_s" => &algos::iq3_s::TYPE,
        "iq4_xs" => &algos::iq4_xs::TYPE,
        "iq4_nl" => &algos::iq4_nl::TYPE,
        _ => &algos::iq2_xxs::TYPE,
    }
}

pub fn quantize(
    floats: &[f32],
    importance: Option<&ImportanceResult>,
    settings: &Settings,
) -> QuantizeOutput {
    let t = iq_type(settings.iq_type.as_str());
    let n = floats.len();
    let sb = t.super_size;

    let mut q_floats = vec![0.0; n];
    let mut blocks = Vec::new();

    let mut x = vec![0.0f32; sb];
    let mut qw = vec![0.0f32; sb];
    let mut y = vec![0.0f32; sb];

    for b in (0..n).step_by(sb) {
        let len = (n - b).min(sb);
        let chunk = &floats[b..b + len];

        // The importance vector is one value per weight, which for a single row is exactly
        // what a llama.cpp imatrix holds. A partial tail block is zero padded and the padding
        // gets zero weight so it doesn't pull on the scales.
        x[..len].copy_from_slice(chunk);
        x[len..].fill(0.0);
        let have_qw = match importance {
            Some(imp) => {
                qw[..len].copy_from_slice(&imp.raw[b..b + len]);
                true
            }
            None if t.needs_imatrix => {
                qw[..len].fill(1.0);
                true
            }
            None => false,
        };
        qw[len..].fill(0.0);

        let enc = (t.quantize)(&x, if have_qw { Some(&qw) } else { None });
        (t.dequantize)(&enc.bytes, &mut y);
        q_floats[b..b + len].copy_from_slice(&y[..len]);

        let (mse, mae) = get_err_stats(chunk, &y[..len]);
        blocks.push(IqBlockMeta {
            idx: b / sb,
            size: len,
            bytes: enc.bytes,
            probe: enc.probe,
            note: enc.note,
            mse,
            mae,
        });
    }

    let mut formula_html = (t.formula_html)();
    if importance.is_none() && t.needs_imatrix {
        formula_html.push_str(" <span style=\"color:var(--text-muted);font-size:0.8rem;cursor:help;text-decoration:underline dotted;\" title=\"llama.cpp requires an imatrix for this type. Uses a flat imatrix (all 1) until importance is enabled.\">Flat imatrix.</span>");
    }

    QuantizeOutput {
        q_floats,
        t_floats: None,
        t_q_floats: None,
        bpw: t.bpw(),
        formula_html,
        block_size: t.sub_size,
        super_block_size: if sb > t.sub_size { sb } else { 0 },
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
    let t = iq_type(settings.iq_type.as_str());
    let mut data = InspectorData::default();

    let Some(bm) = blocks.get(idx / t.super_size) else {
        return data;
    };
    let j = idx % t.super_size;
    let base = idx - j;
    let dec = (t.decode)(&bm.bytes, j);

    let sub_start = base + (j / t.sub_size) * t.sub_size;
    let sub_end = (sub_start + t.sub_size).min(base + bm.size);
    let (mse, mae) = get_err_stats(
        &active[sub_start..sub_end],
        &out.q_floats[sub_start..sub_end],
    );

    data.math_str = Some(dec.math.clone());
    data.block_idx = Some(idx / t.sub_size);
    data.super_idx = Some(bm.idx);
    data.mse = Some(mse);
    data.mae = Some(mae);
    data.scale = Some(dec.dl);
    data.scale_label = Some("Sub-scale:".into());
    data.iq_html = Some(view::render(
        t,
        bm,
        &dec,
        j,
        &active[base..base + bm.size],
        &out.q_floats[base..base + bm.size],
    ));
    data
}
