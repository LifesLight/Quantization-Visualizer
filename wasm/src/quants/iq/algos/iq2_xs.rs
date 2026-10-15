//! IQ2_XS: block_iq2_xs { fp16 d; u16 qs[32]; u8 scales[8] } -> 74 bytes per 256 weights.
//! Each u16 in qs is a 9 bit grid index plus 7 sign bits, scales are 4 bits per 16 weights.

use super::*;
use crate::quants::iq::{grid, lut};

pub static TYPE: IqType = IqType {
    name: "IQ2_XS",
    super_size: QK_K,
    sub_size: 16,
    group_size: 8,
    bytes: 74,
    needs_imatrix: true,
    quantize,
    dequantize,
    decode,
    sub_scales,
    formula_html,
};

const K_MAX_Q: i32 = 3;
const QS: usize = 2;
const SCALES: usize = 66;

/// Port of quantize_row_iq2_xs_impl for one super-block.
pub fn quantize(xbl: &[f32], qw: Option<&[f32]>) -> Encoded {
    let grid = grid::iq2xs();
    let qw = qw.expect("IQ2_XS needs quantization weights");

    let mut bytes = vec![0u8; TYPE.bytes];
    let mut probe = vec![0u16; QK_K / 8];

    let mut scales = [0.0f32; QK_K / 16];
    let mut weight = [0.0f32; 16];
    let mut xval = [0.0f32; 16];
    let mut l = [0i8; 16];
    let mut laux = [0i8; 16];
    let mut waux = [0.0f32; 16];
    let mut is_on_grid: [bool; 2];
    let mut is_on_grid_aux = [true; 2];
    let mut probe_aux = [0u16; 2];
    let mut probe_best: [u16; 2];
    let mut block_signs = [0u8; 2];
    let mut q2 = [0u16; 2 * (QK_K / 16)];

    let mut max_scale = 0.0f32;

    let mut sumx2 = 0.0f32;
    for i in 0..QK_K {
        sumx2 += xbl[i] * xbl[i];
    }
    let sigma2 = sumx2 / QK_K as f32;

    for ib in 0..QK_K / 16 {
        let xb = &xbl[16 * ib..16 * ib + 16];
        let qwb = &qw[16 * ib..16 * ib + 16];
        for i in 0..16 {
            weight[i] = qwb[i] * (sigma2 + xb[i] * xb[i]).sqrt();
        }
        for i in 0..16 {
            waux[i] = weight[i].sqrt();
        }
        for k in 0..2 {
            let r = 8 * k..8 * k + 8;
            block_signs[k] = even_signs(&xb[r.clone()], &weight[r.clone()], &mut xval[r]);
        }
        let mut max = xval[0];
        for i in 1..16 {
            if xval[i] > max {
                max = xval[i];
            }
        }
        l.fill(0);
        if max < GROUP_MAX_EPS {
            scales[ib] = 0.0;
            continue;
        }
        let mut best = 0.0f32;
        let mut scale = max / (2 * K_MAX_Q - 1) as f32;
        is_on_grid = [true; 2];
        probe_best = [0; 2];
        for is in -9..=9 {
            let id = ((2 * K_MAX_Q - 1) as f32 + is as f32 * 0.1) / max;
            let this_scale = 1.0 / id;
            for k in 0..2 {
                for i in 0..8 {
                    let li = nearest_int(0.5 * (id * xval[8 * k + i] - 1.0));
                    laux[8 * k + i] = li.clamp(0, K_MAX_Q - 1) as i8;
                }
                let u = pack_levels(&laux[8 * k..8 * k + 8], 2);
                is_on_grid_aux[k] = true;
                probe_aux[k] = 0;
                if grid.map[u] < 0 {
                    is_on_grid_aux[k] = false;
                    probe_aux[k] = grid.neighbours[u].len() as u16;
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
            for i in 0..16 {
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
            for k in 0..2 {
                if is_on_grid[k] {
                    continue;
                }
                let mut u = 0usize;
                for i in 0..8 {
                    let li = nearest_int(0.5 * (id * xval[8 * k + i] - 1.0)).clamp(0, K_MAX_Q - 1);
                    u |= (li as usize) << (2 * i);
                    l[8 * k + i] = li as i8;
                }
                probe_best[k] = 0;
                if grid.map[u] < 0 {
                    probe_best[k] = grid.neighbours[u].len() as u16;
                    find_best_neighbour(
                        &grid.neighbours[u],
                        grid,
                        &xval[8 * k..],
                        &waux[8 * k..],
                        scale,
                        &mut l[8 * k..8 * k + 8],
                    );
                }
            }
            let mut sumqx = 0.0f32;
            let mut sumq2 = 0.0f32;
            for i in 0..16 {
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
            for k in 0..2 {
                block_signs[k] = !block_signs[k] & 127;
            }
        }
        for k in 0..2 {
            let u = pack_levels(&l[8 * k..8 * k + 8], 2);
            let grid_index = grid.map[u].max(0) as u16;
            q2[2 * ib + k] = grid_index | ((block_signs[k] as u16) << 9);
            probe[2 * ib + k] = probe_best[k];
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
    for ib in 0..QK_K / 16 {
        let li = nearest_int(0.5 * (id * scales[ib] - 1.0)).clamp(0, 15) as u8;
        if ib % 2 == 0 {
            bytes[SCALES + ib / 2] = li;
        } else {
            bytes[SCALES + ib / 2] |= li << 4;
        }
    }
    for (i, v) in q2.iter().enumerate() {
        put_u16(&mut bytes, QS + 2 * i, *v);
    }
    Encoded {
        bytes,
        probe,
        note: None,
    }
}

/// Port of dequantize_row_iq2_xs.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(get_u16(b, 0));
    for ib32 in 0..QK_K / 32 {
        let sc = b[SCALES + ib32];
        let db = [
            d * (0.5 + (sc & 0xf) as f32) * 0.25,
            d * (0.5 + (sc >> 4) as f32) * 0.25,
        ];
        for l in 0..4 {
            let qs = get_u16(b, QS + 2 * (4 * ib32 + l));
            let row = lut::IQ2XS_GRID[(qs & 511) as usize];
            let signs = lut::KSIGNS_IQ2XS[(qs >> 9) as usize];
            for j in 0..8 {
                let s = if signs & (1 << j) != 0 { -1.0 } else { 1.0 };
                y[32 * ib32 + 8 * l + j] = db[l / 2] * grid_byte(row, j) as f32 * s;
            }
        }
    }
}

fn scale_nibble(b: &[u8], ib16: usize) -> u32 {
    ((b[SCALES + ib16 / 2] >> (4 * (ib16 % 2))) & 0xf) as u32
}

pub fn sub_scales(b: &[u8]) -> Vec<f32> {
    let d = f16_val(get_u16(b, 0));
    (0..QK_K / 16)
        .map(|ib| d * (0.5 + scale_nibble(b, ib) as f32) * 0.25)
        .collect()
}

pub fn decode(b: &[u8], j: usize) -> IqDecode {
    let g = j / 8;
    let jj = j % 8;
    let d = f16_val(get_u16(b, 0));
    let s = scale_nibble(b, j / 16);
    let qs = get_u16(b, QS + 2 * g) as u32;
    let idx = qs & 511;
    let sign7 = qs >> 9;
    let neg = lut::KSIGNS_IQ2XS[sign7 as usize];
    let row = lut::IQ2XS_GRID[idx as usize];
    let levels: Vec<f32> = (0..8).map(|k| grid_byte(row, k) as f32).collect();
    IqDecode {
        d,
        sub_field: s,
        sub_bits: 4,
        dl: d * (0.5 + s as f32) * 0.25,
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
            size: 512,
            levels,
        },
        signs: Signs::Parity { neg },
        delta: None,
    }
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (0.5 + <span class=\"eq-pill\">s<span class=\"bits\">4b</span></span>) &times; &frac14; &times; <span class=\"eq-pill\">grid[idx]<sub>j</sub><span class=\"bits\">9b</span></span> &times; <span class=\"eq-pill\">&plusmn;<span class=\"bits\">7b</span></span>",
        "8-element groups, 512-entry grid, even-sign-parity, 4-bit scale per 16.",
        "512-entry lattice grid with magnitudes {8, 25, 43}. 7 sign bits per 8 weights, the 8th implied by even parity. 4-bit scale per 16 weights, one FP16 scale per 256. 2.3125 bpw.",
    )
}
