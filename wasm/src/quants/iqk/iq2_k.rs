//! IQ2_K: block_iq2_k { fp16 d; u16 extra; u8 scales[8]; u8 qs[64] } -> 76 bytes per 256.
//! 4 non-linear levels, every 16 weights pick the table or its shifted copy (bit in extra)
//! and a 4 bit scale (-8..7).

use super::common::*;
use super::lut::IQ2NL_VALUES;
use super::{formula, KDecode, KType};

pub static TYPE: KType = KType {
    name: "IQ2_K",
    block_bytes: BYTES,
    row_meta: 0,
    sub_size: 16,
    imatrix_block: 16,
    quantize,
    dequantize,
    decode,
    block_scales,
    formula_html,
};

const BYTES: usize = 76;
const SCALES: usize = 4;
const QS: usize = 12;
const FUDGE: f32 = 1.03;

fn table(shifted: bool) -> &'static [i8] {
    if shifted {
        &IQ2NL_VALUES[4..]
    } else {
        &IQ2NL_VALUES[..4]
    }
}

/// Port of quantize_row_iq2_k_impl.
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const BS: usize = 16;
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; nblock * BYTES];
    let val = table(false);
    let sval = table(true);
    let mut weight = [0.0f32; BS];
    let mut sumx = [0.0f32; BS + 1];
    let mut sumw = [0.0f32; BS + 1];
    let mut idx = [0usize; BS];

    for ibl in 0..nblock {
        let y = &mut out[ibl * BYTES..(ibl + 1) * BYTES];
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = 1.5 * sum_sq(xbl) / QK_K as f32;
        let set_weights = |weight: &mut [f32; BS], ib: usize| {
            let xb = &xbl[BS * ib..BS * ib + BS];
            for j in 0..BS {
                weight[j] = match qw {
                    Some(qw) => qw[ibl * QK_K + ib * BS + j] * (sigma2 + xb[j] * xb[j]).sqrt(),
                    None => 0.25 * sigma2 + xb[j] * xb[j],
                };
            }
        };

        let mut extra = 0u16;
        let mut scales = [0.0f32; QK_K / BS];
        let mut sw = [0.0f32; QK_K / BS];
        let mut max_abs_scale = 0.0f32;
        for ib in 0..QK_K / BS {
            let xb = &xbl[BS * ib..BS * ib + BS];
            set_weights(&mut weight, ib);
            let mut amax = 0.0f32;
            for j in 0..BS {
                sw[ib] += weight[j];
                amax = amax.max(xb[j].abs());
            }
            if amax < 1e-16 {
                scales[ib] = 0.0;
                continue;
            }
            sort_pairs(xb, &mut idx);
            for j in 0..BS {
                sumw[j + 1] = sumw[j] + weight[idx[j]];
                sumx[j + 1] = sumx[j] + weight[idx[j]] * xb[idx[j]];
            }
            let (mut best, mut d, mut is_shifted) = (0.0f32, 0.0f32, false);
            // the four levels split the sorted block at i1 <= i2 <= i3, both tables, both signs
            let try_split = |v: [f32; 4], i1: usize, i2: usize, i3: usize| {
                let sumqx = (sumx[i1] - sumx[0]) * v[0]
                    + (sumx[i2] - sumx[i1]) * v[1]
                    + (sumx[i3] - sumx[i2]) * v[2]
                    + (sumx[BS] - sumx[i3]) * v[3];
                let sumq2 = (sumw[i1] - sumw[0]) * v[0] * v[0]
                    + (sumw[i2] - sumw[i1]) * v[1] * v[1]
                    + (sumw[i3] - sumw[i2]) * v[2] * v[2]
                    + (sumw[BS] - sumw[i3]) * v[3] * v[3];
                (sumqx, sumq2)
            };
            let f = |t: &[i8], k: [usize; 4]| k.map(|i| t[i] as f32);
            let cands = [
                (f(val, [0, 1, 2, 3]), false),
                (f(sval, [0, 1, 2, 3]), true),
                (f(val, [3, 2, 1, 0]), false),
                (f(sval, [3, 2, 1, 0]), true),
            ];
            for i1 in 0..BS {
                for i2 in i1..BS {
                    for i3 in i2..BS {
                        for &(v, s) in &cands {
                            let (sumqx, sumq2) = try_split(v, i1, i2, i3);
                            if sumq2 > 0.0 && sumqx * sumqx > best * sumq2 {
                                d = sumqx / sumq2;
                                best = d * sumqx;
                                is_shifted = s;
                            }
                        }
                    }
                }
            }
            scales[ib] = d;
            if is_shifted {
                extra |= 1 << ib;
            }
            max_abs_scale = max_abs_scale.max(scales[ib].abs());
        }

        if max_abs_scale == 0.0 {
            continue;
        }
        let mut ls_q = [0i8; QK_K / BS];
        let d = make_qx_quants(8, &scales, &mut ls_q, &sw);
        if d == 0.0 {
            continue;
        }
        put_u16(y, 2, extra);
        let id = 1.0 / d;

        let mut sumqx = 0.0f32;
        let mut sumq2 = 0.0f32;
        for ib in 0..QK_K / BS {
            let ls = nearest_int(id * scales[ib]).clamp(-8, 7);
            y[SCALES + ib / 2] |= ((ls + 8) << (4 * (ib % 2))) as u8;
            let dl = d * ls as f32;
            if dl == 0.0 {
                continue;
            }
            let bv = table(extra & (1 << ib) != 0);
            let xb = &xbl[BS * ib..BS * ib + BS];
            set_weights(&mut weight, ib);
            let idl = 1.0 / dl;
            let ib32 = ib / 2;
            let qs = QS + 32 * (ib32 / 4) + 16 * (ib % 2);
            for j in 0..16 {
                let ibest = best_index_iq2nl(bv, idl * xb[j]);
                y[qs + j] |= (ibest << (2 * (ib32 % 4))) as u8;
                let q = (bv[ibest] as i32 * ls) as f32;
                sumqx += weight[j] * q * xb[j];
                sumq2 += weight[j] * q * q;
            }
        }
        put_u16(
            y,
            0,
            f16_bits(FUDGE * if sumq2 > 0.0 { sumqx / sumq2 } else { d }),
        );
    }
    out
}

/// Port of dequantize_row_iq2_k.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    for (ibl, blk) in b.chunks(BYTES).enumerate() {
        let d = f16_val(get_u16(blk, 0));
        let mut extra = get_u16(blk, 2);
        let mut qs = QS;
        let mut shift = 0;
        for ib32 in 0..8 {
            let sc = blk[SCALES + ib32];
            let dl1 = d * ((sc & 0xf) as i32 - 8) as f32;
            let dl2 = d * ((sc >> 4) as i32 - 8) as f32;
            let v1 = table(extra & 1 != 0);
            let v2 = table(extra & 2 != 0);
            extra >>= 2;
            let o = ibl * QK_K + 32 * ib32;
            for j in 0..16 {
                y[o + j] = dl1 * v1[((blk[qs + j] >> shift) & 3) as usize] as f32;
                y[o + j + 16] = dl2 * v2[((blk[qs + j + 16] >> shift) & 3) as usize] as f32;
            }
            shift += 2;
            if shift == 8 {
                qs += 32;
                shift = 0;
            }
        }
    }
}

fn field(blk: &[u8], ib: usize) -> i32 {
    ((blk[SCALES + ib / 2] >> (4 * (ib % 2))) & 0xf) as i32
}

pub fn block_scales(b: &[u8], ibl: usize) -> Vec<(f32, bool)> {
    let blk = &b[ibl * BYTES..(ibl + 1) * BYTES];
    let d = f16_val(get_u16(blk, 0));
    let extra = get_u16(blk, 2);
    (0..16)
        .map(|ib| (d * (field(blk, ib) - 8) as f32, extra & (1 << ib) != 0))
        .collect()
}

pub fn decode(b: &[u8], i: usize) -> KDecode {
    let blk = &b[(i / QK_K) * BYTES..(i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let ib = j / 16;
    let ib32 = j / 32;
    let d = f16_val(get_u16(blk, 0));
    let shifted = get_u16(blk, 2) & (1 << ib) != 0;
    let f = field(blk, ib);
    let idx = ((blk[QS + 32 * (ib32 / 4) + j % 32] >> (2 * (ib32 % 4))) & 3) as usize;
    let value = table(shifted)[idx] as f32;
    KDecode::new(d, false, f, 4, d * (f - 8) as f32, "({s}&minus;8)", shifted)
        .level(idx, 2, value, &IQ2NL_VALUES, 4)
        .math(format!("{:.5}&middot;({}&minus;8) &times; {}", d, f, value))
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (<span class=\"eq-pill\">s<span class=\"bits\">4b</span></span> &minus; 8) &times; <span class=\"eq-pill\">iq2nl[sh][q]<span class=\"bits\">1b+2b</span></span>",
        "4 non-linear levels, shifted copy and 4-bit scale per 16, FP16 scale per 256.",
        "Levels {-31, -13, 1, 17}, every 16 weights pick that table or the copy shifted to {-26, -8, 6, 22} (1 bit in extra). The scale per 16 comes from an exhaustive search over all splits of the sorted block, then gets rounded to 4 bits. 2.375 bpw.",
    )
}
