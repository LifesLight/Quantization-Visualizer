//! HTML for the TurboQuant inspector card, same compact rows as the IQ and KT ones.

use super::codebook::Codebook;
use super::TurboVec;
use crate::quants::iq::view::{chain_box, chain_row, row, TINY};

pub struct Hover {
    pub v: usize,
    pub j: usize,
    pub d: usize,
    pub bits: u32,
    pub prod: bool,
    pub level: f32,
    pub qjl_part: f32,
    pub recon: f32,
    /// Values are rotated coefficients, not weights.
    pub rotated: bool,
}

pub fn render(h: &Hover, tv: &TurboVec, cb: &Codebook, orig: &[f32], recon: &[f32]) -> String {
    let mut html = String::from(r#"<div style="display:flex; flex-direction:column; gap:6px;">"#);

    let sq = |a: &[f32]| a.iter().map(|&x| x as f64 * x as f64).sum::<f64>();
    let diff: Vec<f32> = orig.iter().zip(recon).map(|(&a, &b)| a - b).collect();
    let rel = if sq(orig) > 0.0 {
        sq(&diff) / sq(orig)
    } else {
        0.0
    };
    let theory = if h.prod {
        super::prod_distortion(orig.len(), cb.distortion)
    } else {
        cb.distortion
    };
    html.push_str(&format!(
        r#"<div style="{}" title="&Vert;x&minus;x&#771;&Vert;&sup2;/&Vert;x&Vert;&sup2; of this vector: {:.4}, expected {:.4} after a random rotation.">{} #{} &middot; d {} &middot; {}b</div>"#,
        TINY,
        rel,
        theory,
        if h.prod { "TQ_prod" } else { "TQ_mse" },
        h.v,
        h.d,
        h.bits
    ));

    // norm -> level (+ qjl) -> value
    let i = tv.idx.get(h.j).map_or(0, |&i| i as usize);
    let mut chain = chain_row(
        &format!("&Vert;x&Vert; <span style=\"{}\">fp16</span>", TINY),
        &format!("{:.5}", tv.norm),
        "var(--accent-color)",
    );
    chain.push_str(&chain_row(
        &format!(
            "&times; c<sub>{}</sub><span style=\"{}\">/{}</span>",
            i,
            TINY,
            cb.levels.len()
        ),
        &format!("{:.5}", h.level),
        "var(--primary-color)",
    ));
    if h.prod {
        chain.push_str(&chain_row(
            &format!(
                r#"<span title="&radic;(&pi;/2)/d &middot; &gamma; &middot; S&#7488;&middot;qjl at {}, &gamma; = {:.5}">+ qjl<sub>{}</sub></span>"#,
                h.j, tv.gamma, h.j
            ),
            &format!("{:.5}", h.qjl_part),
            "inherit",
        ));
    }
    chain.push_str(&chain_row(
        if h.rotated { "= coeff" } else { "= weight" },
        &format!("{:.5}", h.recon),
        "var(--accent-color)",
    ));
    html.push_str(&chain_box(&chain));

    // how many coordinates of this vector landed on each level
    let n = cb.levels.len();
    let mut count = vec![0usize; n];
    for &k in &tv.idx {
        count[k as usize] += 1;
    }
    let max = count.iter().copied().max().unwrap_or(1).max(1);
    let mut bars = String::new();
    for (k, &c) in count.iter().enumerate() {
        let style = if k == i {
            "background:var(--accent-color);"
        } else {
            "background:var(--primary-color); opacity:0.55;"
        };
        bars.push_str(&format!(
            r#"<div style="flex:1; height:{:.1}%; {} border-radius:1px 1px 0 0;" title="Level {}: {:.5}, {} coords"></div>"#,
            (c as f32 / max as f32 * 100.0).max(if c > 0 { 6.0 } else { 0.0 }),
            style,
            k,
            cb.levels[k],
            c
        ));
    }
    html.push_str(&row(
        "levels",
        &format!(
            "{} Lloyd-Max levels, bar = coords of this vector on that level",
            n
        ),
        &format!(
            r#"<div style="display:flex; gap:{}px; height:20px; align-items:flex-end; border-bottom:1px solid var(--border-color);">{}</div>"#,
            if n > 32 { 0 } else { 1 },
            bars
        ),
    ));

    html.push_str("</div>");
    html
}
