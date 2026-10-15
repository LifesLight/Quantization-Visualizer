//! HTML for the IQ inspector card. Kept to a few compact rows, explanations live in tooltips.

use super::algos::{Codebook, IqDecode, IqType, Signs};
use crate::quants::IqBlockMeta;

pub(crate) const TINY: &str = "font-size:0.65rem; color:var(--text-muted);";

pub(crate) fn trim(v: f32) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i32)
    } else {
        format!("{:.3}", v)
    }
}

/// One labeled row: short muted label on the left, content filling the rest.
pub(crate) fn row(label: &str, title: &str, content: &str) -> String {
    format!(
        r#"<div style="display:flex; align-items:center; gap:6px;" title="{}"><span style="{} width:3.2em; flex:none;">{}</span><div style="flex:1; min-width:0;">{}</div></div>"#,
        title, TINY, label, content
    )
}

pub(crate) fn chain_row(label: &str, value: &str, color: &str) -> String {
    format!(
        r#"<div style="display:flex; justify-content:space-between; gap:6px;"><span>{}</span><span style="font-weight:bold; color:{};">{}</span></div>"#,
        label, color, value
    )
}

pub fn render(
    t: &IqType,
    bm: &IqBlockMeta,
    dec: &IqDecode,
    j: usize,
    orig: &[f32],
    recon: &[f32],
    rotated: bool,
) -> String {
    let mut html = String::from(r#"<div style="display:flex; flex-direction:column; gap:6px;">"#);

    let mut head = format!("{} #{}", t.name, bm.idx);
    if !bm.probe.is_empty() {
        let on = bm.probe.iter().filter(|&&p| p == 0).count();
        head.push_str(&format!(" &middot; {}/{} on grid", on, bm.probe.len()));
    }
    html.push_str(&format!(
        r#"<div style="{}" title="Super-block MSE {:.6}. On grid: groups whose rounded point is a grid entry, the rest use the best nearby entry.">{}</div>"#,
        TINY, bm.mse, head
    ));

    if let Some((short, long)) = &bm.note {
        html.push_str(&format!(
            r#"<div style="font-size:0.65rem; padding:2px 6px; border-radius:4px; border:1px solid #fbbf24; cursor:help;" title="{}">&#9888; {}</div>"#,
            long, short
        ));
    }

    // d -> sub-block scale -> table value -> weight, which is the actual multiplication
    let mut chain = chain_row("d", &format!("{:.5}", dec.d), "var(--accent-color)");
    if dec.sub_bits > 0 {
        chain.push_str(&chain_row(
            &format!(
                "&times; {}",
                dec.dl_expr.replace("{s}", &dec.sub_field.to_string())
            ),
            &format!("{:.5}", dec.dl),
            "var(--primary-color)",
        ));
    }
    let (label, value) = match &dec.codebook {
        Codebook::Grid { idx, size, levels } => {
            let k = j % levels.len();
            let neg = match dec.signs {
                Signs::Parity { neg } | Signs::Full { neg } => neg & (1 << (j % 8)) != 0,
                Signs::None => false,
            };
            let v = match dec.delta {
                Some(d) => format!(
                    "{}{}.125",
                    trim(levels[k]),
                    if d < 0.0 { "&minus;" } else { "+" }
                ),
                None => format!("{}{}", if neg { "&minus;" } else { "" }, trim(levels[k])),
            };
            let probe = bm.probe.get(j / t.group_size).copied().unwrap_or(0);
            let fallback = if probe > 0 {
                format!(
                    r#" <span style="{}" title="Rounded point not on the grid. Best of {} nearby entries by weighted error.">nb{}</span>"#,
                    TINY, probe, probe
                )
            } else {
                String::new()
            };
            (
                format!(
                    "&times; grid #{}<span style=\"{}\">/{}</span>{}",
                    idx, TINY, size, fallback
                ),
                v,
            )
        }
        Codebook::NonLinear { idx, values } => (
            format!("&times; kvalues <span style=\"{}\">q={}</span>", TINY, idx),
            values[*idx].to_string(),
        ),
    };
    chain.push_str(&chain_row(&label, &value, "inherit"));
    chain.push_str(&chain_row(
        if rotated { "= coeff" } else { "= weight" },
        &format!("{:.5}", recon.get(j).copied().unwrap_or(0.0)),
        "var(--accent-color)",
    ));
    html.push_str(&chain_box(&chain));

    // Sub-block scales of this super-block
    let subs = (t.sub_scales)(&bm.bytes);
    if subs.len() > 1 {
        html.push_str(&scale_bars(&subs, j / t.sub_size, t.sub_size));
    }

    match dec.signs {
        Signs::Parity { neg } | Signs::Full { neg } => {
            html.push_str(&signs_row(dec, neg, j, orig));
        }
        Signs::None => {}
    }

    if let Codebook::NonLinear { idx, values } = &dec.codebook {
        html.push_str(&levels_row(dec, *idx, values, orig.get(j).copied()));
    }

    html.push_str("</div>");
    html
}

fn signs_row(dec: &IqDecode, neg: u8, j: usize, orig: &[f32]) -> String {
    let parity = matches!(dec.signs, Signs::Parity { .. });
    let (boxes, flipped) = sign_boxes(neg, parity, j, orig);
    let title = if parity {
        match flipped {
            Some(k) => format!("7 stored sign bits, dashed 8th implied by even parity. Odd negative count, #{} (smallest w&middot;x&sup2;) flipped.", k + 1),
            None => "7 stored sign bits, dashed 8th implied by even parity.".into(),
        }
    } else {
        "8 stored sign bits.".into()
    };
    row("signs", &title, &boxes)
}

fn levels_row(dec: &IqDecode, idx: usize, values: &[i8], x: Option<f32>) -> String {
    let lo = values[0] as f32;
    let hi = values[values.len() - 1] as f32;
    let pos = |v: f32| ((v - lo) / (hi - lo) * 100.0).clamp(0.0, 100.0);
    let mut strip = String::new();
    for (i, &v) in values.iter().enumerate() {
        let style = if i == idx {
            "background:var(--accent-color); height:100%; width:3px;"
        } else {
            "background:var(--primary-color); opacity:0.5; height:55%; width:1px;"
        };
        strip.push_str(&format!(
            r#"<div style="position:absolute; left:{:.2}%; bottom:0; transform:translateX(-50%); {}" title="{}"></div>"#,
            pos(v as f32),
            style,
            v
        ));
    }
    let mut caption = format!("snapped to {}", values[idx]);
    if let Some(x) = x {
        let t = if dec.dl != 0.0 { x / dec.dl } else { 0.0 };
        strip.push_str(&format!(
            r#"<div style="position:absolute; left:{:.2}%; top:-3px; transform:translateX(-50%); width:0; height:0; border-left:3px solid transparent; border-right:3px solid transparent; border-top:4px solid var(--negative-color);"></div>"#,
            pos(t)
        ));
        caption = format!("w&divide;scale {:.1} &rarr; {}", t, values[idx]);
    }
    row(
        "table",
        "kvalues_iq4nl on a number line from -127 to 113. Red marker: weight divided by the block scale. Green: the level it snapped to.",
        &format!(
            r#"<div style="position:relative; height:12px; margin:0 3px; border-bottom:1px solid var(--border-color);">{}</div><div style="{} margin-top:1px;">{}</div>"#,
            strip, TINY, caption
        ),
    )
}

pub(crate) fn chain_box(chain: &str) -> String {
    format!(
        r#"<div style="display:flex; flex-direction:column; gap:1px; background:var(--hover-bg-strong); padding:5px 7px; border-radius:6px; font-size:0.72rem;">{}</div>"#,
        chain
    )
}

/// Bar per sub-block scale of the hovered super-block, the active one highlighted.
pub(crate) fn scale_bars(subs: &[f32], active: usize, sub_size: usize) -> String {
    let max = subs.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    let mut bars = String::new();
    for (i, &s) in subs.iter().enumerate() {
        let pct = if max > 0.0 {
            s.abs() / max * 100.0
        } else {
            0.0
        };
        let style = if i == active {
            "background:var(--accent-color);"
        } else {
            "background:var(--primary-color); opacity:0.55;"
        };
        bars.push_str(&format!(
            r#"<div style="flex:1; height:{:.1}%; {} border-radius:1px 1px 0 0;" title="Sub-block {}: {:.5}"></div>"#,
            pct.max(6.0),
            style,
            i,
            s
        ));
    }
    row(
        "scales",
        &format!("{} sub-blocks of {} weights", subs.len(), sub_size),
        &format!(
            r#"<div style="display:flex; gap:1px; height:20px; align-items:flex-end; border-bottom:1px solid var(--border-color);">{}</div>"#,
            bars
        ),
    )
}

/// Sign boxes for the 8 weights around j. Flips against the original data get an amber
/// outline, with parity the implied 8th box is dashed. Returns the html and the flipped lane.
pub(crate) fn sign_boxes(neg: u8, parity: bool, j: usize, orig: &[f32]) -> (String, Option<usize>) {
    let start = (j / 8) * 8;
    let mut boxes = String::new();
    let mut flipped = None;
    for k in 0..8 {
        let e = start + k;
        let q_neg = neg & (1 << k) != 0;
        let is_flip = orig.get(e).map_or(false, |&v| (v < 0.0) != q_neg);
        if is_flip {
            flipped = Some(k);
        }
        let bg = if q_neg {
            "var(--negative-color)"
        } else {
            "var(--accent-color)"
        };
        let border = if is_flip {
            "border:1px solid #fbbf24;"
        } else if parity && k == 7 {
            "border:1px dashed var(--text-muted);"
        } else {
            "border:1px solid transparent;"
        };
        let hover = if e == j {
            " box-shadow:0 0 0 1px var(--text-main);"
        } else {
            ""
        };
        boxes.push_str(&format!(
            r#"<div style="flex:1; text-align:center; font-size:0.62rem; line-height:14px; border-radius:3px; background:{}; color:white; font-weight:bold; {}{}">{}</div>"#,
            bg,
            border,
            hover,
            if q_neg { "&minus;" } else { "+" }
        ));
    }
    (
        format!(r#"<div style="display:flex; gap:2px;">{}</div>"#, boxes),
        flipped,
    )
}
