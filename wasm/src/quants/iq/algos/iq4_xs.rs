use super::best_index_iq4nl;
use crate::math_utils::*;
use crate::quants::iq::lut;
use crate::quants::IqBlockMeta;

pub fn block_size() -> usize {
    256
}

pub fn bpw() -> f32 {
    4.25
}

pub fn quantize_block(
    super_block: &[f32],
    weights: &[f32],
    iters: i32,
) -> (Vec<f32>, f32, Vec<f32>, Vec<i8>, Vec<usize>, Vec<u8>) {
    let mut q_block = vec![0.0; 256];
    let mut qs_out = vec![0u8; 256];
    let mut scales = [0.0; 8];

    let mut amax_scale = 0.0;
    let mut max_scale = 0.0;

    for ib in 0..8 {
        let chunk = &super_block[ib * 32..(ib + 1) * 32];
        let w_chunk = &weights[ib * 32..(ib + 1) * 32];

        let mut amax = 0.0_f32;
        let mut max = 0.0_f32;
        for &v in chunk {
            if v.abs() > amax {
                amax = v.abs();
                max = v;
            }
        }

        if amax < 1e-6 {
            scales[ib] = 0.0;
            continue;
        }

        // Evaluate initial candidate
        let mut d = if iters > 0 {
            -max / lut::KVALUES_IQ4NL[0]
        } else {
            max / lut::KVALUES_IQ4NL[0]
        };
        let id = if d != 0.0 { 1.0 / d } else { 0.0 };

        let mut sumqx = 0.0;
        let mut sumq2 = 0.0;

        for i in 0..32 {
            let al = id * chunk[i];
            let l = best_index_iq4nl(al);

            let q = lut::KVALUES_IQ4NL[l];
            let w = w_chunk[i];
            sumqx += w * q * chunk[i];
            sumq2 += w * q * q;
        }

        d = if sumq2 > 0.0 { sumqx / sumq2 } else { 0.0 };
        let mut best_score = d * sumqx;
        let mut best_scale = d;

        for is in -iters..=iters {
            let id = (is as f32 + lut::KVALUES_IQ4NL[0]) / max;
            let mut sumqx = 0.0;
            let mut sumq2 = 0.0;

            for i in 0..32 {
                let al = id * chunk[i];
                let l = best_index_iq4nl(al);

                let q = lut::KVALUES_IQ4NL[l];
                let w = w_chunk[i];
                sumqx += w * q * chunk[i];
                sumq2 += w * q * q;
            }

            if sumq2 > 0.0 && sumqx * sumqx > best_score * sumq2 {
                d = sumqx / sumq2;
                best_score = d * sumqx;
                best_scale = d;
            }
        }

        scales[ib] = best_scale;
        if best_scale.abs() > amax_scale {
            amax_scale = best_scale.abs();
            max_scale = best_scale;
        }
    }

    if amax_scale == 0.0 {
        return (q_block, 0.0, vec![], vec![], vec![], qs_out);
    }

    let d = -max_scale / 32.0;
    let d_out = fp16(d);
    let id = if d != 0.0 { 1.0 / d } else { 0.0 };

    for ib in 0..8 {
        let mut l = (id * scales[ib]).round() as i32;
        l = l.clamp(-32, 31);
        let l_off = l + 32;

        let decoded_scale = d_out * (l_off as f32 - 32.0);

        let continuous_scale = d * (l_off as f32 - 32.0);
        let idl = if continuous_scale != 0.0 {
            1.0 / continuous_scale
        } else {
            0.0
        };

        // Output quantized scale value for inspector accuracy
        scales[ib] = decoded_scale;

        let chunk = &super_block[ib * 32..(ib + 1) * 32];
        for i in 0..32 {
            let al = idl * chunk[i];
            let q_idx = best_index_iq4nl(al);
            q_block[ib * 32 + i] = decoded_scale * lut::KVALUES_IQ4NL[q_idx];
            qs_out[ib * 32 + i] = q_idx as u8;
        }
    }

    (q_block, d_out, scales.to_vec(), vec![], vec![], qs_out)
}

pub fn formula_html() -> &'static str {
    "<span>Weight = <span class=\"eq-pill\">NonLinearVal<span class=\"bits\">4b</span></span> &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">6b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Maps 32-weight blocks to 4-bit non-linear logarithmic curves, scaled by hierarchical 6-bit step-sizes.</span>"
}

pub fn format_inspector(
    global_idx: usize,
    _active_floats: &[f32],
    _q_floats: &[f32],
    bm: &IqBlockMeta,
) -> Option<String> {
    let local_idx = global_idx % block_size();
    let scale_chunk_size = if bm.scales.len() > 0 {
        block_size() / bm.scales.len()
    } else {
        32
    };
    let tile_idx = local_idx / scale_chunk_size;

    let mut iq_html = String::new();

    let max_scale = bm.scales.iter().cloned().fold(0.0_f32, f32::max);
    let mut chart_html = String::new();
    for (i, &s) in bm.scales.iter().enumerate() {
        let pct = if max_scale > 0.0 {
            (s / max_scale) * 100.0
        } else {
            0.0
        };
        let is_active = if i == tile_idx {
            "background:var(--accent-color);"
        } else {
            "background:var(--primary-color); opacity:0.6;"
        };
        chart_html.push_str(&format!(
            "<div style=\"flex:1; height:{}%; {}; border-radius:2px 2px 0 0;\" title=\"Tile {}: {:.4}\"></div>",
            pct.max(5.0), is_active, i, s
        ));
    }

    iq_html.push_str(&format!(
        r#"<div style="display:flex; flex-direction:column; gap:8px;">
            <div style="display:flex; justify-content:space-around; align-items:center; background:var(--bg-secondary); padding:8px; border-radius:6px; font-size:0.8rem;">
                <div style="display:flex; flex-direction:column; align-items:center; min-width:65px;">
                    <span style="font-weight:bold; color:var(--accent-color);">{:.4}</span>
                    <span style="font-size:0.65rem; color:var(--text-muted);">Super-Scale</span>
                </div>
                <div style="color:var(--text-muted); font-weight:bold; margin: 0 4px;">&rarr;</div>
                <div style="display:flex; flex-direction:column; align-items:center; min-width:65px;">
                    <span style="font-weight:bold; color:var(--primary-color);">{:.4}</span>
                    <span style="font-size:0.65rem; color:var(--text-muted);">Tile-Scale</span>
                </div>
                <div style="color:var(--text-muted); font-weight:bold; margin: 0 4px;">&rarr;</div>
                <div style="display:flex; flex-direction:column; align-items:center; min-width:65px;">
                    <span style="font-weight:bold; font-size: 0.75rem;">{}</span>
                    <span style="font-size:0.65rem; color:var(--text-muted);">Vector Base</span>
                </div>
            </div>

            <div style="font-size:0.8rem; color:var(--text-muted); margin-top:8px;">Relative Tile Scales</div>
            <div style="display:flex; gap:2px; height:40px; align-items:flex-end; border-bottom:1px solid var(--border-color);">
                {}
            </div>
        </div>"#,
        bm.block_scale, bm.scales.get(tile_idx).unwrap_or(&0.0), "Grid", chart_html
    ));

    Some(iq_html)
}
