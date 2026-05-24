use crate::math_utils::*;
use crate::quants::{
    ImportanceResult, InspectorData, IqBlockMeta, QuantMeta, QuantizeOutput, Settings, lut
};

// Grid extraction helpers
fn get_grid_1bit(kgrid: &[u16]) -> Vec<[u8; 8]> {
    kgrid
        .iter()
        .map(|&val| {
            let mut g = [0u8; 8];
            for i in 0..8 {
                g[i] = ((val >> (2 * i)) & 0x3) as u8;
            }
            g
        })
        .collect()
}

fn get_grid_2bit(kgrid: &[u16]) -> Vec<[u8; 8]> {
    kgrid
        .iter()
        .map(|&val| {
            let mut g = [0u8; 8];
            for i in 0..8 {
                g[i] = ((val >> (2 * i)) & 0x3) as u8;
            }
            g
        })
        .collect()
}

fn get_grid_3bit(kgrid: &[u16]) -> Vec<[u8; 4]> {
    kgrid
        .iter()
        .map(|&val| {
            let mut g = [0u8; 4];
            for i in 0..4 {
                g[i] = ((val >> (3 * i)) & 0x7) as u8;
            }
            g
        })
        .collect()
}

fn get_iq1_s_grid() -> &'static [[u8; 8]] {
    static GRID: std::sync::OnceLock<Vec<[u8; 8]>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| get_grid_1bit(&lut::KGRID_1BIT_2048))
        .as_slice()
}

fn get_iq2_xxs_grid() -> &'static [[u8; 8]] {
    static GRID: std::sync::OnceLock<Vec<[u8; 8]>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| get_grid_2bit(&lut::KGRID_2BIT_256))
        .as_slice()
}

fn get_iq2_xs_grid() -> &'static [[u8; 8]] {
    static GRID: std::sync::OnceLock<Vec<[u8; 8]>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| get_grid_2bit(&lut::KGRID_2BIT_512))
        .as_slice()
}

fn get_iq2_s_grid() -> &'static [[u8; 8]] {
    static GRID: std::sync::OnceLock<Vec<[u8; 8]>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| get_grid_2bit(&lut::KGRID_2BIT_1024))
        .as_slice()
}

fn get_iq3_xxs_grid() -> &'static [[u8; 4]] {
    static GRID: std::sync::OnceLock<Vec<[u8; 4]>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| get_grid_3bit(&lut::KGRID_3BIT_256))
        .as_slice()
}

fn get_iq3_s_grid() -> &'static [[u8; 4]] {
    static GRID: std::sync::OnceLock<Vec<[u8; 4]>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| get_grid_3bit(&lut::KGRID_3BIT_512))
        .as_slice()
}

fn best_index_iq4nl(val: f32) -> usize {
    let mut best_idx = 0;
    let mut best_diff = f32::INFINITY;
    for (i, &v) in lut::KVALUES_IQ4NL.iter().enumerate() {
        let diff = (v - val).abs();
        if diff < best_diff {
            best_diff = diff;
            best_idx = i;
        }
    }
    best_idx
}

// Helper for exhaustive grid search to find optimal representation in codebooks
fn find_best_grid_idx(
    grid: &[[u8; 8]],
    xval: &[f32],
    waux: &[f32],
    scale: f32,
    x_mapped: &[f32],
) -> usize {
    let mut best_dist = f32::INFINITY;
    let mut best_idx = 0;
    for (g_idx, g_vals) in grid.iter().enumerate() {
        let mut dist = 0.0;
        for i in 0..8 {
            let q = x_mapped[g_vals[i] as usize];
            let diff = scale * q - xval[i];
            dist += waux[i] * diff * diff;
        }
        if dist < best_dist {
            best_dist = dist;
            best_idx = g_idx;
        }
    }
    best_idx
}

fn find_best_grid_idx_4(
    grid: &[[u8; 4]],
    xval: &[f32],
    waux: &[f32],
    scale: f32,
    x_mapped: &[f32],
) -> usize {
    let mut best_dist = f32::INFINITY;
    let mut best_idx = 0;
    for (g_idx, g_vals) in grid.iter().enumerate() {
        let mut dist = 0.0;
        for i in 0..4 {
            let q = x_mapped[g_vals[i] as usize];
            let diff = scale * q - xval[i];
            dist += waux[i] * diff * diff;
        }
        if dist < best_dist {
            best_dist = dist;
            best_idx = g_idx;
        }
    }
    best_idx
}

fn quantize_iq1_s_block(
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

    let mut max_scale = 0.0;

    for ib in 0..8 {
        let chunk = &super_block[ib * 32..(ib + 1) * 32];
        let w_chunk = &weights[ib * 32..(ib + 1) * 32];

        let mut max = 0.0_f32;
        for &v in chunk {
            if v.abs() > max {
                max = v.abs();
            }
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
                }
            }
        }

        if best_scale < 0.0 {
            best_scale = -best_scale;
            best_shift = -best_shift;
        }

        let xx = if best_shift == 1 { x_p } else { x_m };
        let mut waux = [0.0; 32];
        for i in 0..32 {
            waux[i] = w_chunk[i].sqrt();
        }

        let mut final_sumqx = 0.0;
        let mut final_sumq2 = 0.0;
        for k in 0..4 {
            let best_g = find_best_grid_idx(
                grid,
                &chunk[k * 8..k * 8 + 8],
                &waux[k * 8..k * 8 + 8],
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

fn quantize_iq1_m_block(
    super_block: &[f32],
    weights: &[f32],
    _iters: i32,
) -> (Vec<f32>, f32, Vec<f32>, Vec<i8>, Vec<usize>, Vec<u8>) {
    let mut q_block = vec![0.0; 256];
    let mut scales = [0.0; 16];
    let mut shifts = [0i8; 16];
    let mut tile_best_grids = [[0usize; 2]; 16];

    let grid = get_iq1_s_grid();
    let x_p = [-1.0 + 0.125, 0.125, 1.0 + 0.125];
    let x_m = [-1.0 - 0.125, -0.125, 1.0 - 0.125];

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

                for j in 0..16 {
                    let i = pairs[j].1;
                    let w = w_chunk[i];
                    let v = chunk[i];
                    let ww = w * v;

                    let (q0, q1, q2, q3) = if j < i1 {
                        if i < 8 {
                            (x_p[0], x_p[0], x_m[0], x_m[0])
                        } else {
                            (x_p[0], x_m[0], x_p[0], x_m[0])
                        }
                    } else if j < i2 {
                        if i < 8 {
                            (x_p[1], x_p[1], x_m[1], x_m[1])
                        } else {
                            (x_p[1], x_m[1], x_p[1], x_m[1])
                        }
                    } else {
                        if i < 8 {
                            (x_p[2], x_p[2], x_m[2], x_m[2])
                        } else {
                            (x_p[2], x_m[2], x_p[2], x_m[2])
                        }
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

        let mut waux = [0.0; 16];
        for i in 0..16 {
            waux[i] = w_chunk[i].sqrt();
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
                &waux[k * 8..k * 8 + 8],
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
    let d_out = fp16(d * 1.1125);
    let id = if d > 0.0 { 1.0 / d } else { 0.0 };

    for ib in 0..16 {
        let mut l = (0.5 * (id * scales[ib] - 1.0)).round() as i32;
        l = l.clamp(0, 7);
        let decoded_scale = d_out * (2.0 * l as f32 + 1.0);

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

fn quantize_iq2_xxs_block(
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
        let mut waux = [0.0; 32];
        for j in 0..32 {
            waux[j] = weights[ib * 32 + j].sqrt();
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
                    let cost = weights[ib * 32 + k * 8 + i] * block[ib * 32 + k * 8 + i].powi(2);
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
                    let w = weights[ib * 32 + k * 8 + i];
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
                    let w = weights[ib * 32 + k * 8 + i];
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

fn quantize_iq2_xs_block(
    block: &[f32],
    weights: &[f32],
    iters: i32,
) -> (Vec<f32>, f32, Vec<f32>, Vec<i8>, Vec<usize>, Vec<u8>) {
    let mut q_block = vec![0.0; 256];
    let mut scales = [0.0; 16];
    let mut tile_best_grids = [[0usize; 2]; 16];
    let mut tile_signs = [[0u8; 16]; 16];
    let grid = get_iq2_xs_grid();

    let mut max_scale = 0.0;
    let q_mapped = [1.0, 3.0, 5.0, 7.0];

    for ib in 0..16 {
        let mut xval = [0.0; 16];
        let mut waux = [0.0; 16];
        for j in 0..16 {
            waux[j] = weights[ib * 16 + j].sqrt();
        }

        let mut block_signs = [0u8; 2];
        for k in 0..2 {
            let mut nflip = 0;
            let mut s = 0u8;
            for i in 0..8 {
                let v = block[ib * 16 + k * 8 + i];
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
                    let cost = weights[ib * 16 + k * 8 + i] * block[ib * 16 + k * 8 + i].powi(2);
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
        let mut best_grids = [0usize; 2];

        for is in -iters..=iters {
            let id = (5.0 + is as f32 * 0.1) / max;
            let this_scale = 1.0 / id;

            let mut current_grids = [0usize; 2];
            let mut sumqx = 0.0;
            let mut sumq2 = 0.0;

            for k in 0..2 {
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
                    let w = weights[ib * 16 + k * 8 + i];
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
            for k in 0..2 {
                block_signs[k] = !block_signs[k];
            }
        }

        if best_scale > 0.0 {
            let mut final_sumqx = 0.0;
            let mut final_sumq2 = 0.0;
            let mut refined_grids = [0usize; 2];

            for k in 0..2 {
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
                    let w = weights[ib * 16 + k * 8 + i];
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

        for k in 0..2 {
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

    for ib in 0..16 {
        let mut l = (0.5 * (id * scales[ib] - 1.0)).round() as i32;
        l = l.clamp(0, 15);
        let decoded_scale = d_fp16 * (2.0 * l as f32 + 1.0);

        for k in 0..2 {
            let g_idx = tile_best_grids[ib][k];
            let g_vals = grid[g_idx];
            for i in 0..8 {
                let q = q_mapped[g_vals[i] as usize];
                let sign = if tile_signs[ib][k * 8 + i] == 1 {
                    -1.0
                } else {
                    1.0
                };
                q_block[ib * 16 + k * 8 + i] = decoded_scale * q * sign;
            }
        }
    }

    let mut out_grids = vec![];
    let mut out_signs = vec![];
    for ib in 0..16 {
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

fn quantize_iq2_s_block(
    block: &[f32],
    weights: &[f32],
    iters: i32,
) -> (Vec<f32>, f32, Vec<f32>, Vec<i8>, Vec<usize>, Vec<u8>) {
    let mut q_block = vec![0.0; 256];
    let mut scales = [0.0; 16];
    let mut tile_best_grids = [[0usize; 2]; 16];
    let mut tile_signs = [[0u8; 16]; 16];
    let grid = get_iq2_s_grid();

    let mut max_scale = 0.0;
    let q_mapped = [1.0, 3.0, 5.0, 7.0];

    for ib in 0..16 {
        let mut xval = [0.0; 16];
        let mut waux = [0.0; 16];
        for j in 0..16 {
            waux[j] = weights[ib * 16 + j].sqrt();
        }

        let mut block_signs = [0u8; 2];
        for k in 0..2 {
            let mut nflip = 0;
            let mut s = 0u8;
            for i in 0..8 {
                let v = block[ib * 16 + k * 8 + i];
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
                    let cost = weights[ib * 16 + k * 8 + i] * block[ib * 16 + k * 8 + i].powi(2);
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
        let mut best_grids = [0usize; 2];

        for is in -iters..=iters {
            let id = (5.0 + is as f32 * 0.1) / max;
            let this_scale = 1.0 / id;

            let mut current_grids = [0usize; 2];
            let mut sumqx = 0.0;
            let mut sumq2 = 0.0;

            for k in 0..2 {
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
                    let w = weights[ib * 16 + k * 8 + i];
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
            for k in 0..2 {
                block_signs[k] = !block_signs[k];
            }
        }

        if best_scale > 0.0 {
            let mut final_sumqx = 0.0;
            let mut final_sumq2 = 0.0;
            let mut refined_grids = [0usize; 2];

            for k in 0..2 {
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
                    let w = weights[ib * 16 + k * 8 + i];
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

        for k in 0..2 {
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
    let d_fp16 = fp16(d * 0.9875);
    let id = if d > 0.0 { 1.0 / d } else { 0.0 };

    for ib in 0..16 {
        let mut l = (0.5 * (id * scales[ib] - 1.0)).round() as i32;
        l = l.clamp(0, 15);
        let decoded_scale = d_fp16 * (2.0 * l as f32 + 1.0);

        for k in 0..2 {
            let g_idx = tile_best_grids[ib][k];
            let g_vals = grid[g_idx];
            for i in 0..8 {
                let q = q_mapped[g_vals[i] as usize];
                let sign = if tile_signs[ib][k * 8 + i] == 1 {
                    -1.0
                } else {
                    1.0
                };
                q_block[ib * 16 + k * 8 + i] = decoded_scale * q * sign;
            }
        }
    }

    let mut out_grids = vec![];
    let mut out_signs = vec![];
    for ib in 0..16 {
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

fn quantize_iq3_xxs_block(
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
        let mut waux = [0.0; 32];
        for j in 0..32 {
            waux[j] = weights[ib * 32 + j].sqrt();
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
                    let cost = weights[ib * 32 + k * 8 + i] * block[ib * 32 + k * 8 + i].powi(2);
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
                    &waux[k * 4..k * 4 + 4],
                    this_scale,
                    &q_mapped,
                );
                current_grids[k] = best_g;
                for i in 0..4 {
                    let q = q_mapped[grid[best_g][i] as usize];
                    let w = weights[ib * 32 + k * 4 + i];
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
                    &waux[k * 4..k * 4 + 4],
                    best_scale,
                    &q_mapped,
                );
                refined_grids[k] = best_g;
                for i in 0..4 {
                    let q = q_mapped[grid[best_g][i] as usize];
                    let w = weights[ib * 32 + k * 4 + i];
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

fn quantize_iq3_s_block(
    block: &[f32],
    weights: &[f32],
    iters: i32,
) -> (Vec<f32>, f32, Vec<f32>, Vec<i8>, Vec<usize>, Vec<u8>) {
    let mut q_block = vec![0.0; 256];
    let mut scales = [0.0; 8];
    let mut tile_best_grids = [[0usize; 8]; 8];
    let mut tile_signs = [[0u8; 32]; 8];
    let grid = get_iq3_s_grid();

    let mut max_scale = 0.0;
    let q_mapped = [1.0, 3.0, 5.0, 7.0, 9.0, 11.0, 13.0, 15.0];

    for ib in 0..8 {
        let mut xval = [0.0; 32];
        let mut waux = [0.0; 32];
        for j in 0..32 {
            waux[j] = weights[ib * 32 + j].sqrt();
        }

        let mut block_signs = [0u8; 4];
        for k in 0..4 {
            let mut s = 0u8;
            for i in 0..8 {
                let v = block[ib * 32 + k * 8 + i];
                if v >= 0.0 {
                    xval[k * 8 + i] = v;
                } else {
                    xval[k * 8 + i] = -v;
                    s |= 1 << i;
                }
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
                    &waux[k * 4..k * 4 + 4],
                    this_scale,
                    &q_mapped,
                );
                current_grids[k] = best_g;
                for i in 0..4 {
                    let q = q_mapped[grid[best_g][i] as usize];
                    let w = weights[ib * 32 + k * 4 + i];
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
                    &waux[k * 4..k * 4 + 4],
                    best_scale,
                    &q_mapped,
                );
                refined_grids[k] = best_g;
                for i in 0..4 {
                    let q = q_mapped[grid[best_g][i] as usize];
                    let w = weights[ib * 32 + k * 4 + i];
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
    let d_out = fp16(d * 1.033);
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

fn quantize_iq4_xs_block(
    super_block: &[f32],
    weights: &[f32],
    iters: i32,
) -> (Vec<f32>, f32, Vec<f32>, Vec<i8>, Vec<usize>, Vec<u8>) {
    let mut q_block = vec![0.0; 256];
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

        let mut best_scale = 0.0;
        let mut best_score = -1.0;

        for is in -iters..=iters {
            let id = (is as f32 + lut::KVALUES_IQ4NL[0].abs()) / max;
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

        scales[ib] = best_scale;
        if best_scale.abs() > amax_scale {
            amax_scale = best_scale.abs();
            max_scale = best_scale;
        }
    }

    if amax_scale == 0.0 {
        return (q_block, 0.0, vec![], vec![], vec![], vec![]);
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

        let chunk = &super_block[ib * 32..(ib + 1) * 32];
        for i in 0..32 {
            let al = idl * chunk[i];
            let q_idx = best_index_iq4nl(al);
            q_block[ib * 32 + i] = decoded_scale * lut::KVALUES_IQ4NL[q_idx];
        }
    }

    (q_block, d_out, scales.to_vec(), vec![], vec![], vec![])
}

fn quantize_iq4_nl_block(
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
        let id = (is as f32 + lut::KVALUES_IQ4NL[0].abs()) / max;

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

pub fn quantize(
    floats: &[f32],
    importance: Option<&ImportanceResult>,
    settings: &Settings,
) -> QuantizeOutput {
    let n = floats.len();
    let mut q_floats = vec![0.0; n];
    let mut blocks = Vec::new();

    let block_size = if settings.iq_type == "iq4_nl" {
        32
    } else {
        256
    };

    let bpw = match settings.iq_type.as_str() {
        "iq1_s" => 1.5625,
        "iq1_m" => 1.75,
        "iq2_xxs" => 2.0625,
        "iq2_xs" => 2.3125,
        "iq2_s" => 2.5625,
        "iq3_xxs" => 3.0625,
        "iq3_s" => 3.4375,
        "iq4_xs" => 4.25,
        "iq4_nl" => 4.5,
        _ => 2.0625,
    };

    let iters = settings.iq_scale_iters as i32;

    let max_pad_len = block_size;
    let mut chunk_padded = vec![0.0; max_pad_len];
    let mut weights = vec![0.0; max_pad_len];

    for b in (0..n).step_by(block_size) {
        let chunk_len = (n - b).min(block_size);
        let chunk = &floats[b..b + chunk_len];

        chunk_padded[..chunk_len].copy_from_slice(chunk);
        chunk_padded[chunk_len..max_pad_len].fill(0.0);

        let mut sum_sq = 0.0;
        for &v in chunk {
            sum_sq += (v * v) as f64;
        }
        let variance = 2.0 * sum_sq / chunk_len.max(1) as f64;

        if let Some(imp) = importance {
            for i in 0..chunk_len {
                let m_i = imp.raw[b + i] as f64;
                weights[i] = (m_i * (variance + (chunk[i] * chunk[i]) as f64).sqrt()) as f32;
            }
        } else {
            for i in 0..chunk_len {
                weights[i] = chunk[i] * chunk[i];
            }
        }
        weights[chunk_len..max_pad_len].fill(0.0);

        let (q_block, block_scale, scales, aux8, grids, signs) = match settings.iq_type.as_str() {
            "iq1_s" => quantize_iq1_s_block(&chunk_padded, &weights, iters),
            "iq1_m" => quantize_iq1_m_block(&chunk_padded, &weights, iters),
            "iq2_xxs" => quantize_iq2_xxs_block(&chunk_padded, &weights, iters),
            "iq2_xs" => quantize_iq2_xs_block(&chunk_padded, &weights, iters),
            "iq2_s" => quantize_iq2_s_block(&chunk_padded, &weights, iters),
            "iq3_xxs" => quantize_iq3_xxs_block(&chunk_padded, &weights, iters),
            "iq3_s" => quantize_iq3_s_block(&chunk_padded, &weights, iters),
            "iq4_xs" => quantize_iq4_xs_block(&chunk_padded, &weights, iters),
            "iq4_nl" => quantize_iq4_nl_block(&chunk_padded, &weights, iters),
            _ => quantize_iq2_xxs_block(&chunk_padded, &weights, iters),
        };

        for i in 0..chunk_len {
            q_floats[b + i] = q_block[i];
        }

        let (mse, mae) = get_err_stats(chunk, &q_block[..chunk_len]);
        blocks.push(IqBlockMeta {
            idx: b / block_size,
            size: chunk_len,
            block_scale,
            mse,
            mae,
            scales,
            aux8,
            grids,
            signs,
        });
    }

    let formula_html = match settings.iq_type.as_str() {
        "iq1_s" => "<span>Weight = ( <span class=\"eq-pill\">Codebook<span class=\"bits\">1.5b</span></span> &plusmn; <span title=\"Adaptive shift offset (+0.125 or -0.125) evaluated per tile to optimally center the grid vector distribution.\" style=\"border-bottom: 1px dotted var(--text-muted); cursor: help;\">0.125</span> ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">3b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Groups of 8 are snapped to a 3-state {-1, 0, 1} vector grid, utilizing an adaptive sign-shift offset.</span>",
        "iq1_m" => "<span>Weight = ( <span class=\"eq-pill\">Codebook<span class=\"bits\">1.5b</span></span> &plusmn; <span title=\"Adaptive shift offset evaluated per tile to optimally center the grid vector distribution.\" style=\"border-bottom: 1px dotted var(--text-muted); cursor: help;\">0.125</span> ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">3b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">16-element sub-blocks with exhaustive SSD search over 4 configuration permutations.</span>",
        "iq2_xxs" => "<span>Weight = <span class=\"eq-pill\">Sign<span class=\"bits\">1b</span></span> &times; ( 2 &times; <span class=\"eq-pill\">GridVal<span class=\"bits\">2b</span></span> + 1 ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">4b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Snaps groups of 8 to a 2-bit coordinate grid, enforcing strict even-sign-parity to save storage.</span>",
        "iq2_xs" => "<span>Weight = <span class=\"eq-pill\">Sign<span class=\"bits\">1b</span></span> &times; ( 2 &times; <span class=\"eq-pill\">GridVal<span class=\"bits\">2b</span></span> + 1 ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">4b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">16-element sub-blocks, 512-entry 2-bit coordinate grid, enforcing even-sign-parity constraints.</span>",
        "iq2_s" => "<span>Weight = <span class=\"eq-pill\">Sign<span class=\"bits\">1b</span></span> &times; ( 2 &times; <span class=\"eq-pill\">GridVal<span class=\"bits\">2b</span></span> + 1 ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">4b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">16-element sub-blocks, 1024-entry 2-bit coordinate grid, enforcing even-sign-parity constraints.</span>",
        "iq3_xxs" => "<span>Weight = <span class=\"eq-pill\">Sign<span class=\"bits\">1b</span></span> &times; ( 2 &times; <span class=\"eq-pill\">GridVal<span class=\"bits\">3b</span></span> + 1 ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">4b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Snaps groups of 4 to a 3-bit coordinate grid, sharing sign-parity constraints across dual pairs.</span>",
        "iq3_s" => "<span>Weight = <span class=\"eq-pill\">Sign<span class=\"bits\">1b</span></span> &times; ( 2 &times; <span class=\"eq-pill\">GridVal<span class=\"bits\">3b</span></span> + 1 ) &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">4b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">32-element sub-blocks, 512-entry 3-bit coordinate grid, sharing sign-parity constraints across dual pairs.</span>",
        "iq4_xs" => "<span>Weight = <span class=\"eq-pill\">NonLinearVal<span class=\"bits\">4b</span></span> &times; ( <span class=\"eq-pill\">SuperScale<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">TileScale<span class=\"bits\">6b</span></span> )</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Maps 32-weight blocks to 4-bit non-linear logarithmic curves, scaled by hierarchical 6-bit step-sizes.</span>",
        "iq4_nl" => "<span>Weight = <span class=\"eq-pill\">NonLinearVal<span class=\"bits\">4b</span></span> &times; <span class=\"eq-pill\">BlockScale<span class=\"bits\">16b</span></span></span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Direct 4-bit non-linear quantization with standalone 16-bit scales, optimized for Gaussian outlier distributions.</span>",
        _ => "",
    }.to_string();

    QuantizeOutput {
        q_floats,
        t_floats: None,
        t_q_floats: None,
        bpw,
        formula_html,
        block_size,
        super_block_size: 0,
        meta: QuantMeta::Iq(blocks),
    }
}

pub fn format_inspector(
    idx: usize,
    active: &[f32],
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::Iq(blocks) = &out.meta else {
        return InspectorData::default();
    };
    let block_size = out.block_size;
    let b_idx = idx / block_size;
    let mut data = InspectorData::default();

    if let Some(bm) = blocks.get(b_idx) {
        let q_val = out.q_floats[idx];

        let local_idx = idx % block_size;
        let scale_chunk_size = if bm.scales.len() > 0 {
            block_size / bm.scales.len()
        } else {
            32
        };
        let tile_idx = local_idx / scale_chunk_size;
        let sub_group_idx = local_idx % 8;

        let sign_char = if q_val < 0.0 { "-" } else { "+" };
        data.math_str = Some(format!("{} {:.4} (Grid mapped)", sign_char, q_val.abs()));

        let mut iq_html = String::new();

        if settings.iq_type == "iq4_nl" {
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

            iq_html.push_str(&format!(
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
                scale_val, lut::KVALUES_IQ4NL[active_centroid_idx], active_centroid_idx, distribution_html, active_centroid_idx
            ));
        } else {
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

            let grid_val_disp = if settings.iq_type.starts_with("iq3_") {
                let g1 = bm
                    .grids
                    .get(
                        tile_idx * (scale_chunk_size / 4)
                            + ((local_idx % scale_chunk_size) / 8) * 2,
                    )
                    .unwrap_or(&0);
                let g2 = bm
                    .grids
                    .get(
                        tile_idx * (scale_chunk_size / 4)
                            + ((local_idx % scale_chunk_size) / 8) * 2
                            + 1,
                    )
                    .unwrap_or(&0);
                format!("#{} & #{}", g1, g2)
            } else if !bm.grids.is_empty() {
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
        }

        if settings.iq_type == "iq1_s"
            || settings.iq_type == "iq1_m"
            || settings.iq_type == "iq2_xxs"
            || settings.iq_type == "iq2_xs"
            || settings.iq_type == "iq2_s"
            || settings.iq_type == "iq3_xxs"
            || settings.iq_type == "iq3_s"
        {
            let start = (idx / 8) * 8;
            let group_orig = &active[start..start + 8];
            let group_q = &out.q_floats[start..start + 8];

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

                snap_html.push_str(&format!(r#"
                    <div style="flex:1; display:flex; flex-direction:column; align-items:center; border-radius:4px; padding:4px 0; {}">
                        <span style="font-size:0.6rem; {}">{:.2}</span>
                        <span style="font-size:0.7rem; font-weight:bold;">{}</span>
                        <span style="font-size:0.65rem; font-weight:bold;">{:.2}</span>
                    </div>
                "#, bg, if is_hover { "color:#ddd;" } else { "color:var(--text-muted);" }, o_v, arrow, q_v));
            }
            snap_html.push_str("</div></div>");
            iq_html.push_str(&snap_html);

            if settings.iq_type.starts_with("iq2_") || settings.iq_type.starts_with("iq3_") {
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

                    parity_html.push_str(&format!(r#"
                        <div style="flex:1; text-align:center; padding:2px; border-radius:4px; background:{}; color:white; font-weight:bold; {}">
                            {}
                        </div>
                    "#, bg, border, sign_char));
                }

                let text = if parity_violation {
                    format!(
                        "Parity Violation! Flipped sign of element #{} to satisfy even parity.",
                        flip_idx + 1
                    )
                } else {
                    "Even negatives. Parity satisfied.".to_string()
                };

                parity_html.push_str(&format!(r#"
                        </div>
                        <div style="font-size:0.75rem; margin-top:6px; color:var(--text-muted); line-height:1.2; min-height:2.4em; display:flex; align-items:center;">
                            {}
                        </div>
                    </div>
                "#, text));

                iq_html.push_str(&parity_html);
            }
        }

        data.iq_html = if iq_html.is_empty() {
            None
        } else {
            Some(iq_html)
        };

        data.block_idx = Some(bm.idx);
        data.scale = Some(bm.block_scale);
        data.mse = Some(bm.mse);
        data.mae = Some(bm.mae);
    }
    data
}
