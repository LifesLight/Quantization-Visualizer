use super::best_index_iq4nl;
use crate::math_utils::*;
use crate::quants::iq::lut;
use crate::quants::IqBlockMeta;

pub fn block_size() -> usize {
    32
}

pub fn bpw() -> f32 {
    4.5
}

pub fn quantize_block(
    block: &[f32],
    weights: &[f32],
    iters: i32,
) -> (Vec<f32>, f32, Vec<f32>, Vec<i8>, Vec<usize>, Vec<u8>) {
    let mut q_block = vec![0.0; 32];

    let mut max = 0.0_f32;
    let mut amax = 0.0_f32;
    for i in 0..32 {
        let abs_v = block[i].abs();
        if abs_v > amax {
            amax = abs_v;
            max = block[i];
        }
    }

    if amax < 1e-6 {
        return (q_block, 0.0, vec![], vec![], vec![], vec![]);
    }

    let mut best_scale = 0.0;
    let mut best_score = -1.0;

    for is in -iters..=iters {
        // FIX: Map directly to raw initial negative LUT index mapping to utilize the full signed dynamic range.
        let id = (is as f32 + lut::KVALUES_IQ4NL[0]) / max;

        let mut sumqx = 0.0;
        let mut sumq2 = 0.0;

        for i in 0..32 {
            let al = id * block[i];
            let l = best_index_iq4nl(al);

            let q = lut::KVALUES_IQ4NL[l];
            let w = weights[i];
            sumqx += w * q * block[i];
            sumq2 += w * q * q;
        }

        let scale_cand = if sumq2 > 0.0 { sumqx / sumq2 } else { 0.0 };
        let score = if sumq2 > 0.0 {
            sumqx * sumqx / sumq2
        } else {
            0.0
        };

        if score > best_score {
            best_score = score;
            best_scale = scale_cand;
        }
    }

    let d = fp16(best_scale);
    let id = if best_scale != 0.0 {
        1.0 / best_scale
    } else {
        0.0
    };

    for i in 0..32 {
        let l = best_index_iq4nl(id * block[i]);
        q_block[i] = d * lut::KVALUES_IQ4NL[l];
    }

    (q_block, d, vec![], vec![], vec![], vec![])
}

pub fn formula_html() -> &'static str {
    "<span>Weight = <span class=\"eq-pill\">NonLinearVal<span class=\"bits\">4b</span></span> &times; <span class=\"eq-pill\">BlockScale<span class=\"bits\">16b</span></span></span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Direct 4-bit non-linear quantization with standalone 16-bit scales, optimized for Gaussian outlier distributions.</span>"
}

pub fn format_inspector(
    global_idx: usize,
    _active_floats: &[f32],
    q_floats: &[f32],
    bm: &IqBlockMeta,
) -> Option<String> {
    let q_val = q_floats[global_idx];
    let scale_val = bm.block_scale;
    let id = if scale_val != 0.0 {
        1.0 / scale_val
    } else {
        0.0
    };
    let unscaled_val = id * q_val;
    let active_centroid_idx = best_index_iq4nl(unscaled_val);

    let mut distribution_html = String::new();
    for i in 0..16 {
        let centroid_val = lut::KVALUES_IQ4NL[i];
        let is_hovered_centroid = i == active_centroid_idx;
        let bg_color = if is_hovered_centroid {
            "background:var(--accent-color); color:white; font-weight:bold;"
        } else if centroid_val < 0.0 {
            "background:var(--negative-color); opacity:0.6; color:white;"
        } else {
            "background:var(--primary-color); opacity:0.6; color:white;"
        };

        distribution_html.push_str(&format!(
            "<div style=\"flex:1; text-align:center; padding:4px 0; font-size:0.65rem; border-radius:3px; {};\" title=\"Centroid {}: {:.1} (Quantized: {:.4})\">{}</div>",
            bg_color, i, centroid_val, scale_val * centroid_val, centroid_val.round() as i32
        ));
    }

    Some(format!(
        r#"<div style="display:flex; flex-direction:column; gap:8px;">
            <div style="display:flex; justify-content:space-around; align-items:center; background:var(--bg-secondary); padding:8px; border-radius:6px; font-size:0.8rem;">
                <div style="display:flex; flex-direction:column; align-items:center; min-width:75px;">
                    <span style="font-weight:bold; color:var(--accent-color);">{:.4}</span>
                    <span style="font-size:0.65rem; color:var(--text-muted);">Block-Scale</span>
                </div>
                <div style="color:var(--text-muted); font-weight:bold; margin: 0 4px;">&rarr;</div>
                <div style="display:flex; flex-direction:column; align-items:center; min-width:75px;">
                    <span style="font-weight:bold; color:var(--primary-color);">{:.1}</span>
                    <span style="font-size:0.65rem; color:var(--text-muted);">Centroid</span>
                </div>
                <div style="color:var(--text-muted); font-weight:bold; margin: 0 4px;">&rarr;</div>
                <div style="display:flex; flex-direction:column; align-items:center; min-width:75px;">
                    <span style="font-weight:bold; font-size: 0.75rem;">#{:02}</span>
                    <span style="font-size:0.65rem; color:var(--text-muted);">Centroid Idx</span>
                </div>
            </div>

            <div style="font-size:0.8rem; color:var(--text-muted); margin-top:8px;">Non-Linear Gaussian Centroids (16 levels)</div>
            <div style="display:flex; gap:2px; padding:2px 0;">
                {}
            </div>
            <div style="font-size:0.75rem; color:var(--text-muted); line-height:1.3; margin-top:4px;">
                Weight mapped to non-linear index #{}. Centroids are non-uniformly spaced to allocate higher resolution to smaller values, matching a Gaussian bell curve.
            </div>
        </div>"#,
        scale_val,
        lut::KVALUES_IQ4NL[active_centroid_idx],
        active_centroid_idx,
        distribution_html,
        active_centroid_idx
    ))
}
