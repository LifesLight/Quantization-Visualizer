pub mod iq1_m;
pub mod iq1_s;
pub mod iq2_s;
pub mod iq2_xs;
pub mod iq2_xxs;
pub mod iq3_s;
pub mod iq3_xxs;
pub mod iq4_nl;
pub mod iq4_xs;

use crate::quants::iq::lut;

// Grid extraction helpers
pub(crate) fn get_grid_1bit(kgrid: &[u16]) -> Vec<[u8; 8]> {
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

pub(crate) fn get_grid_2bit(kgrid: &[u16]) -> Vec<[u8; 8]> {
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

pub(crate) fn get_grid_3bit(kgrid: &[u16]) -> Vec<[u8; 4]> {
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

pub(crate) fn best_index_iq4nl(val: f32) -> usize {
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
pub(crate) fn find_best_grid_idx(
    grid: &[[u8; 8]],
    xval: &[f32],
    weights: &[f32],
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
            dist += weights[i] * diff * diff;
        }
        if dist < best_dist {
            best_dist = dist;
            best_idx = g_idx;
        }
    }
    best_idx
}

pub(crate) fn find_best_grid_idx_4(
    grid: &[[u8; 4]],
    xval: &[f32],
    weights: &[f32],
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
            dist += weights[i] * diff * diff;
        }
        if dist < best_dist {
            best_dist = dist;
            best_idx = g_idx;
        }
    }
    best_idx
}
