//! IQ2_XXS: block_iq2_xxs { fp16 d; u16 qs[32] } -> 66 bytes per 256 weights.
//! Every 32 weights use two u32: 4 grid indices (8b each), then 4x7 sign bits and a 4 bit scale.

use super::*;
use crate::quants::iq::{grid, lut};

pub static TYPE: IqType = IqType {
    name: "IQ2_XXS",
    super_size: QK_K,
    sub_size: 32,
    group_size: 8,
    bytes: 66,
    needs_imatrix: true,
    quantize,
    dequantize,
    decode,
    sub_scales,
    formula_html,
};

const K_MAX_Q: i32 = 3;

/// Port of quantize_row_iq2_xxs_impl for one super-block.
pub fn quantize(xbl: &[f32], qw: Option<&[f32]>) -> Encoded {
    let grid = grid::iq2xxs();
    let qw = qw.expect("IQ2_XXS needs quantization weights");

    let mut bytes = vec![0u8; TYPE.bytes];
    let mut probe = vec![0u16; QK_K / 8];

    let mut scales = [0.0f32; QK_K / 32];
    let mut weight = [0.0f32; 32];
    let mut xval = [0.0f32; 32];
    let mut l = [0i8; 32];
    let mut laux = [0i8; 32];
    let mut waux = [0.0f32; 32];
    let mut block_signs = [0u8; 4];
    let mut q2 = [0u32; 2 * (QK_K / 32)];

    let mut max_scale = 0.0f32;

    let mut sumx2 = 0.0f32;
    for i in 0..QK_K {
        sumx2 += xbl[i] * xbl[i];
    }
    let sigma2 = sumx2 / QK_K as f32;

    for ib in 0..QK_K / 32 {
        let xb = &xbl[32 * ib..32 * ib + 32];
        let qwb = &qw[32 * ib..32 * ib + 32];
        for i in 0..32 {
            weight[i] = qwb[i] * (sigma2 + xb[i] * xb[i]).sqrt();
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
        if max < GROUP_MAX_EPS {
            scales[ib] = 0.0;
            l.fill(0);
            continue;
        }
        let mut lq = [0u8; 32];
        let mut scale = make_qp_quants(K_MAX_Q + 1, &xval, &mut lq, &weight);
        for i in 0..32 {
            l[i] = lq[i] as i8;
        }
        let eff_max = scale * K_MAX_Q as f32;
        if eff_max <= 0.0 {
            scales[ib] = 0.0;
            l.fill(0);
            continue;
        }
        let mut best = 0.0f32;
        for is in -6..=6 {
            let id = ((2 * K_MAX_Q - 1) as f32 + is as f32 * 0.1) / eff_max;
            let this_scale = 1.0 / id;
            for k in 0..4 {
                for i in 0..8 {
                    let li = nearest_int(0.5 * (id * xval[8 * k + i] - 1.0));
                    laux[8 * k + i] = li.clamp(0, K_MAX_Q - 1) as i8;
                }
                let u = pack_levels(&laux[8 * k..8 * k + 8], 2);
                if grid.map[u] < 0 {
                    find_best_neighbour(
                        &grid.neighbours[u],
                        grid,
                        &xval[8 * k..],
                        &waux[8 * k..],
                        this_scale,
                        &mut laux[8 * k..8 * k + 8],
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
            }
        }
        if scale > 0.0 {
            let id = 1.0 / scale;
            for k in 0..4 {
                let mut u = 0usize;
                for i in 0..8 {
                    let li = nearest_int(0.5 * (id * xval[8 * k + i] - 1.0)).clamp(0, K_MAX_Q - 1);
                    u |= (li as usize) << (2 * i);
                }
                let mut grid_index = grid.map[u];
                if grid_index < 0 {
                    probe[4 * ib + k] = grid.neighbours[u].len() as u16;
                    grid_index = find_best_neighbour(
                        &grid.neighbours[u],
                        grid,
                        &xval[8 * k..],
                        &waux[8 * k..],
                        scale,
                        &mut l[8 * k..8 * k + 8],
                    ) as i32;
                }
                let pg = &grid.points[grid_index as usize];
                for i in 0..8 {
                    l[8 * k + i] = (pg[i] - 1) / 2;
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
        for k in 0..4 {
            let u = pack_levels(&l[8 * k..8 * k + 8], 2);
            let grid_index = grid.map[u].max(0) as u32;
            q2[2 * ib] |= grid_index << (8 * k);
            q2[2 * ib + 1] |= (block_signs[k] as u32) << (7 * k);
        }
        scales[ib] = scale;
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

    let d = max_scale / 31.0;
    put_u16(&mut bytes, 0, f16_bits(d));
    let id = 1.0 / d;
    for ib in 0..QK_K / 32 {
        let li = nearest_int(0.5 * (id * scales[ib] - 1.0)).clamp(0, 15);
        q2[2 * ib + 1] |= (li as u32) << 28;
    }
    for (i, v) in q2.iter().enumerate() {
        put_u32(&mut bytes, 2 + 4 * i, *v);
    }
    Encoded {
        bytes,
        probe,
        note: None,
    }
}

/// Port of dequantize_row_iq2_xxs.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(get_u16(b, 0));
    for ib32 in 0..QK_K / 32 {
        let aux0 = get_u32(b, 2 + 8 * ib32);
        let aux1 = get_u32(b, 2 + 8 * ib32 + 4);
        let db = d * (0.5 + (aux1 >> 28) as f32) * 0.25;
        for l in 0..4 {
            let row = lut::IQ2XXS_GRID[((aux0 >> (8 * l)) & 255) as usize];
            let signs = lut::KSIGNS_IQ2XS[((aux1 >> (7 * l)) & 127) as usize];
            for j in 0..8 {
                let s = if signs & (1 << j) != 0 { -1.0 } else { 1.0 };
                y[32 * ib32 + 8 * l + j] = db * grid_byte(row, j) as f32 * s;
            }
        }
    }
}

pub fn sub_scales(b: &[u8]) -> Vec<f32> {
    let d = f16_val(get_u16(b, 0));
    (0..QK_K / 32)
        .map(|ib| d * (0.5 + (get_u32(b, 2 + 8 * ib + 4) >> 28) as f32) * 0.25)
        .collect()
}

pub fn decode(b: &[u8], j: usize) -> IqDecode {
    let ib = j / 32;
    let l = (j % 32) / 8;
    let jj = j % 8;
    let d = f16_val(get_u16(b, 0));
    let aux0 = get_u32(b, 2 + 8 * ib);
    let aux1 = get_u32(b, 2 + 8 * ib + 4);
    let s = aux1 >> 28;
    let idx = (aux0 >> (8 * l)) & 255;
    let sign7 = (aux1 >> (7 * l)) & 127;
    let neg = lut::KSIGNS_IQ2XS[sign7 as usize];
    let row = lut::IQ2XXS_GRID[idx as usize];
    let levels: Vec<f32> = (0..8).map(|k| grid_byte(row, k) as f32).collect();
    let dl = d * (0.5 + s as f32) * 0.25;
    IqDecode {
        d,
        sub_field: s,
        sub_bits: 4,
        dl,
        dl_expr: "d&middot;(0.5+{s})&middot;&frac14;".into(),
        math: format!(
            "{:.5}&middot;(0.5+{})&middot;&frac14; &times; {} &times; {}1",
            d,
            s,
            levels[jj],
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
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (0.5 + <span class=\"eq-pill\">s<span class=\"bits\">4b</span></span>) &times; &frac14; &times; <span class=\"eq-pill\">grid[idx]<sub>j</sub><span class=\"bits\">8b</span></span> &times; <span class=\"eq-pill\">&plusmn;<span class=\"bits\">7b</span></span>",
        "8-element groups, 256-entry grid, even-sign-parity, 4-bit scale per 32.",
        "256-entry lattice grid with magnitudes {8, 25, 43}. Searched with odd levels {1, 3, 5}, decoded as {1, 3.125, 5.375}. 7 sign bits per 8 weights, the 8th implied by even parity. 4-bit scale per 32 weights, one FP16 scale per 256. 2.0625 bpw.",
    )
}
