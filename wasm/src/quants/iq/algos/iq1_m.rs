//! IQ1_M: block_iq1_m { u8 qs[32]; u8 qh[16]; u8 scales[8] } -> 56 bytes per 256 weights.
//! Same ternary grid as IQ1_S, but 3 bit scales per 16 weights and a +-0.125 shift per group of 8.
//! There is no separate d field, its fp16 bits are spread over the top nibbles of the scale words.

use super::*;
use crate::quants::iq::{grid, lut};

pub static TYPE: IqType = IqType {
    name: "IQ1_M",
    super_size: QK_K,
    sub_size: 16,
    group_size: 8,
    bytes: 56,
    needs_imatrix: false,
    quantize,
    dequantize,
    decode,
    sub_scales,
    formula_html,
};

const BLOCK: usize = 16;
const QS: usize = 0;
const QH: usize = 32;
const SCALES: usize = 48;

/// Port of quantize_row_iq1_m_impl for one super-block.
pub fn quantize(xbl: &[f32], qw: Option<&[f32]>) -> Encoded {
    let grid = grid::iq1();

    let mut bytes = vec![0u8; TYPE.bytes];
    let mut probe = vec![0u16; QK_K / 8];

    let x_p = [
        -1.0 + lut::IQ1M_DELTA,
        lut::IQ1M_DELTA,
        1.0 + lut::IQ1M_DELTA,
    ];
    let x_m = [
        -1.0 - lut::IQ1M_DELTA,
        -lut::IQ1M_DELTA,
        1.0 - lut::IQ1M_DELTA,
    ];
    let masks: [u8; 4] = [0x00, 0x80, 0x08, 0x88];

    let mut scales = [0.0f32; QK_K / BLOCK];
    let mut shifts = [0usize; QK_K / BLOCK];
    let mut weight = [0.0f32; BLOCK];
    let mut idx = [0usize; BLOCK];
    let mut l = [0i8; BLOCK];
    let mut index = [0u16; BLOCK / 8];
    let mut sumqx = [0.0f32; 4];
    let mut sumq2 = [0.0f32; 4];

    let mut max_scale = 0.0f32;

    let mut sumx2 = 0.0f32;
    for i in 0..QK_K {
        sumx2 += xbl[i] * xbl[i];
    }
    let sigma2 = 2.0 * sumx2 / QK_K as f32;

    let set_weights = |weight: &mut [f32; BLOCK], ib: usize, xb: &[f32]| match qw {
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
    };

    for ib in 0..QK_K / BLOCK {
        let xb = &xbl[BLOCK * ib..BLOCK * ib + BLOCK];
        set_weights(&mut weight, ib, xb);
        let mut max = xb[0].abs();
        for i in 1..BLOCK {
            if xb[i].abs() > max {
                max = xb[i].abs();
            }
        }
        if max < GROUP_MAX_EPS_IQ1_M {
            scales[ib] = 0.0;
            shifts[ib] = 0;
            continue;
        }
        sort_pairs(xb, &mut idx);
        let mut best_score = -f32::MAX;
        let mut scale = max;
        let mut besti1: i32 = -1;
        let mut besti2: i32 = -1;
        let mut best_k: i32 = -1;
        {
            // Prefix sums per half, because each half can use its own shift sign.
            // Since v[0]-v[1] = v[1]-v[2] = -1 for both x_p and x_m, the 3-way split sum
            // collapses to T*v[2] - px[i1] - px[i2].
            let mut px = [[0.0f32; BLOCK + 1]; 2];
            let mut pw = [[0.0f32; BLOCK + 1]; 2];
            for j in 0..BLOCK {
                let i = idx[j];
                let h = if i < BLOCK / 2 { 0 } else { 1 };
                px[h][j + 1] = px[h][j] + weight[i] * xb[i];
                px[1 - h][j + 1] = px[1 - h][j];
                pw[h][j + 1] = pw[h][j] + weight[i];
                pw[1 - h][j + 1] = pw[1 - h][j];
            }
            let txs = [px[0][BLOCK], px[1][BLOCK]];
            let tws = [pw[0][BLOCK], pw[1][BLOCK]];
            let p2 = x_p[2];
            let m2 = x_m[2];
            let cp1 = x_p[0] * x_p[0] - x_p[1] * x_p[1];
            let cp2 = x_p[1] * x_p[1] - x_p[2] * x_p[2];
            let cm1 = x_m[0] * x_m[0] - x_m[1] * x_m[1];
            let cm2 = x_m[1] * x_m[1] - x_m[2] * x_m[2];
            for i1 in 0..=BLOCK {
                for i2 in i1..=BLOCK {
                    let mut qx_p = [0.0f32; 2];
                    let mut qx_m = [0.0f32; 2];
                    let mut q2_p = [0.0f32; 2];
                    let mut q2_m = [0.0f32; 2];
                    for h in 0..2 {
                        let sx = px[h][i1] + px[h][i2];
                        qx_p[h] = txs[h] * p2 - sx;
                        qx_m[h] = txs[h] * m2 - sx;
                        q2_p[h] = tws[h] * p2 * p2 + pw[h][i1] * cp1 + pw[h][i2] * cp2;
                        q2_m[h] = tws[h] * m2 * m2 + pw[h][i1] * cm1 + pw[h][i2] * cm2;
                    }
                    sumqx[0] = qx_p[0] + qx_p[1];
                    sumqx[1] = qx_p[0] + qx_m[1];
                    sumqx[2] = qx_m[0] + qx_p[1];
                    sumqx[3] = qx_m[0] + qx_m[1];
                    sumq2[0] = q2_p[0] + q2_p[1];
                    sumq2[1] = q2_p[0] + q2_m[1];
                    sumq2[2] = q2_m[0] + q2_p[1];
                    sumq2[3] = q2_m[0] + q2_m[1];
                    for k in 0..4 {
                        if sumq2[k] > 0.0 && sumqx[k] * sumqx[k] > best_score * sumq2[k] {
                            scale = sumqx[k] / sumq2[k];
                            best_score = scale * sumqx[k];
                            besti1 = i1 as i32;
                            besti2 = i2 as i32;
                            best_k = k as i32;
                        }
                    }
                }
            }
        }
        if besti1 < 0 || besti2 < 0 || best_k < 0 {
            scales[ib] = 0.0;
            shifts[ib] = 0;
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
        let mut best_k = best_k as usize;
        if scale < 0.0 {
            for j in 0..BLOCK {
                l[j] = 2 - l[j];
            }
            scale = -scale;
            best_k = 3 - best_k;
        }
        let mut all_on_grid = true;
        for k in 0..BLOCK / 8 {
            let xx = pick(&x_p, &x_m, best_k, k);
            let u = pack_levels(&l[8 * k..8 * k + 8], 2);
            let mut grid_index = grid.map[u];
            if grid_index < 0 {
                all_on_grid = false;
                probe[2 * ib + k] = grid.neighbours[u].len() as u16;
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
            let mut sumqx_f = 0.0f32;
            let mut sumq2_f = 0.0f32;
            for k in 0..BLOCK / 8 {
                let xx = pick(&x_p, &x_m, best_k, k);
                let pg = &grid.points[index[k] as usize];
                for j in 0..8 {
                    let w = weight[8 * k + j];
                    let q = xx[((pg[j] - 1) / 2) as usize];
                    sumqx_f += w * q * xb[8 * k + j];
                    sumq2_f += w * q * q;
                }
            }
            if sumqx_f > 0.0 && sumq2_f > 0.0 {
                scale = sumqx_f / sumq2_f;
            }
        }
        bytes[QS + 2 * ib] = (index[0] & 255) as u8;
        bytes[QS + 2 * ib + 1] = (index[1] & 255) as u8;
        bytes[QH + ib] = ((index[0] >> 8) | ((index[1] >> 8) << 4)) as u8;
        scales[ib] = scale;
        shifts[ib] = best_k;
        if scale > max_scale {
            max_scale = scale;
        }
    }

    if max_scale == 0.0 {
        return Encoded {
            bytes,
            probe,
            note: None,
        };
    }

    let mut sc = [0u16; 4];
    let mut d = max_scale / 15.0;
    let id = 1.0 / d;
    let mut sumqx_f = 0.0f32;
    let mut sumq2_f = 0.0f32;
    for ib in 0..QK_K / BLOCK {
        let li = nearest_int(0.5 * (id * scales[ib] - 1.0)).clamp(0, 7);
        sc[ib / 4] |= (li as u16) << (3 * (ib % 4));
        bytes[QH + ib] |= masks[shifts[ib]];
        let xb = &xbl[BLOCK * ib..BLOCK * ib + BLOCK];
        set_weights(&mut weight, ib, xb);
        for k in 0..BLOCK / 8 {
            let xx = pick(&x_p, &x_m, shifts[ib], k);
            let gi = bytes[QS + 2 * ib + k] as usize
                + (((bytes[QH + ib] as usize) << (8 - 4 * k)) & 0x700);
            let pg = &grid.points[gi];
            for j in 0..8 {
                let w = weight[8 * k + j];
                let q = xx[((pg[j] - 1) / 2) as usize] * (2 * li + 1) as f32;
                sumqx_f += w * q * xb[8 * k + j];
                sumq2_f += w * q * q;
            }
        }
    }
    if sumq2_f > 0.0 {
        d = sumqx_f / sumq2_f;
    }
    let s = f16_bits(d * 1.1125);
    sc[0] |= (s & 0x000f) << 12;
    sc[1] |= (s & 0x00f0) << 8;
    sc[2] |= (s & 0x0f00) << 4;
    sc[3] |= s & 0xf000;
    for (i, v) in sc.iter().enumerate() {
        put_u16(&mut bytes, SCALES + 2 * i, *v);
    }
    Encoded {
        bytes,
        probe,
        note: None,
    }
}

/// Shift choice per group of 8. best_k: 0 = (+,+), 1 = (+,-), 2 = (-,+), 3 = (-,-)
fn pick<'a>(x_p: &'a [f32; 3], x_m: &'a [f32; 3], best_k: usize, k: usize) -> &'a [f32; 3] {
    let plus = if k == 0 { best_k < 2 } else { best_k % 2 == 0 };
    if plus {
        x_p
    } else {
        x_m
    }
}

fn sc(b: &[u8], i: usize) -> u16 {
    get_u16(b, SCALES + 2 * i)
}

fn d_bits(b: &[u8]) -> u16 {
    (sc(b, 0) >> 12) | ((sc(b, 1) >> 8) & 0x00f0) | ((sc(b, 2) >> 4) & 0x0f00) | (sc(b, 3) & 0xf000)
}

/// 3 bit scale of 16-weight block `ib16`.
fn scale3(b: &[u8], ib16: usize) -> u32 {
    ((sc(b, ib16 / 4) >> (3 * (ib16 % 4))) & 0x7) as u32
}

fn grid_index(b: &[u8], g: usize) -> usize {
    let qh = b[QH + g / 2] as usize;
    b[QS + g] as usize | ((qh << (8 - 4 * (g % 2))) & 0x700)
}

fn delta_neg(b: &[u8], g: usize) -> bool {
    b[QH + g / 2] & (if g % 2 == 0 { 0x08 } else { 0x80 }) != 0
}

fn grid_level(row: u64, j: usize) -> f32 {
    grid_byte(row, j) as i8 as f32
}

/// Port of dequantize_row_iq1_m.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(d_bits(b));
    for ib in 0..QK_K / 32 {
        let dl1 = d * (2 * scale3(b, 2 * ib) + 1) as f32;
        let dl2 = d * (2 * scale3(b, 2 * ib + 1) + 1) as f32;
        for l in 0..4 {
            let g = 4 * ib + l;
            let dl = if l < 2 { dl1 } else { dl2 };
            let delta = if delta_neg(b, g) {
                -lut::IQ1S_DELTA
            } else {
                lut::IQ1S_DELTA
            };
            let row = lut::IQ1S_GRID[grid_index(b, g)];
            for j in 0..8 {
                y[8 * g + j] = dl * (grid_level(row, j) + delta);
            }
        }
    }
}

pub fn sub_scales(b: &[u8]) -> Vec<f32> {
    let d = f16_val(d_bits(b));
    (0..QK_K / 16)
        .map(|ib| d * (2 * scale3(b, ib) + 1) as f32)
        .collect()
}

pub fn decode(b: &[u8], j: usize) -> IqDecode {
    let g = j / 8;
    let d = f16_val(d_bits(b));
    let s = scale3(b, j / 16);
    let neg_delta = delta_neg(b, g);
    let idx = grid_index(b, g);
    let row = lut::IQ1S_GRID[idx];
    let levels: Vec<f32> = (0..8).map(|k| grid_level(row, k)).collect();
    IqDecode {
        d,
        sub_field: s,
        sub_bits: 3,
        dl: d * (2 * s + 1) as f32,
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
        delta: Some(if neg_delta {
            -lut::IQ1S_DELTA
        } else {
            lut::IQ1S_DELTA
        }),
    }
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (2&middot;<span class=\"eq-pill\">s<span class=\"bits\">3b</span></span> + 1) &times; ( <span class=\"eq-pill\">grid[idx]<sub>j</sub><span class=\"bits\">11b</span></span> + <span class=\"eq-pill\">&plusmn;0.125<span class=\"bits\">1b</span></span> )",
        "8-element groups, 2048-entry {-1, 0, 1} grid, &plusmn;0.125 shift per 8, 3-bit scale per 16.",
        "2048-entry ternary grid, no sign bits. &plusmn;0.125 shift per 8 weights, 3-bit scale per 16. The FP16 scale has no own field, its 4 nibbles sit in the top bits of the scale words. Refit by least squares with a 1.1125 fudge factor. 1.75 bpw.",
    )
}
