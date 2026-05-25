use super::{find_best_grid_idx_4, get_grid_3bit};
use crate::math_utils::*;
use crate::quants::iq::lut;
use crate::quants::IqBlockMeta;

pub fn block_size() -> usize {
    256
}

pub fn bpw() -> f32 {
    3.0625
}

fn get_iq3_xxs_grid() -> &'static [[u8; 4]] {
    static GRID: std::sync::OnceLock<Vec<[u8; 4]>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| get_grid_3bit(&lut::KGRID_3BIT_256))
        .as_slice()
}

pub fn quantize_block(
    block: &[f32],
    weights: &[f32],
    iters: i32,
) -> (Vec<f32>, f32, Vec<f32>, Vec<i8>, Vec<usize>, Vec<u8>) {
    let mut q_block = vec![0.0; 256];
    let mut scales = [0.0; 8];
    let mut tile_best_grids = [[0usize; 8]; 8];
    let mut tile_signs = [[0u8; 32]; 8];
    let grid = get_iq3_xxs_grid();

    let mut max_scale = 0.0;
    let q_mapped = [1.0, 3.0, 5.0, 7.0, 9.0, 11.0, 13.0, 15.0];

    for ib in 0..8 {
        let mut xval = [0.0; 32];
        let w_chunk = &weights[ib * 32..ib * 32 + 32];

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
        let mut best_grids = [0usize; 8];

        for is in -iters..=iters {
            let id = (15.0 + is as f32 * 0.2) / max;
            let this_scale = 1.0 / id;

            let mut current_grids = [0usize; 8];
            let mut sumqx = 0.0;
            let mut sumq2 = 0.0;

            for k in 0..8 {
                let best_g = find_best_grid_idx_4(
                    grid,
                    &xval[k * 4..k * 4 + 4],
                    &w_chunk[k * 4..k * 4 + 4],
                    this_scale,
                    &q_mapped,
                );
                current_grids[k] = best_g;
                for i in 0..4 {
                    let q = q_mapped[grid[best_g][i] as usize];
                    let w = w_chunk[k * 4 + i];
                    sumqx += w * xval[k * 4 + i] * q;
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

        if best_scale > 0.0 {
            let mut final_sumqx = 0.0;
            let mut final_sumq2 = 0.0;
            let mut refined_grids = [0usize; 8];

            for k in 0..8 {
                let best_g = find_best_grid_idx_4(
                    grid,
                    &xval[k * 4..k * 4 + 4],
                    &w_chunk[k * 4..k * 4 + 4],
                    best_scale,
                    &q_mapped,
                );
                refined_grids[k] = best_g;
                for i in 0..4 {
                    let q = q_mapped[grid[best_g][i] as usize];
                    let w = w_chunk[k * 4 + i];
                    final_sumqx += w * xval[k * 4 + i] * q;
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
    let d_out = fp16(d * 1.0125);
    let id = if d > 0.0 { 1.0 / d } else { 0.0 };

    for ib in 0..8 {
        let mut l = (0.5 * (id * scales[ib] - 1.0)).round() as i32;
        l = l.clamp(0, 15);
        let decoded_scale = d_out * (2.0 * l as f32 + 1.0);

        for k in 0..8 {
            let g_idx = tile_best_grids[ib][k];
            let g_vals = grid[g_idx];
            for i in 0..4 {
                let q = q_mapped[g_vals[i] as usize];
                let sign = if tile_signs[ib][(k / 2) * 8 + (k % 2) * 4 + i] == 1 {
                    -1.0
                } else {
                    1.0
                };
                q_block[ib * 32 + k * 4 + i] = decoded_scale * q * sign;
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
        d_out,
        scales.to_vec(),
        vec![],
        out_grids,
        out_signs,
    )
}

pub fn formula_html() -> &'static str {
    "<span>Weight = <span class=\"eq-pill\">Sign<span class=\"bits\">1b</span></span> &times; ( 2 &times; <span class=\"eq-pill\">GridVal<span class=\"bits\">3b</span></span> + 1 ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">4b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Snaps groups of 4 to a 3-bit coordinate grid, sharing sign-parity constraints across dual pairs.</span>"
}

pub fn format_inspector(
    global_idx: usize,
    active_floats: &[f32],
    q_floats: &[f32],
    bm: &IqBlockMeta,
) -> Option<String> {
    crate::quants::iq::algos::iq3_s::format_inspector(global_idx, active_floats, q_floats, bm)
}
