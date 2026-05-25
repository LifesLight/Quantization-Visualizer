use super::{find_best_grid_idx, get_grid_1bit};
use crate::math_utils::*;
use crate::quants::iq::lut;
use crate::quants::IqBlockMeta;

pub fn block_size() -> usize {
    256
}

pub fn bpw() -> f32 {
    1.5625
}

fn get_iq1_s_grid() -> &'static [[u8; 8]] {
    static GRID: std::sync::OnceLock<Vec<[u8; 8]>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| get_grid_1bit(&lut::KGRID_1BIT_2048))
        .as_slice()
}

pub fn quantize_block(
    super_block: &[f32],
    weights: &[f32],
    _iters: i32,
) -> (Vec<f32>, f32, Vec<f32>, Vec<i8>, Vec<usize>, Vec<u8>) {
    let mut q_block = vec![0.0; 256];
    let mut scales = [0.0; 8];
    let mut shifts = [1i8; 8];
    let mut tile_best_grids = [[0usize; 4]; 8];

    let grid = get_iq1_s_grid();
    let x_p = [-1.0 + 0.125, 0.125, 1.0 + 0.125];
    let x_m = [-1.0 - 0.125, -0.125, 1.0 - 0.125];

    let mut sumx2 = 0.0;
    for &v in super_block {
        sumx2 += v * v;
    }
    let sigma2 = 2.0 * sumx2 / 256.0;

    let mut max_scale = 0.0;

    for ib in 0..8 {
        let chunk = &super_block[ib * 32..(ib + 1) * 32];
        let base_w_chunk = &weights[ib * 32..(ib + 1) * 32];

        let mut max = 0.0_f32;
        let mut w_chunk = [0.0; 32];
        for i in 0..32 {
            let v = chunk[i];
            if v.abs() > max {
                max = v.abs();
            }
            w_chunk[i] = base_w_chunk[i] * (sigma2 + v * v).sqrt();
        }

        if max < 1e-6 {
            scales[ib] = 0.0;
            shifts[ib] = 1;
            continue;
        }

        let mut pairs: Vec<(f32, usize)> = chunk
            .iter()
            .copied()
            .enumerate()
            .map(|(i, v)| (v, i))
            .collect();
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let mut sumx = vec![0.0; 33];
        let mut sumw = vec![0.0; 33];
        for j in 0..32 {
            let idx = pairs[j].1;
            sumx[j + 1] = sumx[j] + w_chunk[idx] * chunk[idx];
            sumw[j + 1] = sumw[j] + w_chunk[idx];
        }

        let mut best_score = -f32::MAX;
        let mut best_scale = max;
        let mut best_shift = 1;
        let mut found_split = false;

        for i1 in 0..=32 {
            for i2 in i1..=32 {
                let sumqx_p = (sumx[i1] - sumx[0]) * x_p[0]
                    + (sumx[i2] - sumx[i1]) * x_p[1]
                    + (sumx[32] - sumx[i2]) * x_p[2];
                let sumq2_p = (sumw[i1] - sumw[0]) * x_p[0] * x_p[0]
                    + (sumw[i2] - sumw[i1]) * x_p[1] * x_p[1]
                    + (sumw[32] - sumw[i2]) * x_p[2] * x_p[2];
                if sumq2_p > 0.0 && sumqx_p * sumqx_p > best_score * sumq2_p {
                    best_scale = sumqx_p / sumq2_p;
                    best_score = best_scale * sumqx_p;
                    best_shift = 1;
                    found_split = true;
                }

                let sumqx_m = (sumx[i1] - sumx[0]) * x_m[0]
                    + (sumx[i2] - sumx[i1]) * x_m[1]
                    + (sumx[32] - sumx[i2]) * x_m[2];
                let sumq2_m = (sumw[i1] - sumw[0]) * x_m[0] * x_m[0]
                    + (sumw[i2] - sumw[i1]) * x_m[1] * x_m[1]
                    + (sumw[32] - sumw[i2]) * x_m[2] * x_m[2];
                if sumq2_m > 0.0 && sumqx_m * sumqx_m > best_score * sumq2_m {
                    best_scale = sumqx_m / sumq2_m;
                    best_score = best_scale * sumqx_m;
                    best_shift = -1;
                    found_split = true;
                }
            }
        }

        if !found_split {
            scales[ib] = 0.0;
            shifts[ib] = 1;
            continue;
        }

        if best_scale < 0.0 {
            best_scale = -best_scale;
            best_shift = -best_shift;
        }

        let xx = if best_shift == 1 { x_p } else { x_m };

        let mut final_sumqx = 0.0;
        let mut final_sumq2 = 0.0;
        for k in 0..4 {
            let best_g = find_best_grid_idx(
                grid,
                &chunk[k * 8..k * 8 + 8],
                &w_chunk[k * 8..k * 8 + 8],
                best_scale,
                &xx,
            );
            tile_best_grids[ib][k] = best_g;
            for i in 0..8 {
                let q = xx[grid[best_g][i] as usize];
                let w = w_chunk[k * 8 + i];
                final_sumqx += w * q * chunk[k * 8 + i];
                final_sumq2 += w * q * q;
            }
        }

        if final_sumqx > 0.0 && final_sumq2 > 0.0 {
            best_scale = final_sumqx / final_sumq2;
        }

        scales[ib] = best_scale;
        shifts[ib] = best_shift;
        if best_scale > max_scale {
            max_scale = best_scale;
        }
    }

    if max_scale == 0.0 {
        return (q_block, 0.0, vec![], vec![], vec![], vec![]);
    }

    let d = max_scale / 15.0;
    let d_out = fp16(d * 1.125);
    let id = if d > 0.0 { 1.0 / d } else { 0.0 };

    for ib in 0..8 {
        let mut l = (0.5 * (id * scales[ib] - 1.0)).round() as i32;
        l = l.clamp(0, 7);
        let decoded_scale = d_out * (2.0 * l as f32 + 1.0);
        let xx = if shifts[ib] == 1 { x_p } else { x_m };

        for k in 0..4 {
            let g_idx = tile_best_grids[ib][k];
            for i in 0..8 {
                let q = xx[grid[g_idx][i] as usize];
                q_block[ib * 32 + k * 8 + i] = decoded_scale * q;
            }
        }
    }

    let mut out_grids = vec![];
    for ib in 0..8 {
        out_grids.extend_from_slice(&tile_best_grids[ib]);
    }

    (
        q_block,
        d_out,
        scales.to_vec(),
        shifts.to_vec(),
        out_grids,
        vec![],
    )
}

pub fn formula_html() -> &'static str {
    "<span>Weight = ( <span class=\"eq-pill\">Codebook<span class=\"bits\">1.5b</span></span> &plusmn; <span title=\"Adaptive shift offset (+0.125 or -0.125) evaluated per tile to optimally center the grid vector distribution.\" style=\"border-bottom: 1px dotted var(--text-muted); cursor: help;\">0.125</span> ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">3b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Groups of 8 are snapped to a 3-state {-1, 0, 1} vector grid, utilizing an adaptive sign-shift offset.</span>"
}

pub fn format_inspector(
    global_idx: usize,
    active_floats: &[f32],
    q_floats: &[f32],
    bm: &IqBlockMeta,
) -> Option<String> {
    let local_idx = global_idx % block_size();
    let scale_chunk_size = if bm.scales.len() > 0 {
        block_size() / bm.scales.len()
    } else {
        32
    };
    let tile_idx = local_idx / scale_chunk_size;
    let sub_group_idx = local_idx % 8;

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

    let grid_val_disp = if !bm.grids.is_empty() {
        format!(
            "Entry #{}",
            bm.grids
                .get(tile_idx * (scale_chunk_size / 8) + (local_idx % scale_chunk_size) / 8)
                .unwrap_or(&0)
        )
    } else {
        "Grid".to_string()
    };

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
        bm.block_scale, bm.scales.get(tile_idx).unwrap_or(&0.0), grid_val_disp, chart_html
    ));

    let start = (global_idx / 8) * 8;
    let group_orig = &active_floats[start..start + 8];
    let group_q = &q_floats[start..start + 8];

    let mut snap_html = format!(
        r#"
        <div style="margin-top:12px; border-top:1px solid var(--border-color); padding-top:8px;">
            <div style="font-size:0.85rem; font-weight:bold; margin-bottom:4px;">Vector Snapping (Codebook Group)</div>
            <div style="display:flex; gap:4px;">
    "#
    );

    for i in 0..8 {
        let o_v = group_orig[i];
        let q_v = group_q[i];
        let is_hover = i == sub_group_idx;
        let bg = if is_hover {
            "background:var(--primary-color); color:white;"
        } else {
            "background:var(--bg-secondary);"
        };
        let arrow = if q_v.abs() > o_v.abs() {
            "&uarr;"
        } else {
            "&darr;"
        };

        snap_html.push_str(&format!(
            r#"
            <div style="flex:1; display:flex; flex-direction:column; align-items:center; border-radius:4px; padding:4px 0; {}">
                <span style="font-size:0.6rem; {}">{:.2}</span>
                <span style="font-size:0.7rem; font-weight:bold;">{}</span>
                <span style="font-size:0.65rem; font-weight:bold;">{:.2}</span>
            </div>
        "#,
            bg,
            if is_hover { "color:#ddd;" } else { "color:var(--text-muted);" },
            o_v,
            arrow,
            q_v
        ));
    }
    snap_html.push_str("</div></div>");
    iq_html.push_str(&snap_html);

    Some(iq_html)
}
