//! HTML for the KT inspector card, same compact rows as the IQ one.

use super::codebook::lane_state;
use super::common::{KtDecode, QK_K};
use super::{KtType, TrellisMeta};
use crate::quants::iq::view::{chain_box, chain_row, row, scale_bars, sign_boxes, trim, TINY};

pub fn render(
    t: &KtType,
    meta: &TrellisMeta,
    dec: &KtDecode,
    idx: usize,
    orig: &[f32],
    recon: &[f32],
) -> String {
    let ibl = idx / QK_K;
    let j = idx % QK_K;
    let mut html = String::from(r#"<div style="display:flex; flex-direction:column; gap:6px;">"#);

    let (mse, _) = meta.err.get(ibl).copied().unwrap_or((0.0, 0.0));
    html.push_str(&format!(
        r#"<div style="{}" title="Super-block MSE {:.6}. All super-blocks share the FP32 row scale.">{} #{} &middot; row d {:.5}</div>"#,
        TINY, mse, t.name, ibl, meta.row.d
    ));

    // d -> block scale -> generated value -> weight
    let mut chain = chain_row(
        &format!("{} <span style=\"{}\">row</span>", dec.d_label, TINY),
        &format!("{:.5}", dec.d),
        "var(--accent-color)",
    );
    chain.push_str(&chain_row(
        &format!(
            "&times; {}",
            dec.dl_expr.replace("{s}", &dec.field.to_string())
        ),
        &format!("{:.5}", dec.dl),
        "var(--primary-color)",
    ));
    let neg = dec.neg.map_or(false, |n| n & (1 << dec.lane) != 0);
    let bank = if dec.offset != 4096 {
        format!(
            r#"<span style="{}" title="Second codebook bank, seed offset 36864.">&middot;b1</span>"#,
            TINY
        )
    } else {
        String::new()
    };
    chain.push_str(&chain_row(
        &format!(
            "&times; gen #{}<span style=\"{}\">/{}b</span>{}",
            dec.idx, TINY, dec.index_bits, bank
        ),
        &format!("{}{}", if neg { "&minus;" } else { "" }, trim(dec.value)),
        "inherit",
    ));
    chain.push_str(&chain_row(
        "= weight",
        &format!("{:.5}", recon.get(j).copied().unwrap_or(0.0)),
        "var(--accent-color)",
    ));
    html.push_str(&chain_box(&chain));

    html.push_str(&scale_bars(&(t.block_scales)(&meta.row, ibl), j / 32, 32));

    // How the hovered lane's value comes out of the generator
    let b = lane_state(dec.idx, dec.offset, dec.lane).to_le_bytes();
    let sum = format!(
        "{}+{}+{}+{}&minus;126 = {}",
        b[0],
        b[1],
        b[2],
        b[3],
        b[0] as i32 + b[1] as i32 + b[2] as i32 + b[3] as i32 - 126
    );
    html.push_str(&row(
        "lcg",
        &format!(
            "Seed = index + {}. Lane {} steps x = 0xCBAC1FED&middot;x {} times, keeps x &amp; 0x3f3f3f3f and sums its 4 bytes minus 126.{}",
            dec.offset,
            dec.lane,
            dec.lane + 1,
            if dec.neg.is_some() { " Absolute value, the sign is stored separately." } else { "" }
        ),
        &format!(r#"<span style="font-size:0.65rem;">{}</span>"#, sum),
    ));

    if let Some(n) = dec.neg {
        let (boxes, _) = sign_boxes(n, false, j, orig);
        html.push_str(&row("signs", "8 stored sign bits, one per weight.", &boxes));
    }

    html.push_str("</div>");
    html
}
