//! HTML for the IQ*_K inspector card, same compact rows as the IQ and KT ones.

use super::common::QK_K;
use super::{IqkMeta, KDecode, KType};
use crate::quants::iq::view::{chain_box, chain_row, row, scale_bars, trim, TINY};

pub fn render(
    t: &KType,
    meta: &IqkMeta,
    dec: &KDecode,
    idx: usize,
    recon: &[f32],
    rotated: bool,
) -> String {
    let ibl = idx / QK_K;
    let j = idx % QK_K;
    let mut html = String::from(r#"<div style="display:flex; flex-direction:column; gap:6px;">"#);

    let (mse, _) = meta.err.get(ibl).copied().unwrap_or((0.0, 0.0));
    let scope = match t.row_meta {
        0 => "One FP16 scale per 256.",
        2 => "One FP16 scale for the whole row.",
        _ => "One FP32 scale for the whole row.",
    };
    html.push_str(&format!(
        r#"<div style="{}" title="Super-block MSE {:.6}. {}">{} #{} &middot; {} d {:.5}</div>"#,
        TINY,
        mse,
        scope,
        t.name,
        ibl,
        if dec.row { "row" } else { "block" },
        dec.d
    ));

    // d -> block scale -> table value -> weight
    let mut chain = chain_row(
        &format!(
            "d <span style=\"{}\">{}</span>",
            TINY,
            match t.row_meta {
                4 => "fp32 row",
                2 => "fp16 row",
                _ => "fp16",
            }
        ),
        &format!("{:.5}", dec.d),
        "var(--accent-color)",
    );
    chain.push_str(&chain_row(
        &format!(
            "&times; {}<span style=\"{}\">/{}b</span>",
            dec.dl_expr.replace("{s}", &dec.field.to_string()),
            TINY,
            dec.field_bits
        ),
        &format!("{:.5}", dec.dl),
        "var(--primary-color)",
    ));
    let label = match dec.pair {
        Some((a, b, k)) => format!(
            "&times; pair #{}<span style=\"{}\">/32 ({}, {})<sub>{}</sub></span>",
            dec.idx,
            TINY,
            trim(a),
            trim(b),
            k
        ),
        None => format!(
            "&times; q={}<span style=\"{}\">/{}b{}</span>",
            dec.idx,
            TINY,
            dec.idx_bits,
            if dec.shifted { " &middot;sh" } else { "" }
        ),
    };
    chain.push_str(&chain_row(&label, &trim(dec.value), "inherit"));
    chain.push_str(&chain_row(
        if rotated { "= coeff" } else { "= weight" },
        &format!("{:.5}", recon.get(j).copied().unwrap_or(0.0)),
        "var(--accent-color)",
    ));
    html.push_str(&chain_box(&chain));

    // where the value sits in its table, and the shifted copy next to it
    let all: Vec<f32> = dec.base.iter().chain(dec.up.iter()).copied().collect();
    let lo = all.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = all.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let partner = dec.pair.map(|(a, b, k)| if k == 0 { b } else { a });
    let mut tracks = track(
        &dec.base,
        lo,
        hi,
        (!dec.shifted).then_some(dec.mark),
        partner,
        !dec.shifted,
    );
    let title = if dec.up.is_empty() {
        "The 8 IQ3_K levels, this weight and the other one of its pair."
    } else {
        tracks.push_str(&track(
            &dec.up,
            lo,
            hi,
            dec.shifted.then_some(dec.mark),
            None,
            dec.shifted,
        ));
        "Top: the table, bottom: its shifted copy. Each block uses one of them."
    };
    html.push_str(&row(
        "levels",
        title,
        &format!(
            r#"<div style="display:flex; flex-direction:column; gap:2px;">{}</div>"#,
            tracks
        ),
    ));

    let scales = (t.block_scales)(&meta.bytes, ibl);
    let dls: Vec<f32> = scales.iter().map(|s| s.0).collect();
    let active = j / t.sub_size;
    html.push_str(&scale_bars(&dls, active, t.sub_size));
    if !dec.up.is_empty() {
        html.push_str(&row(
            "shift",
            "Blocks that use the shifted table.",
            &shift_boxes(&scales, active),
        ));
    }

    html.push_str("</div>");
    html
}

/// Levels as ticks on a line, `mark` is this weight's level.
fn track(
    levels: &[f32],
    lo: f32,
    hi: f32,
    mark: Option<usize>,
    partner: Option<f32>,
    used: bool,
) -> String {
    let span = if hi > lo { hi - lo } else { 1.0 };
    let mut ticks = String::new();
    for (i, &v) in levels.iter().enumerate() {
        let left = (v - lo) / span * 100.0;
        let (h, bg) = if mark == Some(i) {
            ("100%", "var(--accent-color)")
        } else if partner == Some(v) {
            ("100%", "var(--primary-color)")
        } else {
            ("55%", "var(--text-muted)")
        };
        ticks.push_str(&format!(
            r#"<div style="position:absolute; left:calc({:.2}% - 1px); bottom:0; width:2px; height:{}; background:{};" title="{}"></div>"#,
            left,
            h,
            bg,
            trim(v)
        ));
    }
    format!(
        r#"<div style="position:relative; height:9px; border-bottom:1px solid var(--border-color); opacity:{};">{}</div>"#,
        if used { "1" } else { "0.35" },
        ticks
    )
}

fn shift_boxes(scales: &[(f32, bool)], active: usize) -> String {
    let mut boxes = String::new();
    for (i, &(_, sh)) in scales.iter().enumerate() {
        let bg = if sh {
            "var(--primary-color)"
        } else {
            "var(--hover-bg-strong)"
        };
        let ring = if i == active {
            " box-shadow:0 0 0 1px var(--text-main);"
        } else {
            ""
        };
        boxes.push_str(&format!(
            r#"<div style="flex:1; height:8px; border-radius:2px; background:{};{}" title="Block {}: {}"></div>"#,
            bg,
            ring,
            i,
            if sh { "shifted" } else { "table" }
        ));
    }
    format!(r#"<div style="display:flex; gap:2px;">{}</div>"#, boxes)
}
