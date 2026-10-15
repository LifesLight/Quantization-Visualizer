//! HTML for the QTIP inspector card: the bit window of the hovered weight, how the code turns the
//! state into a value, and the values the step could have reached.

use super::codes::{self, Code};
use super::trellis::{self, Codebook};
use super::T;
use crate::quants::iq::view::{chain_box, chain_row, row, TINY};

pub fn signed(v: f32, prec: usize) -> String {
    if v < 0.0 {
        format!("&minus;{:.*}", prec, -v)
    } else {
        format!("{:.*}", prec, v)
    }
}

pub fn render(
    cb: &Codebook,
    stream: &[u8],
    tile: usize,
    j: usize,
    target: f32,
    wscale: f32,
    recon: f32,
    rotated: bool,
) -> String {
    let steps = T / cb.v;
    let step = j / cb.v;
    let v = j % cb.v;
    let state = trellis::state_at(cb, stream, steps, step);
    let mut html = String::from(r#"<div style="display:flex; flex-direction:column; gap:6px;">"#);

    html.push_str(&format!(
        r#"<div style="{}" title="Tiles of 256 weights, each its own tail-biting trellis.">{} tile #{} &middot; step {}{}</div>"#,
        TINY,
        cb.code.name(),
        tile,
        step,
        if cb.v == 2 { format!(" &middot; {} of 2", v + 1) } else { String::new() }
    ));

    html.push_str(&row(
        "bits",
        &format!(
            "The tile's bitstream ({} bits, it wraps around). This weight's state is the {} bit window, the last {} bits came in with its step, the rest it shares with the weights before it.",
            steps * cb.kv() as usize,
            cb.l,
            cb.kv()
        ),
        &bit_strip(cb, stream, steps, step),
    ));

    // state -> code -> times scale -> weight
    let mut chain = chain_row(
        &format!("s <span style=\"{}\">{}b</span>", TINY, cb.l),
        &format!("0x{:0w$x}", state, w = (cb.l as usize + 3) / 4),
        "inherit",
    );
    let value = cb.value(state, v);
    match cb.code {
        Code::OneMad => {
            let x = codes::mad_word(state);
            chain.push_str(&chain_row(
                &format!(
                    "a&middot;s + b <span style=\"{}\">mod 2&sup3;&sup2;</span>",
                    TINY
                ),
                &format!("{:08x}", x),
                "inherit",
            ));
            let sum = (x & 255) + (x >> 8 & 255) + (x >> 16 & 255) + (x >> 24);
            chain.push_str(&chain_row("&Sigma; bytes", &sum.to_string(), "inherit"));
            chain.push_str(&chain_row(
                "(&Sigma; &minus; 510) / 147.8",
                &signed(value, 4),
                "var(--primary-color)",
            ));
        }
        Code::ThreeInst => {
            let (hi, lo) = codes::inst_halves(state);
            chain.push_str(&chain_row(
                &format!("hi &oplus; m <span style=\"{}\">fp16</span>", TINY),
                &signed(hi.to_f32(), 4),
                "inherit",
            ));
            chain.push_str(&chain_row(
                &format!("+ lo &oplus; m <span style=\"{}\">fp16</span>", TINY),
                &signed(lo.to_f32(), 4),
                "inherit",
            ));
            chain.push_str(&chain_row(
                "= code",
                &signed(value, 4),
                "var(--primary-color)",
            ));
        }
        Code::Hyb => {
            let (x, idx, flip) = codes::hyb_index(state);
            let [a, b] = codes::hyb_entry(idx);
            chain.push_str(&chain_row(
                &format!(
                    "<span title=\"bits 6..14 pick entry #{}, bit 15 {}\">s&sup2; + s <span style=\"{}\">&rarr; #{}</span></span>",
                    idx,
                    if flip { "is set, the first value flips sign" } else { "is clear, no sign flip" },
                    TINY,
                    idx
                ),
                &format!("{:x}", x),
                "inherit",
            ));
            let (a, b) = (if flip { -a } else { a }, b);
            let pair = if v == 0 {
                format!("<b>{}</b>, {}", signed(a, 3), signed(b, 3))
            } else {
                format!("{}, <b>{}</b>", signed(a, 3), signed(b, 3))
            };
            chain.push_str(&chain_row(
                &format!("lut <span style=\"{}\">({})</span>", TINY, pair),
                &signed(value, 4),
                "var(--primary-color)",
            ));
        }
    }
    chain.push_str(&chain_row(
        &format!("&times; d <span style=\"{}\">fp32</span>", TINY),
        &format!("{:.5}", wscale),
        "inherit",
    ));
    chain.push_str(&chain_row(
        if rotated { "= coeff" } else { "= weight" },
        &signed(recon, 5),
        "var(--accent-color)",
    ));
    html.push_str(&chain_box(&chain));

    html.push_str(&row(
        "reach",
        &format!(
            "The {} values this step could pick from the previous state (only its last {} bits are free), the one the path took, and the weight / d it had to match (caret). Not always the closest, the next weights share these bits.",
            1u32 << cb.kv(),
            cb.kv()
        ),
        &reach(cb, stream, steps, step, v, target),
    ));

    html.push_str("</div>");
    html
}

/// The bits around the window of `step`, grouped by step, window highlighted, new bits bold.
fn bit_strip(cb: &Codebook, stream: &[u8], steps: usize, step: usize) -> String {
    let kv = cb.kv() as usize;
    let nbits = steps * kv;
    let l = cb.l as usize;
    let start = step * kv;
    let group = if kv < 2 { 4 } else { kv };
    let lo = start as isize - kv as isize;
    let hi = (start + l + kv) as isize;
    let mut s = String::from(
        r#"<div style="font-family:'JetBrains Mono', monospace; font-size:0.68rem; white-space:nowrap; overflow:hidden; letter-spacing:0.02em;">"#,
    );
    for p in lo..hi {
        let pos = p.rem_euclid(nbits as isize) as usize;
        if p > lo && pos % group == 0 {
            s.push(if pos == 0 { '|' } else { ' ' });
        }
        let b = trellis::bit(stream, nbits, pos);
        let in_win = p >= start as isize && p < (start + l) as isize;
        let fresh = p >= (start + l - kv) as isize && in_win;
        let style = if fresh {
            "color:var(--accent-color); font-weight:bold; background:var(--highlight-bg);"
        } else if in_win {
            "background:var(--highlight-bg);"
        } else {
            "color:var(--text-muted); opacity:0.6;"
        };
        s.push_str(&format!(r#"<span style="{}">{}</span>"#, style, b));
    }
    s.push_str("</div>");
    s
}

/// Ticks for the values reachable from the previous state, the chosen one, and a caret at the target.
fn reach(cb: &Codebook, stream: &[u8], steps: usize, step: usize, v: usize, target: f32) -> String {
    let kv = cb.kv();
    let prev = trellis::state_at(cb, stream, steps, (step + steps - 1) % steps);
    let base = (prev << kv) & ((1u32 << cb.l) - 1);
    let state = trellis::state_at(cb, stream, steps, step);
    let vals: Vec<f32> = (0..1u32 << kv).map(|t| cb.value(base | t, v)).collect();
    let chosen = (state & ((1 << kv) - 1)) as usize;
    let lo = vals.iter().copied().fold(target, f32::min);
    let hi = vals.iter().copied().fold(target, f32::max);
    let span = if hi > lo { hi - lo } else { 1.0 };
    let pct = |x: f32| (x - lo) / span * 100.0;
    let mut ticks = String::new();
    for (t, &x) in vals.iter().enumerate() {
        if t == chosen {
            continue;
        }
        ticks.push_str(&format!(
            r#"<div style="position:absolute; left:calc({:.2}% - 1px); bottom:3px; width:2px; height:55%; background:var(--text-muted);" title="{}"></div>"#,
            pct(x),
            signed(x, 3)
        ));
    }
    ticks.push_str(&format!(
        r#"<div style="position:absolute; left:calc({:.2}% - 1px); bottom:3px; width:2px; height:100%; background:var(--accent-color);" title="picked {}"></div>"#,
        pct(vals[chosen]),
        signed(vals[chosen], 3)
    ));
    ticks.push_str(&format!(
        r#"<div style="position:absolute; left:calc({:.2}% - 4px); bottom:-4px; width:0; height:0; border-left:4px solid transparent; border-right:4px solid transparent; border-bottom:5px solid var(--primary-color);" title="target {}"></div>"#,
        pct(target),
        signed(target, 3)
    ));
    format!(
        r#"<div style="position:relative; height:12px; border-bottom:1px solid var(--border-color); margin-bottom:3px;">{}</div>"#,
        ticks
    )
}
