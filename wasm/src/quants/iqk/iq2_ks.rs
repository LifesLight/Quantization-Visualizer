//! IQ2_KS: fp16 row scale, then block_iq2_ks { u16 extra; u8 scales[4]; u8 qs[64] } -> 70 bytes.
//! IQ2_K levels per 32 weights, 5 bit scale (4 bits in scales, the 5th in the top byte of extra),
//! shift bits in the low byte of extra. This is the AVX2 path llama-quantize takes on x86.

use super::common::*;
use super::lut::IQ2NL_VALUES;
use super::{formula, KDecode, KType};

pub static TYPE: KType = KType {
    name: "IQ2_KS",
    block_bytes: BYTES,
    row_meta: 2,
    sub_size: 32,
    imatrix_block: 32,
    quantize,
    dequantize,
    decode,
    block_scales,
    formula_html,
};

const BYTES: usize = 70;
const SCALES: usize = 2;
const QS: usize = 6;

fn table(shifted: bool) -> &'static [i8] {
    if shifted {
        &IQ2NL_VALUES[4..]
    } else {
        &IQ2NL_VALUES[..4]
    }
}

/// Nearest of the 4 levels times d for every lane, first one wins on ties.
fn nearest(d: f32, xb: &[f32], values: &[i8], sign: f32) -> [usize; 32] {
    let mut best = [0.0f32; 32];
    let mut idx = [0usize; 32];
    for j in 0..4 {
        let val = d * values[j] as f32;
        for k in 0..32 {
            let diff = if sign > 0.0 { xb[k] - val } else { xb[k] + val };
            let diff = diff * diff;
            if j == 0 || diff < best[k] {
                best[k] = diff;
                idx[k] = j;
            }
        }
    }
    idx
}

/// compute_1block_iq2ks: best scale for d and -d, returns (indices, d, score).
fn compute_1block(d: f32, xb: &[f32], w: &[f32], values: &[i8]) -> Option<([usize; 32], f32, f32)> {
    let idx1 = nearest(d, xb, values, 1.0);
    let idx2 = nearest(d, xb, values, -1.0);
    let mut sqx = [[0.0f32; 8]; 2];
    let mut sq2 = [[0.0f32; 8]; 2];
    for k in 0..32 {
        let l = k % 8;
        for (c, idx) in [&idx1, &idx2].iter().enumerate() {
            let q = values[idx[k]] as f32;
            let wq = w[k] * q;
            sqx[c][l] = wq.mul_add(xb[k], sqx[c][l]);
            sq2[c][l] = wq.mul_add(q, sq2[c][l]);
        }
    }
    let (sumqx_1, sumq2_1) = (hsum8(&sqx[0]), hsum8(&sq2[0]));
    let (sumqx_2, sumq2_2) = (hsum8(&sqx[1]), hsum8(&sq2[1]));
    let mut result = None;
    if sumq2_1 > 0.0 {
        let d = sumqx_1 / sumq2_1;
        result = Some((idx1, d, sumqx_1 * d));
    }
    let take2 = match result {
        None => sumq2_2 > 0.0,
        Some((_, _, score)) => sumq2_2 > 0.0 && sumqx_2 * sumqx_2 > score * sumq2_2,
    };
    if take2 {
        let d = sumqx_2 / sumq2_2;
        result = Some((idx2, d, sumqx_2 * d));
    }
    result
}

/// compute_1block_iq2ks_rmse: weighted error of the nearest levels for one sign of d.
fn rmse(d: f32, xb: &[f32], w: &[f32], values: &[i8]) -> (f32, [usize; 32]) {
    let idx = nearest(d, xb, values, 1.0);
    let mut acc = [0.0f32; 8];
    for k in 0..32 {
        let q = values[idx[k]] as f32;
        let diff = (-d).mul_add(q, xb[k]);
        acc[k % 8] = (w[k] * diff).mul_add(diff, acc[k % 8]);
    }
    (hsum8(&acc), idx)
}

/// Port of quantize_row_iq2_ks_fast_impl.
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const BS: usize = 32;
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; 2 + nblock * BYTES];
    let nsub = nblock * (QK_K / BS);
    let mut all_scales = vec![0.0f32; nsub];
    let mut all_sw = vec![0.0f32; nsub];
    let mut all_ls = vec![0i8; nsub];
    let mut weight = [0.0f32; BS];

    let weights = |weight: &mut [f32; BS], ibl: usize, ib: usize, sigma2: f32| {
        let xb = &x[ibl * QK_K + BS * ib..ibl * QK_K + BS * ib + BS];
        for j in 0..BS {
            weight[j] = match qw {
                Some(qw) => qw[ibl * QK_K + ib * BS + j] * (sigma2 + xb[j] * xb[j]).sqrt(),
                None => 0.25 * sigma2 + xb[j] * xb[j],
            };
        }
    };

    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = 1.5 * sum_sq(xbl) / QK_K as f32;
        let mut extra = 0u16;
        for ib in 0..QK_K / BS {
            let si = ibl * (QK_K / BS) + ib;
            let xb = &xbl[BS * ib..BS * ib + BS];
            weights(&mut weight, ibl, ib, sigma2);
            let (mut amax, mut max, mut sumw) = (0.0f32, 0.0f32, 0.0f32);
            for j in 0..BS {
                let ax = xb[j].abs();
                if ax > amax {
                    amax = ax;
                    max = xb[j];
                }
                sumw += weight[j];
            }
            all_sw[si] = sumw;
            if amax < 1e-14 {
                all_scales[si] = 0.0;
                continue;
            }
            let mut d = max / IQ2NL_VALUES[7] as f32;
            let mut best = 0.0f32;
            if let Some((_, td, score)) = compute_1block(d, xb, &weight, table(false)) {
                best = score;
                d = td;
            }
            for itry in -13..=13 {
                let dt = max / (IQ2NL_VALUES[0] as f32 + 0.5 * itry as f32);
                if let Some((_, td, score)) = compute_1block(dt, xb, &weight, table(false)) {
                    if score > best {
                        best = score;
                        d = td;
                    }
                }
            }
            let mut is_shifted = false;
            for itry in -13..=13 {
                let dt = max / (IQ2NL_VALUES[4] as f32 + 0.5 * itry as f32);
                if let Some((_, td, score)) = compute_1block(dt, xb, &weight, table(true)) {
                    if score > best {
                        best = score;
                        d = td;
                        is_shifted = true;
                    }
                }
            }
            all_scales[si] = d;
            if is_shifted {
                extra |= 1 << ib;
            }
        }
        put_u16(&mut out, 2 + ibl * BYTES, extra);
    }

    let d = make_qx_quants(16, &all_scales, &mut all_ls, &all_sw);
    if d == 0.0 {
        return out;
    }

    let mut vsumqx = [0.0f32; 8];
    let mut vsumq2 = [0.0f32; 8];
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = 1.5 * sum_sq(xbl) / QK_K as f32;
        let y = &mut out[2 + ibl * BYTES..2 + (ibl + 1) * BYTES];
        let mut idx = [[0usize; 32]; 4];
        for ib in 0..QK_K / BS {
            let si = ibl * (QK_K / BS) + ib;
            let bv = table(get_u16(y, 0) & (1 << ib) != 0);
            let xb = &xbl[BS * ib..BS * ib + BS];
            weights(&mut weight, ibl, ib, sigma2);
            let ls = all_ls[si] as i32 - 16;
            let dl = d * ls as f32;
            let (rmse1, mut idx1) = rmse(dl, xb, &weight, bv);
            if all_ls[si] > 0 && dl > all_scales[si] {
                let (rmse2, idx2) = rmse(d * (all_ls[si] as i32 - 17) as f32, xb, &weight, bv);
                if rmse2 < rmse1 {
                    all_ls[si] -= 1;
                    idx1 = idx2;
                }
            } else if all_ls[si] < 15 && dl < all_scales[si] {
                let (rmse2, idx2) = rmse(d * (all_ls[si] as i32 - 15) as f32, xb, &weight, bv);
                if rmse2 < rmse1 {
                    all_ls[si] += 1;
                    idx1 = idx2;
                }
            }
            let vd = (all_ls[si] as i32 - 16) as f32;
            for k in 0..32 {
                let vq = vd * bv[idx1[k]] as f32;
                let wvq = weight[k] * vq;
                vsumqx[k % 8] = wvq.mul_add(xb[k], vsumqx[k % 8]);
                vsumq2[k % 8] = wvq.mul_add(vq, vsumq2[k % 8]);
            }
            let ls = all_ls[si] as u16;
            y[SCALES + ib / 2] |= ((ls & 0xf) << (4 * (ib % 2))) as u8;
            let e = get_u16(y, 0) | ((ls >> 4) << (8 + ib));
            put_u16(y, 0, e);
            idx[ib % 4] = idx1;
            if ib % 4 == 3 {
                for j in 0..32 {
                    y[QS + 32 * (ib / 4) + j] =
                        (idx[0][j] | (idx[1][j] << 2) | (idx[2][j] << 4) | (idx[3][j] << 6)) as u8;
                }
            }
        }
    }
    let sumqx = hsum8(&vsumqx);
    let sumq2 = hsum8(&vsumq2);
    put_u16(
        &mut out,
        0,
        f16_bits(1.0 * if sumq2 > 0.0 { sumqx / sumq2 } else { d }),
    );
    out
}

/// Port of dequantize_row_iq2_ks.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(get_u16(b, 0));
    for (ibl, blk) in b[2..].chunks(BYTES).enumerate() {
        let mut extra = get_u16(blk, 0);
        let mut qs = QS;
        let mut shift = 0;
        for ib64 in 0..4 {
            let sc = blk[SCALES + ib64];
            let dl1 = d * (((sc & 0xf) as u16 | ((extra >> 4) & 0x10)) as i32 - 16) as f32;
            let dl2 = d * (((sc >> 4) as u16 | ((extra >> 5) & 0x10)) as i32 - 16) as f32;
            let v1 = table(extra & 1 != 0);
            let v2 = table(extra & 2 != 0);
            extra >>= 2;
            let o = ibl * QK_K + 64 * ib64;
            for j in 0..32 {
                y[o + j] = dl1 * v1[((blk[qs + j] >> shift) & 3) as usize] as f32;
                y[o + j + 32] = dl2 * v2[((blk[qs + j] >> (shift + 2)) & 3) as usize] as f32;
            }
            shift += 4;
            if shift == 8 {
                qs += 32;
                shift = 0;
            }
        }
    }
}

fn field(blk: &[u8], ib: usize) -> i32 {
    let lo = (blk[SCALES + ib / 2] >> (4 * (ib % 2))) & 0xf;
    let hi = (get_u16(blk, 0) >> (8 + ib)) & 1;
    lo as i32 | (hi as i32) << 4
}

pub fn block_scales(b: &[u8], ibl: usize) -> Vec<(f32, bool)> {
    let d = f16_val(get_u16(b, 0));
    let blk = &b[2 + ibl * BYTES..2 + (ibl + 1) * BYTES];
    let extra = get_u16(blk, 0);
    (0..8)
        .map(|ib| (d * (field(blk, ib) - 16) as f32, extra & (1 << ib) != 0))
        .collect()
}

pub fn decode(b: &[u8], i: usize) -> KDecode {
    let d = f16_val(get_u16(b, 0));
    let blk = &b[2 + (i / QK_K) * BYTES..2 + (i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let ib = j / 32;
    let shifted = get_u16(blk, 0) & (1 << ib) != 0;
    let f = field(blk, ib);
    let idx = ((blk[QS + 32 * (ib / 4) + j % 32] >> (2 * (ib % 4))) & 3) as usize;
    let value = table(shifted)[idx] as f32;
    KDecode::new(
        d,
        true,
        f,
        5,
        d * (f - 16) as f32,
        "({s}&minus;16)",
        shifted,
    )
    .level(idx, 2, value, &IQ2NL_VALUES, 4)
    .math(format!(
        "{:.5}&middot;({}&minus;16) &times; {}",
        d, f, value
    ))
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (<span class=\"eq-pill\">s<span class=\"bits\">5b</span></span> &minus; 16) &times; <span class=\"eq-pill\">iq2nl[sh][q]<span class=\"bits\">1b+2b</span></span>",
        "IQ2_K levels with shifted copy and 5-bit scale per 32, one FP16 scale per row.",
        "Same 4 levels and shifted copy as IQ2_K, but per 32 weights and with no FP16 scale per 256: the 5-bit scales of the whole row share one FP16 scale. Each scale is searched over 27 trial values per table, then rounded with one step of rmse correction. 2.1875 bpw plus 16 bits per row.",
    )
}
