//! IQ3_XXS: block_iq3_xxs { fp16 d; u8 qs[96] } -> 98 bytes per 256 weights.
//! qs[0..64] are 8 bit grid indices for groups of 4, qs[64..96] one u32 per 32 weights
//! holding 4x7 sign bits and a 4 bit scale.

use super::*;
use crate::quants::iq::{grid, lut};

pub static TYPE: IqType = IqType {
    name: "IQ3_XXS",
    super_size: QK_K,
    sub_size: 32,
    group_size: 4,
    bytes: 98,
    needs_imatrix: false,
    quantize,
    dequantize,
    decode,
    sub_scales,
    formula_html,
};

const K_MAX_Q: i32 = 8;
const QS: usize = 2;
const SAS: usize = QS + QK_K / 4;

/// Port of quantize_row_iq3_xxs_impl (grid_size = 256) for one super-block.
pub fn quantize(xbl: &[f32], qw: Option<&[f32]>) -> Encoded {
    let grid = grid::iq3xxs();

    let mut bytes = vec![0u8; TYPE.bytes];
    let mut probe = vec![0u16; QK_K / 4];

    let mut scales = [0.0f32; QK_K / 32];
    let mut weight = [0.0f32; 32];
    let mut xval = [0.0f32; 32];
    let mut l = [0i8; 32];
    let mut laux = [0i8; 32];
    let mut waux = [0.0f32; 32];
    let mut is_on_grid: [bool; 8];
    let mut is_on_grid_aux = [true; 8];
    let mut probe_aux = [0u16; 8];
    let mut probe_best: [u16; 8];
    let mut block_signs = [0u8; 8];
    let mut scales_and_signs = [0u32; QK_K / 32];

    let mut max_scale = 0.0f32;

    let mut sumx2 = 0.0f32;
    for i in 0..QK_K {
        sumx2 += xbl[i] * xbl[i];
    }
    let sigma2 = 2.0 * sumx2 / QK_K as f32;

    for ib in 0..QK_K / 32 {
        let xb = &xbl[32 * ib..32 * ib + 32];
        match qw {
            Some(qw) => {
                for i in 0..32 {
                    weight[i] = qw[32 * ib + i] * (sigma2 + xb[i] * xb[i]).sqrt();
                }
            }
            None => {
                for i in 0..32 {
                    weight[i] = xb[i] * xb[i];
                }
            }
        }
        for i in 0..32 {
            waux[i] = weight[i].sqrt();
        }
        for k in 0..4 {
            let r = 8 * k..8 * k + 8;
            block_signs[k] = even_signs(&xb[r.clone()], &weight[r.clone()], &mut xval[r]);
        }
        let mut max = xval[0];
        for i in 1..32 {
            if xval[i] > max {
                max = xval[i];
            }
        }
        l.fill(0);
        if max < GROUP_MAX_EPS_IQ3_XXS {
            scales[ib] = 0.0;
            continue;
        }
        let mut best = 0.0f32;
        let mut scale = max / (2 * K_MAX_Q - 1) as f32;
        is_on_grid = [true; 8];
        probe_best = [0; 8];
        for is in -15..=15 {
            let id = ((2 * K_MAX_Q - 1) as f32 + is as f32 * 0.2) / max;
            let this_scale = 1.0 / id;
            for k in 0..8 {
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
            for i in 0..32 {
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
            let id = 1.0 / scale;
            for k in 0..8 {
                if is_on_grid[k] {
                    continue;
                }
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
            for i in 0..32 {
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
            for k in 0..4 {
                block_signs[k] = !block_signs[k] & 127;
            }
        }
        for k in 0..8 {
            let u = pack_levels(&l[4 * k..4 * k + 4], 3);
            bytes[QS + 8 * ib + k] = grid.map[u].max(0) as u8;
            probe[8 * ib + k] = probe_best[k];
        }
        scales_and_signs[ib] = block_signs[0] as u32
            | (block_signs[1] as u32) << 7
            | (block_signs[2] as u32) << 14
            | (block_signs[3] as u32) << 21;
        scales[ib] = scale;
        if scale > max_scale {
            max_scale = scale;
        }
    }

    if max_scale == 0.0 {
        bytes[QS..].fill(0);
        return Encoded {
            bytes,
            probe,
            note: None,
        };
    }

    let d = max_scale / 31.0;
    put_u16(&mut bytes, 0, f16_bits(d * 1.0125));
    let id = 1.0 / d;
    for ib in 0..QK_K / 32 {
        let li = nearest_int(0.5 * (id * scales[ib] - 1.0)).clamp(0, 15);
        scales_and_signs[ib] |= (li as u32) << 28;
    }
    for (i, v) in scales_and_signs.iter().enumerate() {
        put_u32(&mut bytes, SAS + 4 * i, *v);
    }
    Encoded {
        bytes,
        probe,
        note: None,
    }
}

/// Port of dequantize_row_iq3_xxs.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(get_u16(b, 0));
    for ib32 in 0..QK_K / 32 {
        let aux32 = get_u32(b, SAS + 4 * ib32);
        let db = d * (0.5 + (aux32 >> 28) as f32) * 0.5;
        for l in 0..4 {
            let signs = lut::KSIGNS_IQ2XS[((aux32 >> (7 * l)) & 127) as usize];
            let grid1 = lut::IQ3XXS_GRID[b[QS + 8 * ib32 + 2 * l] as usize] as u64;
            let grid2 = lut::IQ3XXS_GRID[b[QS + 8 * ib32 + 2 * l + 1] as usize] as u64;
            for j in 0..4 {
                let s1 = if signs & (1 << j) != 0 { -1.0 } else { 1.0 };
                let s2 = if signs & (1 << (j + 4)) != 0 {
                    -1.0
                } else {
                    1.0
                };
                y[32 * ib32 + 8 * l + j] = db * grid_byte(grid1, j) as f32 * s1;
                y[32 * ib32 + 8 * l + j + 4] = db * grid_byte(grid2, j) as f32 * s2;
            }
        }
    }
}

pub fn sub_scales(b: &[u8]) -> Vec<f32> {
    let d = f16_val(get_u16(b, 0));
    (0..QK_K / 32)
        .map(|ib| d * (0.5 + (get_u32(b, SAS + 4 * ib) >> 28) as f32) * 0.5)
        .collect()
}

pub fn decode(b: &[u8], j: usize) -> IqDecode {
    let ib = j / 32;
    let l = (j % 32) / 8;
    let g = j / 4;
    let jj = j % 8;
    let d = f16_val(get_u16(b, 0));
    let aux32 = get_u32(b, SAS + 4 * ib);
    let s = aux32 >> 28;
    let sign7 = (aux32 >> (7 * l)) & 127;
    let neg = lut::KSIGNS_IQ2XS[sign7 as usize];
    let idx = b[QS + g] as u32;
    let row = lut::IQ3XXS_GRID[idx as usize] as u64;
    let levels: Vec<f32> = (0..4).map(|k| grid_byte(row, k) as f32).collect();
    IqDecode {
        d,
        sub_field: s,
        sub_bits: 4,
        dl: d * (0.5 + s as f32) * 0.5,
        dl_expr: "d&middot;(0.5+{s})&middot;&frac12;".into(),
        math: format!(
            "{:.5}&middot;(0.5+{})&middot;&frac12; &times; {} &times; {}1",
            d,
            s,
            levels[j % 4],
            sign_char(neg & (1 << jj) != 0)
        ),
        codebook: Codebook::Grid {
            idx: idx as usize,
            size: 256,
            levels,
        },
        signs: Signs::Parity { neg },
        delta: None,
    }
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (0.5 + <span class=\"eq-pill\">s<span class=\"bits\">4b</span></span>) &times; &frac12; &times; <span class=\"eq-pill\">grid[idx]<sub>j</sub><span class=\"bits\">8b</span></span> &times; <span class=\"eq-pill\">&plusmn;<span class=\"bits\">7b</span></span>",
        "4-element groups, 256-entry grid, even-sign-parity per 8, 4-bit scale per 32.",
        "256-entry D4 lattice grid with odd magnitudes 1 to 13, top level 15.5 instead of 15. 7 sign bits per 8 weights, the 8th implied by even parity. 4-bit scale per 32 weights, one FP16 scale per 256 with a 1.0125 fudge factor. 3.0625 bpw.",
    )
}
