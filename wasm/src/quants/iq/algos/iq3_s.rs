//! IQ3_S: block_iq3_s { fp16 d; u8 qs[64]; u8 qh[8]; u8 signs[32]; u8 scales[4] } -> 110 bytes.
//! 9 bit grid indices for groups of 4 (low 8 in qs, top bit in qh), 8 full sign bits per 8 weights,
//! and a 4 bit scale per 32 weights.

use super::*;
use crate::quants::iq::{grid, lut};

pub static TYPE: IqType = IqType {
    name: "IQ3_S",
    super_size: QK_K,
    sub_size: 32,
    group_size: 4,
    bytes: 110,
    needs_imatrix: false,
    quantize,
    dequantize,
    decode,
    sub_scales,
    formula_html,
};

const K_MAX_Q: i32 = 8;
const BLOCK: usize = 32;
const BS4: usize = BLOCK / 4;
const BS8: usize = BLOCK / 8;
const QS: usize = 2;
const QH: usize = 66;
const SIGNS: usize = 74;
const SCALES: usize = 106;

/// Port of quantize_row_iq3_s_impl (block_size = 32) for one super-block.
pub fn quantize(xbl: &[f32], qw: Option<&[f32]>) -> Encoded {
    let grid = grid::iq3s();

    let mut bytes = vec![0u8; TYPE.bytes];
    let mut probe = vec![0u16; QK_K / 4];

    let mut scales = [0.0f32; QK_K / BLOCK];
    let mut weight = [0.0f32; BLOCK];
    let mut xval = [0.0f32; BLOCK];
    let mut l = [0i8; BLOCK];
    let mut laux = [0i8; BLOCK];
    let mut waux = [0.0f32; BLOCK];
    let mut is_on_grid: [bool; BS4];
    let mut is_on_grid_aux = [false; BS4];
    let mut probe_aux = [0u16; BS4];
    let mut probe_best: [u16; BS4];
    let mut block_signs = [0u8; BS8];

    let mut max_scale = 0.0f32;
    // Upstream writes qs/signs through pointers that are only advanced for non-zero
    // sub-blocks, so `slot` can lag behind `ib`. qh is indexed by ib and doesn't shift.
    let mut slot = 0usize;
    let mut skipped = Vec::new();

    let mut sumx2 = 0.0f32;
    for i in 0..QK_K {
        sumx2 += xbl[i] * xbl[i];
    }
    let sigma2 = 2.0 * sumx2 / QK_K as f32;

    for ib in 0..QK_K / BLOCK {
        let xb = &xbl[BLOCK * ib..BLOCK * ib + BLOCK];
        match qw {
            Some(qw) => {
                for i in 0..BLOCK {
                    weight[i] = qw[BLOCK * ib + i] * (sigma2 + xb[i] * xb[i]).sqrt();
                }
            }
            None => {
                for i in 0..BLOCK {
                    weight[i] = xb[i] * xb[i];
                }
            }
        }
        for i in 0..BLOCK {
            waux[i] = weight[i].sqrt();
        }
        for k in 0..BS8 {
            let mut s = 0u8;
            for i in 0..8 {
                if xb[8 * k + i] >= 0.0 {
                    xval[8 * k + i] = xb[8 * k + i];
                } else {
                    xval[8 * k + i] = -xb[8 * k + i];
                    s |= 1 << i;
                }
            }
            block_signs[k] = s;
        }
        let mut max = xval[0];
        for i in 1..BLOCK {
            if xval[i] > max {
                max = xval[i];
            }
        }
        l.fill(0);
        if max == 0.0 {
            scales[ib] = 0.0;
            skipped.push(ib);
            continue;
        }
        let mut best = 0.0f32;
        let mut scale = max / (2 * K_MAX_Q - 1) as f32;
        is_on_grid = [false; BS4];
        probe_best = [0; BS4];
        for is in -9..=9 {
            let id = ((2 * K_MAX_Q - 1) as f32 + is as f32 * 0.2) / max;
            let this_scale = 1.0 / id;
            for k in 0..BS4 {
                for i in 0..4 {
                    let li = nearest_int(0.5 * (id * xval[4 * k + i] - 1.0));
                    laux[4 * k + i] = li.clamp(0, K_MAX_Q - 1) as i8;
                }
                let u = pack_levels(&laux[4 * k..4 * k + 4], 3);
                is_on_grid_aux[k] = true;
                probe_aux[k] = 0;
                if grid.map[u] < 0 {
                    is_on_grid_aux[k] = false;
                    probe_aux[k] = grid.neighbours[u].len() as u16;
                    find_best_neighbour(
                        &grid.neighbours[u],
                        grid,
                        &xval[4 * k..],
                        &waux[4 * k..],
                        this_scale,
                        &mut laux[4 * k..4 * k + 4],
                    );
                }
            }
            let mut sumqx = 0.0f32;
            let mut sumq2 = 0.0f32;
            for i in 0..BLOCK {
                let w = weight[i];
                let q = (2 * laux[i] as i32 + 1) as f32;
                sumqx += w * xval[i] * q;
                sumq2 += w * q * q;
            }
            if sumq2 > 0.0 && sumqx * sumqx > best * sumq2 {
                scale = sumqx / sumq2;
                best = scale * sumqx;
                l = laux;
                is_on_grid = is_on_grid_aux;
                probe_best = probe_aux;
            }
        }
        let n_not_ongrid = is_on_grid.iter().filter(|&&g| !g).count();
        if n_not_ongrid > 0 && scale > 0.0 {
            // Unlike the other types this redoes every group, the on-grid skip is commented out upstream
            let id = 1.0 / scale;
            for k in 0..BS4 {
                let mut u = 0usize;
                for i in 0..4 {
                    let li = nearest_int(0.5 * (id * xval[4 * k + i] - 1.0)).clamp(0, K_MAX_Q - 1);
                    u |= (li as usize) << (3 * i);
                }
                let mut grid_index = grid.map[u];
                probe_best[k] = 0;
                if grid_index < 0 {
                    probe_best[k] = grid.neighbours[u].len() as u16;
                    grid_index = find_best_neighbour(
                        &grid.neighbours[u],
                        grid,
                        &xval[4 * k..],
                        &waux[4 * k..],
                        scale,
                        &mut l[4 * k..4 * k + 4],
                    ) as i32;
                }
                let pg = &grid.points[grid_index as usize];
                for i in 0..4 {
                    l[4 * k + i] = (pg[i] - 1) / 2;
                }
            }
            let mut sumqx = 0.0f32;
            let mut sumq2 = 0.0f32;
            for i in 0..BLOCK {
                let w = weight[i];
                let q = (2 * l[i] as i32 + 1) as f32;
                sumqx += w * xval[i] * q;
                sumq2 += w * q * q;
            }
            if sumq2 > 0.0 {
                scale = sumqx / sumq2;
            }
        }
        if scale < 0.0 {
            scale = -scale;
            for k in 0..BS8 {
                block_signs[k] = !block_signs[k];
            }
        }
        for k in 0..BS4 {
            let u = pack_levels(&l[4 * k..4 * k + 4], 3);
            let grid_index = grid.map[u].max(0) as usize;
            let g = ib * BS4 + k;
            bytes[QS + slot * BS4 + k] = (grid_index & 255) as u8;
            bytes[QH + g / 8] |= ((grid_index >> 8) << (g % 8)) as u8;
            probe[slot * BS4 + k] = probe_best[k];
        }
        for k in 0..BS8 {
            bytes[SIGNS + slot * BS8 + k] = block_signs[k];
        }
        slot += 1;
        scales[ib] = scale;
        if scale > max_scale {
            max_scale = scale;
        }
    }

    let note = if skipped.is_empty() || skipped == [QK_K / BLOCK - 1] {
        None
    } else {
        let list = skipped
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Some((
            format!("zero sub-block {}, later slots shifted", list),
            format!("Sub-block {} is all zero. llama.cpp skips it without advancing the qs/signs pointers, following sub-blocks are stored one slot early while qh and scales stay in place. Upstream bug, reproduced.", list),
        ))
    };

    if max_scale == 0.0 {
        return Encoded { bytes, probe, note };
    }

    let d = max_scale / 31.0;
    put_u16(&mut bytes, 0, f16_bits(d * 1.033));
    let id = 1.0 / d;
    for ib in (0..QK_K / BLOCK).step_by(2) {
        let l1 = nearest_int(0.5 * (id * scales[ib] - 1.0)).clamp(0, 15) as u8;
        let l2 = nearest_int(0.5 * (id * scales[ib + 1] - 1.0)).clamp(0, 15) as u8;
        bytes[SCALES + ib / 2] = l1 | (l2 << 4);
    }
    Encoded { bytes, probe, note }
}

fn grid_index(b: &[u8], g: usize) -> usize {
    (b[QS + g] as usize) | ((((b[QH + g / 8] >> (g % 8)) & 1) as usize) << 8)
}

fn scale_nibble(b: &[u8], ib32: usize) -> u32 {
    ((b[SCALES + ib32 / 2] >> (4 * (ib32 % 2))) & 0xf) as u32
}

/// Port of dequantize_row_iq3_s.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(get_u16(b, 0));
    for ib32 in 0..QK_K / 32 {
        let db = d * (1 + 2 * scale_nibble(b, ib32)) as f32;
        for l in 0..4 {
            let row1 = lut::IQ3S_GRID[grid_index(b, 8 * ib32 + 2 * l)] as u64;
            let row2 = lut::IQ3S_GRID[grid_index(b, 8 * ib32 + 2 * l + 1)] as u64;
            let signs = b[SIGNS + 4 * ib32 + l];
            for j in 0..4 {
                let s1 = if signs & (1 << j) != 0 { -1.0 } else { 1.0 };
                let s2 = if signs & (1 << (j + 4)) != 0 {
                    -1.0
                } else {
                    1.0
                };
                y[32 * ib32 + 8 * l + j] = db * grid_byte(row1, j) as f32 * s1;
                y[32 * ib32 + 8 * l + j + 4] = db * grid_byte(row2, j) as f32 * s2;
            }
        }
    }
}

pub fn sub_scales(b: &[u8]) -> Vec<f32> {
    let d = f16_val(get_u16(b, 0));
    (0..QK_K / 32)
        .map(|ib| d * (1 + 2 * scale_nibble(b, ib)) as f32)
        .collect()
}

pub fn decode(b: &[u8], j: usize) -> IqDecode {
    let g = j / 4;
    let jj = j % 8;
    let d = f16_val(get_u16(b, 0));
    let s = scale_nibble(b, j / 32);
    let idx = grid_index(b, g);
    let neg = b[SIGNS + j / 8];
    let row = lut::IQ3S_GRID[idx] as u64;
    let levels: Vec<f32> = (0..4).map(|k| grid_byte(row, k) as f32).collect();
    IqDecode {
        d,
        sub_field: s,
        sub_bits: 4,
        dl: d * (1 + 2 * s) as f32,
        dl_expr: "d&middot;(1+2&middot;{s})".into(),
        math: format!(
            "{:.5}&middot;(1+2&middot;{}) &times; {} &times; {}1",
            d,
            s,
            levels[j % 4],
            sign_char(neg & (1 << jj) != 0)
        ),
        codebook: Codebook::Grid {
            idx,
            size: 512,
            levels,
        },
        signs: Signs::Full { neg },
        delta: None,
    }
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (1 + 2&middot;<span class=\"eq-pill\">s<span class=\"bits\">4b</span></span>) &times; <span class=\"eq-pill\">grid[idx]<sub>j</sub><span class=\"bits\">9b</span></span> &times; <span class=\"eq-pill\">&plusmn;<span class=\"bits\">8b</span></span>",
        "4-element groups, 512-entry grid, independent signs, 4-bit scale per 32.",
        "512-entry D4 lattice grid with odd magnitudes 1 to 15. All 8 signs stored. 4-bit scale per 32 weights, one FP16 scale per 256 with a 1.033 fudge factor. 3.4375 bpw.",
    )
}
