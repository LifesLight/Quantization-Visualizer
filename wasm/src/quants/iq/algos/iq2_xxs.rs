use super::{find_best_grid_idx, get_grid_2bit};
use crate::math_utils::*;
use crate::quants::iq::lut;
use crate::quants::IqBlockMeta;

pub fn block_size() -> usize {
    256
}

pub fn bpw() -> f32 {
    2.0625
}

fn get_iq2_xxs_grid() -> &'static [[u8; 8]] {
    static GRID: std::sync::OnceLock<Vec<[u8; 8]>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| get_grid_2bit(&lut::KGRID_2BIT_256))
        .as_slice()
}

pub fn quantize_block(
    block: &[f32],
    weights: &[f32],
    iters: i32,
) -> (Vec<f32>, f32, Vec<f32>, Vec<i8>, Vec<usize>, Vec<u8>) {
    let mut q_block = vec![0.0; 256];
    let mut scales = [0.0; 8];
    let mut tile_best_grids = [[0usize; 4]; 8];
    let mut tile_signs = [[0u8; 32]; 8];
    let grid = get_iq2_xxs_grid();

    let mut max_scale = 0.0;
    let q_mapped = [1.0, 3.0, 5.0, 7.0];

    for ib in 0..8 {
        let mut xval = [0.0; 32];
        let w_chunk = &weights[ib * 32..ib * 32 + 32];

        let mut waux = [0.0; 32];
        for i in 0..32 {
            waux[i] = w_chunk[i].sqrt();
        }

        let mut block_signs = [0u8; 4];
        for k in 0..4 {
            let mut nflip = 0;
            let mut s = 0u8;
            for i in 0..8 {
                let v = block[ib * 32 + k * 8 + i];
                if v >= 0.0 {
                    xval[k * 8 + i] = v;
                } else {
                    xval[k * 8 + i] = -v;
                    nflip += 1;
                    s |= 1 << i;
                }
            }
            if nflip % 2 != 0 {
                let mut min_cost = f32::INFINITY;
                let mut imin = 0;
                for i in 0..8 {
                    let cost = w_chunk[k * 8 + i] * block[ib * 32 + k * 8 + i].powi(2);
                    if cost < min_cost {
                        min_cost = cost;
                        imin = i;
                    }
                }
                xval[k * 8 + imin] = -xval[k * 8 + imin];
                s ^= 1 << imin;
            }
            block_signs[k] = s;
        }

        let max = xval.iter().cloned().fold(0.0_f32, f32::max);
        if max < 1e-6 {
            scales[ib] = 0.0;
            continue;
        }

        let mut best_scale = 0.0;
        let mut best_score = -1.0;
        let mut best_grids = [0usize; 4];

        for is in -iters..=iters {
            let id = (5.0 + is as f32 * 0.1) / max;
            let this_scale = 1.0 / id;

            let mut current_grids = [0usize; 4];
            let mut sumqx = 0.0;
            let mut sumq2 = 0.0;

            for k in 0..4 {
                let best_g = find_best_grid_idx(
                    grid,
                    &xval[k * 8..k * 8 + 8],
                    &waux[k * 8..k * 8 + 8],
                    this_scale,
                    &q_mapped,
                );
                current_grids[k] = best_g;
                for i in 0..8 {
                    let q = q_mapped[grid[best_g][i] as usize];
                    let w = w_chunk[k * 8 + i];
                    sumqx += w * xval[k * 8 + i] * q;
                    sumq2 += w * q * q;
                }
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
                best_grids = current_grids;
            }
        }

        if best_scale < 0.0 {
            best_scale = -best_scale;
            for k in 0..4 {
                block_signs[k] = !block_signs[k];
            }
        }

        // Final Grid Refinement
        if best_scale > 0.0 {
            let mut final_sumqx = 0.0;
            let mut final_sumq2 = 0.0;
            let mut refined_grids = [0usize; 4];

            for k in 0..4 {
                let best_g = find_best_grid_idx(
                    grid,
                    &xval[k * 8..k * 8 + 8],
                    &waux[k * 8..k * 8 + 8],
                    best_scale,
                    &q_mapped,
                );
                refined_grids[k] = best_g;
                for i in 0..8 {
                    let q = q_mapped[grid[best_g][i] as usize];
                    let w = w_chunk[k * 8 + i];
                    final_sumqx += w * xval[k * 8 + i] * q;
                    final_sumq2 += w * q * q;
                }
            }
            if final_sumq2 > 0.0 {
                best_scale = final_sumqx / final_sumq2;
            }
            tile_best_grids[ib] = refined_grids;
        } else {
            tile_best_grids[ib] = best_grids;
        }

        scales[ib] = best_scale;

        for k in 0..4 {
            for i in 0..8 {
                tile_signs[ib][k * 8 + i] = if (block_signs[k] & (1 << i)) != 0 {
                    1
                } else {
                    0
                };
            }
        }

        if best_scale > max_scale {
            max_scale = best_scale;
        }
    }

    if max_scale == 0.0 {
        return (q_block, 0.0, vec![], vec![], vec![], vec![]);
    }

    let d = max_scale / 31.0;
    let d_fp16 = fp16(d);
    let id = if d > 0.0 { 1.0 / d } else { 0.0 };

    for ib in 0..8 {
        let mut l = (0.5 * (id * scales[ib] - 1.0)).round() as i32;
        l = l.clamp(0, 15);
        let decoded_scale = d_fp16 * (2.0 * l as f32 + 1.0);

        for k in 0..4 {
            let g_idx = tile_best_grids[ib][k];
            let g_vals = grid[g_idx];
            for i in 0..8 {
                let q = q_mapped[g_vals[i] as usize];
                let sign = if tile_signs[ib][k * 8 + i] == 1 {
                    -1.0
                } else {
                    1.0
                };
                q_block[ib * 32 + k * 8 + i] = decoded_scale * q * sign;
            }
        }
    }

    let mut out_grids = vec![];
    let mut out_signs = vec![];
    for ib in 0..8 {
        out_grids.extend_from_slice(&tile_best_grids[ib]);
        out_signs.extend_from_slice(&tile_signs[ib]);
    }

    (
        q_block,
        d_fp16,
        scales.to_vec(),
        vec![],
        out_grids,
        out_signs,
    )
}

pub fn formula_html() -> &'static str {
    "<span>Weight = <span class=\"eq-pill\">Sign<span class=\"bits\">1b</span></span> &times; ( 2 &times; <span class=\"eq-pill\">GridVal<span class=\"bits\">2b</span></span> + 1 ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">4b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Snaps groups of 8 to a 2-bit coordinate grid, enforcing strict even-sign-parity to save storage.</span>"
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

    let mut orig_neg = 0;
    let mut parity_violation = false;
    let mut flip_idx = 8;

    for i in 0..8 {
        if group_orig[i] < 0.0 {
            orig_neg += 1;
        }
    }

    if orig_neg % 2 != 0 {
        parity_violation = true;
        for i in 0..8 {
            let o_sign = group_orig[i] < 0.0;
            let q_sign = group_q[i] < 0.0;
            if o_sign != q_sign {
                flip_idx = i;
                break;
            }
        }
    }

    let mut parity_html = format!(
        r#"
        <div style="margin-top:12px; border-top:1px solid var(--border-color); padding-top:8px;">
            <div style="font-size:0.85rem; font-weight:bold; margin-bottom:4px;">Sign Parity Enforcement</div>
            <div style="display:flex; gap:4px;">
    "#
    );

    for i in 0..8 {
        let q_sign = group_q[i] < 0.0;
        let o_sign = group_orig[i] < 0.0;

        let mut is_flipped = i == flip_idx;
        if parity_violation && flip_idx == 8 && o_sign != q_sign {
            is_flipped = true;
        }

        let bg = if q_sign {
            "var(--negative-color)"
        } else {
            "var(--accent-color)"
        };
        let sign_char = if q_sign { "-" } else { "+" };
        let border = if is_flipped {
            "border:2px solid #fbbf24; transform:scale(1.1);"
        } else {
            "border:2px solid transparent;"
        };

        parity_html.push_str(&format!(
            r#"
            <div style="flex:1; text-align:center; padding:2px; border-radius:4px; background:{}; color:white; font-weight:bold; {}">
                {}
            </div>
        "#,
            bg, border, sign_char
        ));
    }

    let text = if parity_violation {
        format!(
            "Parity Violation! Flipped sign of element #{} to satisfy even parity.",
            flip_idx + 1
        )
    } else {
        "Even negatives. Parity satisfied.".to_string()
    };

    parity_html.push_str(&format!(
        r#"
            </div>
            <div style="font-size:0.75rem; margin-top:6px; color:var(--text-muted); line-height:1.2; min-height:2.4em; display:flex; align-items:center;">
                {}
            </div>
        </div>
    "#,
        text
    ));

    iq_html.push_str(&parity_html);

    Some(iq_html)
}
