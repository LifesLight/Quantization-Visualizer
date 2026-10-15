//! IQ1_S: block_iq1_s { fp16 d; u8 qs[32]; u16 qh[8] } -> 50 bytes per 256 weights.
//! Groups of 8 use an 11 bit index into a ternary (-1/0/+1) grid. Each qh word holds the
//! top 3 index bits of 4 groups, a 3 bit scale and the sign of the shared +-0.125 shift.

use super::*;
use crate::quants::iq::{grid, lut};

pub static TYPE: IqType = IqType {
    name: "IQ1_S",
    super_size: QK_K,
    sub_size: 32,
    group_size: 8,
    bytes: 50,
    needs_imatrix: true,
    quantize,
    dequantize,
    decode,
    sub_scales,
    formula_html,
};

const BLOCK: usize = 32;
const QS: usize = 2;
const QH: usize = 34;

/// Port of quantize_row_iq1_s_impl for one super-block.
pub fn quantize(xbl: &[f32], qw: Option<&[f32]>) -> Encoded {
    let grid = grid::iq1();
    let qw = qw.expect("IQ1_S needs quantization weights");

    let mut bytes = vec![0u8; TYPE.bytes];
    let mut probe = vec![0u16; QK_K / 8];

    let x_p = [
        -1.0 + lut::IQ1S_DELTA,
        lut::IQ1S_DELTA,
        1.0 + lut::IQ1S_DELTA,
    ];
    let x_m = [
        -1.0 - lut::IQ1S_DELTA,
        -lut::IQ1S_DELTA,
        1.0 - lut::IQ1S_DELTA,
    ];

    let mut scales = [0.0f32; QK_K / BLOCK];
    let mut shifts = [0i8; QK_K / BLOCK];
    let mut weight = [0.0f32; BLOCK];
    let mut sumx = [0.0f32; BLOCK + 1];
    let mut sumw = [0.0f32; BLOCK + 1];
    let mut idx = [0usize; BLOCK];
    let mut l = [0i8; BLOCK];
    let mut index = [0u16; BLOCK / 8];
    let mut qh = [0u16; QK_K / BLOCK];

    let mut max_scale = 0.0f32;

    let mut sumx2 = 0.0f32;
    for i in 0..QK_K {
        sumx2 += xbl[i] * xbl[i];
    }
    let sigma2 = 2.0 * sumx2 / QK_K as f32;

    for ib in 0..QK_K / BLOCK {
        let xb = &xbl[BLOCK * ib..BLOCK * ib + BLOCK];
        for i in 0..BLOCK {
            weight[i] = qw[BLOCK * ib + i] * (sigma2 + xb[i] * xb[i]).sqrt();
        }
        let mut max = xb[0].abs();
        for i in 1..BLOCK {
            if xb[i].abs() > max {
                max = xb[i].abs();
            }
        }
        if max < GROUP_MAX_EPS_IQ1_S {
            scales[ib] = 0.0;
            shifts[ib] = 1;
            continue;
        }
        // With only -1/0/+1 the weighted SSD can be minimized exactly: sort the values and
        // try every pair of split points (i1, i2) with prefix sums, for both shift signs.
        sort_pairs(xb, &mut idx);
        sumx[0] = 0.0;
        sumw[0] = 0.0;
        for j in 0..BLOCK {
            let i = idx[j];
            sumx[j + 1] = sumx[j] + weight[i] * xb[i];
            sumw[j + 1] = sumw[j] + weight[i];
        }
        let mut best_score = -f32::MAX;
        let mut scale = max;
        let mut besti1: i32 = -1;
        let mut besti2: i32 = -1;
        let mut best_shift = 0i8;
        for i1 in 0..=BLOCK {
            for i2 in i1..=BLOCK {
                let mut sumqx = (sumx[i1] - sumx[0]) * x_p[0]
                    + (sumx[i2] - sumx[i1]) * x_p[1]
                    + (sumx[BLOCK] - sumx[i2]) * x_p[2];
                let mut sumq2 = (sumw[i1] - sumw[0]) * x_p[0] * x_p[0]
                    + (sumw[i2] - sumw[i1]) * x_p[1] * x_p[1]
                    + (sumw[BLOCK] - sumw[i2]) * x_p[2] * x_p[2];
                if sumq2 > 0.0 && sumqx * sumqx > best_score * sumq2 {
                    scale = sumqx / sumq2;
                    best_score = scale * sumqx;
                    besti1 = i1 as i32;
                    besti2 = i2 as i32;
                    best_shift = 1;
                }
                sumqx = (sumx[i1] - sumx[0]) * x_m[0]
                    + (sumx[i2] - sumx[i1]) * x_m[1]
                    + (sumx[BLOCK] - sumx[i2]) * x_m[2];
                sumq2 = (sumw[i1] - sumw[0]) * x_m[0] * x_m[0]
                    + (sumw[i2] - sumw[i1]) * x_m[1] * x_m[1]
                    + (sumw[BLOCK] - sumw[i2]) * x_m[2] * x_m[2];
                if sumq2 > 0.0 && sumqx * sumqx > best_score * sumq2 {
                    scale = sumqx / sumq2;
                    best_score = scale * sumqx;
                    besti1 = i1 as i32;
                    besti2 = i2 as i32;
                    best_shift = -1;
                }
            }
        }
        if besti1 < 0 || besti2 < 0 || best_shift == 0 {
            scales[ib] = 0.0;
            shifts[ib] = 1;
            continue;
        }
        let (i1, i2) = (besti1 as usize, besti2 as usize);
        for j in 0..i1 {
            l[idx[j]] = 0;
        }
        for j in i1..i2 {
            l[idx[j]] = 1;
        }
        for j in i2..BLOCK {
            l[idx[j]] = 2;
        }
        if scale < 0.0 {
            for j in 0..BLOCK {
                l[j] = 2 - l[j];
            }
            scale = -scale;
            best_shift = -best_shift;
        }
        let mut all_on_grid = true;
        let xx = if best_shift == 1 { &x_p } else { &x_m };
        for k in 0..BLOCK / 8 {
            let u = pack_levels(&l[8 * k..8 * k + 8], 2);
            let mut grid_index = grid.map[u];
            if grid_index < 0 {
                all_on_grid = false;
                probe[4 * ib + k] = grid.neighbours[u].len() as u16;
                grid_index = iq1_find_best_neighbour2(
                    &grid.neighbours[u],
                    grid,
                    &xb[8 * k..],
                    &weight[8 * k..],
                    scale,
                    xx,
                    &mut l[8 * k..8 * k + 8],
                ) as i32;
            }
            index[k] = grid_index as u16;
        }
        if !all_on_grid {
            let mut sumqx = 0.0f32;
            let mut sumq2 = 0.0f32;
            for k in 0..BLOCK / 8 {
                let pg = &grid.points[index[k] as usize];
                for j in 0..8 {
                    let w = weight[8 * k + j];
                    let q = xx[((pg[j] - 1) / 2) as usize];
                    sumqx += w * q * xb[8 * k + j];
                    sumq2 += w * q * q;
                }
            }
            if sumqx > 0.0 && sumq2 > 0.0 {
                scale = sumqx / sumq2;
            }
        }
        let mut h = 0u16;
        for k in 0..BLOCK / 8 {
            bytes[QS + (BLOCK / 8) * ib + k] = (index[k] & 255) as u8;
            h |= (index[k] >> 8) << (3 * k);
        }
        qh[ib] = h;
        scales[ib] = scale;
        shifts[ib] = best_shift;
        if scale > max_scale {
            max_scale = scale;
        }
    }

    if max_scale == 0.0 {
        for (i, v) in qh.iter().enumerate() {
            put_u16(&mut bytes, QH + 2 * i, *v);
        }
        return Encoded {
            bytes,
            probe,
            note: None,
        };
    }

    let d = max_scale / 15.0;
    put_u16(&mut bytes, 0, f16_bits(d * 1.125));
    let id = 1.0 / d;
    for ib in 0..QK_K / BLOCK {
        let mut li = nearest_int(0.5 * (id * scales[ib] - 1.0)).clamp(0, 7) as u16;
        if shifts[ib] == -1 {
            li |= 8;
        }
        qh[ib] |= li << 12;
    }
    for (i, v) in qh.iter().enumerate() {
        put_u16(&mut bytes, QH + 2 * i, *v);
    }
    Encoded {
        bytes,
        probe,
        note: None,
    }
}

fn grid_index(b: &[u8], g: usize) -> usize {
    let qh = get_u16(b, QH + 2 * (g / 4));
    (b[QS + g] as usize) | ((((qh >> (3 * (g % 4))) & 7) as usize) << 8)
}

fn grid_level(row: u64, j: usize) -> f32 {
    grid_byte(row, j) as i8 as f32
}

/// Port of dequantize_row_iq1_s.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(get_u16(b, 0));
    for ib in 0..QK_K / 32 {
        let qh = get_u16(b, QH + 2 * ib);
        let dl = d * (2 * ((qh >> 12) & 7) + 1) as f32;
        let delta = if qh & 0x8000 != 0 {
            -lut::IQ1S_DELTA
        } else {
            lut::IQ1S_DELTA
        };
        for l in 0..4 {
            let row = lut::IQ1S_GRID[grid_index(b, 4 * ib + l)];
            for j in 0..8 {
                y[32 * ib + 8 * l + j] = dl * (grid_level(row, j) + delta);
            }
        }
    }
}

pub fn sub_scales(b: &[u8]) -> Vec<f32> {
    let d = f16_val(get_u16(b, 0));
    (0..QK_K / 32)
        .map(|ib| d * (2 * ((get_u16(b, QH + 2 * ib) >> 12) & 7) + 1) as f32)
        .collect()
}

pub fn decode(b: &[u8], j: usize) -> IqDecode {
    let ib = j / 32;
    let g = j / 8;
    let d = f16_val(get_u16(b, 0));
    let qh = get_u16(b, QH + 2 * ib);
    let s = ((qh >> 12) & 7) as u32;
    let neg_delta = qh & 0x8000 != 0;
    let delta = if neg_delta {
        -lut::IQ1S_DELTA
    } else {
        lut::IQ1S_DELTA
    };
    let idx = grid_index(b, g);
    let row = lut::IQ1S_GRID[idx];
    let levels: Vec<f32> = (0..8).map(|k| grid_level(row, k)).collect();
    let dl = d * (2 * s + 1) as f32;
    IqDecode {
        d,
        sub_field: s,
        sub_bits: 3,
        dl,
        dl_expr: "d&middot;(2&middot;{s}+1)".into(),
        math: format!(
            "{:.5}&middot;(2&middot;{}+1) &times; ({}{}.125)",
            d,
            s,
            levels[j % 8],
            if neg_delta { "&minus;" } else { "+" }
        ),
        codebook: Codebook::Grid {
            idx,
            size: 2048,
            levels,
        },
        signs: Signs::None,
        delta: Some(delta),
    }
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (2&middot;<span class=\"eq-pill\">s<span class=\"bits\">3b</span></span> + 1) &times; ( <span class=\"eq-pill\">grid[idx]<sub>j</sub><span class=\"bits\">11b</span></span> + <span class=\"eq-pill\">&plusmn;0.125<span class=\"bits\">1b</span></span> )",
        "8-element groups, 2048-entry {-1, 0, 1} grid, &plusmn;0.125 shift and 3-bit scale per 32.",
        "2048-entry ternary grid, no sign bits. Every 32 weights share one &plusmn;0.125 shift and one 3-bit scale. Split points are solved exactly by sorting, then snapped to the grid. One FP16 scale per 256 with a 1.125 fudge factor. 1.5625 bpw.",
    )
}
