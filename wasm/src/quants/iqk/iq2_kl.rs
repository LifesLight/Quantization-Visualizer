//! IQ2_KL: fp16 row scale, then block_iq2_kl { u16 scales_h; u8 scales_l[4]; u8 qs[64]; u8 qh[16] }
//! -> 86 bytes per 256. Pairs of weights take one of 32 (x, y) points picked from the 8x8 grid
//! of IQ3_K levels (5 bit index), 6 bit scale per 32.

use super::common::*;
use super::lut::{IQ2KL_K_INDEX, IQ2KL_NEIGHBOURS, IQ2KL_VALUES, IQ3NL_VALUES};
use super::{formula, KDecode, KType};

pub static TYPE: KType = KType {
    name: "IQ2_KL",
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

const BYTES: usize = 86;
const SCALES_L: usize = 2;
const QS: usize = 6;
const QH: usize = 70;
const FUDGE: f32 = 1.025;

/// The 32 allowed pairs, as the quantizer builds them from k_index.
fn grid() -> [(i8, i8); 32] {
    let mut g = [(0i8, 0i8); 32];
    for j in 0..64 {
        let i = IQ2KL_K_INDEX[j];
        if i >= 0 {
            g[i as usize] = (IQ3NL_VALUES[j / 8], IQ3NL_VALUES[j % 8]);
        }
    }
    g
}

fn index(g: &[(i8, i8); 32], id: f32, x1: f32, x2: f32, w1: f32, w2: f32) -> usize {
    let sx1 = id * x1;
    let sx2 = id * x2;
    let l1 = best_index_iq3nl(&IQ3NL_VALUES, sx1);
    let l2 = best_index_iq3nl(&IQ3NL_VALUES, sx2);
    let i = IQ2KL_K_INDEX[8 * l1 + l2];
    if i >= 0 {
        return i as usize;
    }
    let mut best = f32::MAX;
    let mut ibest = 0;
    for &n in &IQ2KL_NEIGHBOURS[(-i - 1) as usize] {
        let diff1 = g[n as usize].0 as f32 - sx1;
        let diff2 = g[n as usize].1 as f32 - sx2;
        let score = w1 * diff1 * diff1 + w2 * diff2 * diff2;
        if score < best {
            best = score;
            ibest = n as usize;
        }
    }
    ibest
}

/// Port of quantize_row_iq2_kl_impl.
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const BS: usize = 32;
    const NTRY: i32 = 5;
    let g = grid();
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; 2 + nblock * BYTES];
    let mut all_scales = vec![0.0f32; nblock * (QK_K / BS)];
    let mut weight = [0.0f32; BS];
    let v0 = IQ3NL_VALUES[0] as f32;

    let weights = |weight: &mut [f32; BS], ibl: usize, ib: usize, sigma2: f32| {
        let xb = &x[ibl * QK_K + ib * BS..ibl * QK_K + ib * BS + BS];
        for j in 0..BS {
            weight[j] = match qw {
                Some(qw) => qw[ibl * QK_K + ib * BS + j] * (sigma2 + xb[j] * xb[j]).sqrt(),
                None => xb[j].abs(),
            };
        }
    };
    let sigma2_of = |xbl: &[f32]| sum_sq(xbl) * (2.25 / QK_K as f32);
    // sumqx, sumq2 of the pair indices for +id and -id
    let sums = |id: f32, xb: &[f32], w: &[f32; BS]| {
        let (mut sqx_p, mut sq2_p, mut sqx_m, mut sq2_m) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for j in (0..BS).step_by(2) {
            let (w1, w2) = (w[j], w[j + 1]);
            let i = index(&g, id, xb[j], xb[j + 1], w1, w2);
            let (q1, q2) = (g[i].0 as f32, g[i].1 as f32);
            sqx_p += w1 * q1 * xb[j] + w2 * q2 * xb[j + 1];
            sq2_p += w1 * q1 * q1 + w2 * q2 * q2;
            let i = index(&g, -id, xb[j], xb[j + 1], w1, w2);
            let (q1, q2) = (g[i].0 as f32, g[i].1 as f32);
            sqx_m += w1 * q1 * xb[j] + w2 * q2 * xb[j + 1];
            sq2_m += w1 * q1 * q1 + w2 * q2 * q2;
        }
        (sqx_p, sq2_p, sqx_m, sq2_m)
    };

    let (mut max_scale, mut max_abs_scale) = (0.0f32, 0.0f32);
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = sigma2_of(xbl);
        for ib in 0..QK_K / BS {
            let xb = &xbl[ib * BS..ib * BS + BS];
            weights(&mut weight, ibl, ib, sigma2);
            let (amax, max) = max_abs_signed(xb);
            let si = ibl * (QK_K / BS) + ib;
            if amax < 1e-16 {
                all_scales[si] = 0.0;
                continue;
            }
            let d0 = -max / v0;
            let (sqx_p, sq2_p, sqx_m, sq2_m) = sums(1.0 / d0, xb, &weight);
            let mut d = sqx_p / sq2_p;
            let mut best = d * sqx_p;
            if sq2_m > 0.0 && sqx_m * sqx_m > best * sq2_m {
                d = sqx_m / sq2_m;
                best = d * sqx_m;
            }
            for itry in -NTRY..=NTRY {
                let id = (itry + IQ3NL_VALUES[0] as i32) as f32 / max;
                let (sqx_p, sq2_p, sqx_m, sq2_m) = sums(id, xb, &weight);
                if sq2_p > 0.0 && sqx_p * sqx_p > best * sq2_p {
                    d = sqx_p / sq2_p;
                    best = d * sqx_p;
                }
                if sq2_m > 0.0 && sqx_m * sqx_m > best * sq2_m {
                    d = sqx_m / sq2_m;
                    best = d * sqx_m;
                }
            }
            all_scales[si] = d;
            if d.abs() > max_abs_scale {
                max_abs_scale = d.abs();
                max_scale = d;
            }
        }
    }

    if max_abs_scale == 0.0 {
        put_u16(&mut out, 0, f16_bits(0.0));
        return out;
    }

    let mut d = -max_scale / 32.0;
    let id = 1.0 / d;
    let mut sumqx = 0.0f32;
    let mut sumq2 = 0.0f32;
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = sigma2_of(xbl);
        let y = &mut out[2 + ibl * BYTES..2 + (ibl + 1) * BYTES];
        for ib in 0..QK_K / BS {
            let xb = &xbl[ib * BS..ib * BS + BS];
            weights(&mut weight, ibl, ib, sigma2);
            let ls = nearest_int(id * all_scales[ibl * (QK_K / BS) + ib]).clamp(-32, 31);
            let (lsmin, lsmax) = ((ls - 1).max(-32), (ls + 1).min(31));
            let mut best_score = f32::MAX;
            let mut best_ls = ls;
            for ils in lsmin..=lsmax {
                let dl = d * ils as f32;
                let idl = if dl != 0.0 { 1.0 / dl } else { 0.0 };
                let mut score = 0.0f32;
                for j in 0..BS / 2 {
                    let (w1, w2) = (weight[2 * j], weight[2 * j + 1]);
                    let i = index(&g, idl, xb[2 * j], xb[2 * j + 1], w1, w2);
                    let diff1 = dl * g[i].0 as f32 - xb[2 * j];
                    let diff2 = dl * g[i].1 as f32 - xb[2 * j + 1];
                    score += w1 * diff1 * diff1 + w2 * diff2 * diff2;
                }
                if score < best_score {
                    best_score = score;
                    best_ls = ils;
                }
            }
            let ls = best_ls;
            let uls = (ls + 32) as u16;
            y[SCALES_L + ib % 4] |= ((uls & 0xf) << (4 * (ib / 4))) as u8;
            let sh = get_u16(y, 0) | ((uls >> 4) << (2 * ib));
            put_u16(y, 0, sh);
            if ls == 0 {
                continue;
            }
            let dl = d * ls as f32;
            let idl = 1.0 / dl;
            for j in 0..BS / 2 {
                let (w1, w2) = (weight[2 * j], weight[2 * j + 1]);
                let i = index(&g, idl, xb[2 * j], xb[2 * j + 1], w1, w2);
                y[QS + 16 * (ib / 2) + j] |= ((i & 0xf) << (4 * (ib % 2))) as u8;
                y[QH + j] |= ((i >> 4) << ib) as u8;
                let q1 = (ls * g[i].0 as i32) as f32;
                let q2 = (ls * g[i].1 as i32) as f32;
                sumqx += w1 * q1 * xb[2 * j] + w2 * q2 * xb[2 * j + 1];
                sumq2 += w1 * q1 * q1 + w2 * q2 * q2;
            }
        }
    }
    if sumq2 > 0.0 {
        d = sumqx / sumq2;
    }
    put_u16(&mut out, 0, f16_bits(FUDGE * d));
    out
}

fn field(blk: &[u8], ib: usize) -> i32 {
    let lo = (blk[SCALES_L + ib % 4] >> (4 * (ib / 4))) & 0xf;
    let hi = (get_u16(blk, 0) >> (2 * ib)) & 3;
    lo as i32 | (hi as i32) << 4
}

fn pair(i: usize) -> (i8, i8) {
    let v = IQ2KL_VALUES[i].to_le_bytes();
    (v[0] as i8, v[1] as i8)
}

/// Port of dequantize_row_iq2_kl.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(get_u16(b, 0));
    for (ibl, blk) in b[2..].chunks(BYTES).enumerate() {
        for ib64 in 0..4 {
            let dl1 = d * (field(blk, 2 * ib64) - 32) as f32;
            let dl2 = d * (field(blk, 2 * ib64 + 1) - 32) as f32;
            let o = ibl * QK_K + 64 * ib64;
            for j in 0..16 {
                let qs = blk[QS + 16 * ib64 + j];
                let qh = blk[QH + j];
                let v1 = pair((qs & 0xf) as usize | (((qh >> (2 * ib64)) & 1) as usize) << 4);
                let v2 = pair((qs >> 4) as usize | (((qh >> (2 * ib64 + 1)) & 1) as usize) << 4);
                y[o + 2 * j] = dl1 * v1.0 as f32;
                y[o + 2 * j + 1] = dl1 * v1.1 as f32;
                y[o + 2 * j + 32] = dl2 * v2.0 as f32;
                y[o + 2 * j + 33] = dl2 * v2.1 as f32;
            }
        }
    }
}

pub fn block_scales(b: &[u8], ibl: usize) -> Vec<(f32, bool)> {
    let d = f16_val(get_u16(b, 0));
    let blk = &b[2 + ibl * BYTES..2 + (ibl + 1) * BYTES];
    (0..8)
        .map(|ib| (d * (field(blk, ib) - 32) as f32, false))
        .collect()
}

pub fn decode(b: &[u8], i: usize) -> KDecode {
    let d = f16_val(get_u16(b, 0));
    let blk = &b[2 + (i / QK_K) * BYTES..2 + (i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let ib = j / 32;
    let p = (j % 32) / 2;
    let qs = blk[QS + 16 * (ib / 2) + p];
    let qh = blk[QH + p];
    let idx = ((qs >> (4 * (ib % 2))) & 0xf) as usize | (((qh >> ib) & 1) as usize) << 4;
    let (a, c) = pair(idx);
    let value = if j % 2 == 0 { a } else { c } as f32;
    let f = field(blk, ib);
    let mut dec = KDecode::new(d, true, f, 6, d * (f - 32) as f32, "({s}&minus;32)", false)
        .level(idx, 5, value, &IQ3NL_VALUES[..8], 8)
        .math(format!(
            "{:.5}&middot;({}&minus;32) &times; {}",
            d, f, value
        ));
    dec.mark = IQ3NL_VALUES[..8]
        .iter()
        .position(|&v| v as f32 == value)
        .unwrap_or(0);
    dec.pair = Some((a as f32, c as f32, j % 2));
    dec
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (<span class=\"eq-pill\">s<span class=\"bits\">6b</span></span> &minus; 32) &times; <span class=\"eq-pill\">pair[q]<sub>j</sub><span class=\"bits\">5b/2</span></span>",
        "Pairs of weights pick 1 of 32 points on the IQ3_K grid, 6-bit scale per 32, FP16 per row.",
        "Each pair of weights stores a 5 bit index into 32 allowed (x, y) points, all combinations of the 8 IQ3_K levels that made the cut. A pair whose rounded point is not allowed takes the best of 8 neighbours. 6-bit scale per 32 weights, tried at &plusmn;1 around the rounded value, one FP16 scale per row with a 1.025 fudge. 2.6875 bpw plus 16 bits per row.",
    )
}
