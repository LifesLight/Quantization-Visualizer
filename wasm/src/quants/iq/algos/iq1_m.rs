use super::{find_best_grid_idx, get_grid_1bit};
use crate::math_utils::*;
use crate::quants::iq::lut;
use crate::quants::IqBlockMeta;

pub fn block_size() -> usize {
    256
}

pub fn bpw() -> f32 {
    1.75
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
    let mut scales = [0.0; 16];
    let mut shifts = [0i8; 16];
    let mut tile_best_grids = [[0usize; 2]; 16];

    let grid = get_iq1_s_grid();
    let x_p = [-1.0 + 0.125, 0.125, 1.0 + 0.125, 0.0];
    let x_m = [-1.0 - 0.125, -0.125, 1.0 - 0.125, 0.0];

    let mut max_scale = 0.0;

    for ib in 0..16 {
        let chunk = &super_block[ib * 16..(ib + 1) * 16];
        let w_chunk = &weights[ib * 16..(ib + 1) * 16];

        let mut max = 0.0_f32;
        for &v in chunk {
            if v.abs() > max {
                max = v.abs();
            }
        }
        if max < 1e-6 {
            scales[ib] = 0.0;
            shifts[ib] = 0;
            continue;
        }

        let mut pairs: Vec<(f32, usize)> = chunk
            .iter()
            .copied()
            .enumerate()
            .map(|(i, v)| (v, i))
            .collect();
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let mut best_score = -f32::MAX;
        let mut best_scale = max;
        let mut best_k = 0;

        for i1 in 0..=16 {
            for i2 in i1..=16 {
                let mut sumqx = [0.0; 4];
                let mut sumq2 = [0.0; 4];

                for j in 0..i1 {
                    let i = pairs[j].1;
                    let w = w_chunk[i];
                    let ww = w * chunk[i];

                    let (q0, q1, q2, q3) = if i < 8 {
                        (x_p[0], x_p[0], x_m[0], x_m[0])
                    } else {
                        (x_p[0], x_m[0], x_p[0], x_m[0])
                    };

                    sumqx[0] += ww * q0;
                    sumqx[1] += ww * q1;
                    sumqx[2] += ww * q2;
                    sumqx[3] += ww * q3;

                    sumq2[0] += w * q0 * q0;
                    sumq2[1] += w * q1 * q1;
                    sumq2[2] += w * q2 * q2;
                    sumq2[3] += w * q3 * q3;
                }

                for j in i1..i2 {
                    let i = pairs[j].1;
                    let w = w_chunk[i];
                    let ww = w * chunk[i];

                    let (q0, q1, q2, q3) = if i < 8 {
                        (x_p[1], x_p[1], x_m[1], x_m[1])
                    } else {
                        (x_p[1], x_m[1], x_p[1], x_m[1])
                    };

                    sumqx[0] += ww * q0;
                    sumqx[1] += ww * q1;
                    sumqx[2] += ww * q2;
                    sumqx[3] += ww * q3;

                    sumq2[0] += w * q0 * q0;
                    sumq2[1] += w * q1 * q1;
                    sumq2[2] += w * q2 * q2;
                    sumq2[3] += w * q3 * q3;
                }

                for j in i2..16 {
                    let i = pairs[j].1;
                    let w = w_chunk[i];
                    let ww = w * chunk[i];

                    let (q0, q1, q2, q3) = if i < 8 {
                        (x_p[2], x_p[2], x_m[2], x_m[2])
                    } else {
                        (x_p[2], x_m[2], x_p[2], x_m[2])
                    };

                    sumqx[0] += ww * q0;
                    sumqx[1] += ww * q1;
                    sumqx[2] += ww * q2;
                    sumqx[3] += ww * q3;

                    sumq2[0] += w * q0 * q0;
                    sumq2[1] += w * q1 * q1;
                    sumq2[2] += w * q2 * q2;
                    sumq2[3] += w * q3 * q3;
                }

                for k in 0..4 {
                    if sumq2[k] > 0.0 && sumqx[k] * sumqx[k] > best_score * sumq2[k] {
                        best_scale = sumqx[k] / sumq2[k];
                        best_score = best_scale * sumqx[k];
                        best_k = k;
                    }
                }
            }
        }

        if best_scale < 0.0 {
            best_scale = -best_scale;
            best_k = match best_k {
                0 => 3,
                1 => 2,
                2 => 1,
                _ => 0,
            };
        }

        let mut final_sumqx = 0.0;
        let mut final_sumq2 = 0.0;

        for k in 0..2 {
            let xx = if k == 0 {
                if best_k < 2 {
                    x_p
                } else {
                    x_m
                }
            } else {
                if best_k % 2 == 0 {
                    x_p
                } else {
                    x_m
                }
            };

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

        if final_sumq2 > 0.0 {
            best_scale = final_sumqx / final_sumq2;
        }

        scales[ib] = best_scale;
        shifts[ib] = best_k as i8;
        if best_scale > max_scale {
            max_scale = best_scale;
        }
    }

    if max_scale == 0.0 {
        return (q_block, 0.0, vec![], vec![], vec![], vec![]);
    }

    let d = max_scale / 15.0;
    let id = if d > 0.0 { 1.0 / d } else { 0.0 };
    let mut sumqx_f = 0.0;
    let mut sumq2_f = 0.0;
    let mut ls = [0i32; 16];

    for ib in 0..16 {
        let mut l = (0.5 * (id * scales[ib] - 1.0)).round() as i32;
        l = l.clamp(0, 7);
        ls[ib] = l;

        let w_chunk = &weights[ib * 16..(ib + 1) * 16];
        let chunk = &super_block[ib * 16..(ib + 1) * 16];

        for k in 0..2 {
            let xx = if k == 0 {
                if shifts[ib] < 2 {
                    x_p
                } else {
                    x_m
                }
            } else {
                if shifts[ib] % 2 == 0 {
                    x_p
                } else {
                    x_m
                }
            };

            let g_idx = tile_best_grids[ib][k];
            for i in 0..8 {
                let q = xx[grid[g_idx][i] as usize] * (2.0 * l as f32 + 1.0);
                let w = w_chunk[k * 8 + i];
                sumqx_f += w * q * chunk[k * 8 + i];
                sumq2_f += w * q * q;
            }
        }
    }

    let d_final = if sumq2_f > 0.0 { sumqx_f / sumq2_f } else { d };
    let d_out = fp16(d_final * 1.1125);

    for ib in 0..16 {
        let l_scale = 2.0 * ls[ib] as f32 + 1.0;
        let decoded_scale = d_out * l_scale;
        scales[ib] = l_scale;

        for k in 0..2 {
            let xx = if k == 0 {
                if shifts[ib] < 2 {
                    x_p
                } else {
                    x_m
                }
            } else {
                if shifts[ib] % 2 == 0 {
                    x_p
                } else {
                    x_m
                }
            };

            let g_idx = tile_best_grids[ib][k];
            for i in 0..8 {
                let q = xx[grid[g_idx][i] as usize];
                q_block[ib * 16 + k * 8 + i] = decoded_scale * q;
            }
        }
    }

    let mut out_grids = vec![];
    for ib in 0..16 {
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
    "<span>Weight = ( <span class=\"eq-pill\">Codebook<span class=\"bits\">1.5b</span></span> &plusmn; <span title=\"Adaptive shift offset evaluated per tile to optimally center the grid vector distribution.\" style=\"border-bottom: 1px dotted var(--text-muted); cursor: help;\">0.125</span> ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">3b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">16-element sub-blocks with exhaustive SSD search over 4 configuration permutations.</span>"
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
