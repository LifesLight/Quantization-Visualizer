//! IQ4_KSS: f32 row scale, then block_iq4_kss { u32 qs[32] } -> 128 bytes per 256, exactly 4 bpw.
//! Every u16 holds 4 IQ4_K indices squeezed into 15 bits (the 4 nibbles must have even parity,
//! so the 16th bit is implied) plus one bit of the block's scale byte. A group with odd parity
//! gets one index moved to the closest level that fixes it.

use super::common::*;
use super::iq4_k::table;
use super::iq4_ks::row_scale;
use super::lut::IQ4K_VALUES;
use super::{formula, KDecode, KType};

pub static TYPE: KType = KType {
    name: "IQ4_KSS",
    block_bytes: BYTES,
    row_meta: 4,
    sub_size: 32,
    imatrix_block: 32,
    quantize,
    dequantize,
    decode,
    block_scales,
    formula_html,
};

const BYTES: usize = 128;

/// scramble_table: the j with j ^ (j << 1) == v (v with its parity bit), i.e. the prefix xor.
fn scramble(v: u16) -> u16 {
    let mut j = 0u16;
    let mut acc = 0u16;
    for k in 0..15 {
        acc ^= (v >> k) & 1;
        j |= acc << k;
    }
    j
}

fn popcount(v: u32) -> i32 {
    v.count_ones() as i32
}

/// prune_iq4ks: odd parity, move the one index whose change costs least. The cost of the
/// current index is taken from the unshifted table, like the reference does.
fn prune(v: u16, values: &[i8], x: &[f32; 4], w: &[f32; 4], dl: f32) -> u16 {
    if popcount(v as u32) % 2 == 0 {
        return v;
    }
    let mut best_score = f32::MAX;
    let mut q4 = [0u8; 4];
    let mut jbest = 0;
    let mut bestq = 0u8;
    for j in 0..4 {
        let q = ((v >> (4 * j)) & 0xf) as u8;
        q4[j] = q;
        let pc = popcount(q as u32);
        let diff0 = dl * IQ4K_VALUES[q as usize] as f32 - x[j];
        let qmin = (q as i32 - 2).max(0);
        let qmax = (q as i32 + 2).min(15);
        for iq in qmin..=qmax {
            let qq = iq as u8;
            if qq == q {
                continue;
            }
            if (popcount(qq as u32) - pc).abs() % 2 != 0 {
                let diff1 = dl * values[qq as usize] as f32 - x[j];
                let score = w[j] * (diff1 * diff1 - diff0 * diff0);
                if score < best_score {
                    best_score = score;
                    jbest = j;
                    bestq = qq;
                }
            }
        }
    }
    q4[jbest] = bestq;
    q4[0] as u16 | (q4[1] as u16) << 4 | (q4[2] as u16) << 8 | (q4[3] as u16) << 12
}

/// The 4 weights of group k: two neighbours from each half of the 32 weights.
fn group(xb: &[f32], k: usize) -> [f32; 4] {
    [xb[2 * k], xb[2 * k + 16], xb[2 * k + 1], xb[2 * k + 17]]
}

fn indices(values: &[i8], id: f32, xv: &[f32; 4]) -> (u16, u16) {
    let (mut vp, mut vm) = (0u16, 0u16);
    for j in 0..4 {
        let al = id * xv[j];
        vp |= (best_index_iq4nl(values, al) << (4 * j)) as u16;
        vm |= (best_index_iq4nl(values, -al) << (4 * j)) as u16;
    }
    (vp, vm)
}

/// Port of quantize_row_iq4_kss_impl.
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Vec<u8> {
    const BS: usize = 32;
    const NTRY: i32 = 7;
    let nblock = x.len() / QK_K;
    let mut out = vec![0u8; 4 + nblock * BYTES];
    let mut all_scales = vec![0.0f32; nblock * 8];
    let mut weight = [0.0f32; BS];
    let values = table(false);
    let shifted_values = table(true);
    let sigma2_of = |xbl: &[f32]| sum_sq(xbl) * (2.0 / QK_K as f32);
    let set_weights = |weight: &mut [f32; BS], ibl: usize, ib: usize, sigma2: f32| {
        let xb = &x[ibl * QK_K + ib * BS..ibl * QK_K + ib * BS + BS];
        for j in 0..BS {
            weight[j] = match qw {
                Some(qw) => qw[ibl * QK_K + ib * BS + j] * (sigma2 + xb[j] * xb[j]).sqrt(),
                None => xb[j] * xb[j],
            };
        }
    };

    let mut amax_scale = 0.0f32;
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = sigma2_of(xbl);
        for ib in 0..8 {
            let xb = &xbl[ib * BS..ib * BS + BS];
            set_weights(&mut weight, ibl, ib, sigma2);
            let (amax, max) = max_abs_signed(xb);
            if amax < 1e-16 {
                all_scales[ibl * 8 + ib] = 0.0;
                continue;
            }
            let mut best = 0.0f32;
            let mut d = -max / IQ4K_VALUES[0] as f32;
            for itry in -NTRY..=NTRY {
                for t in [values, shifted_values] {
                    let id = (itry + t[0] as i32) as f32 / max;
                    let this_d = 1.0 / id;
                    let (mut sqx_p, mut sq2_p, mut sqx_m, mut sq2_m) =
                        (0.0f32, 0.0f32, 0.0f32, 0.0f32);
                    for k in 0..BS / 4 {
                        let xv = group(xb, k);
                        let wv = group(&weight, k);
                        let (vp, vm) = indices(t, id, &xv);
                        let vp = prune(vp, t, &xv, &wv, this_d);
                        let vm = prune(vm, t, &xv, &wv, this_d);
                        for j in 0..4 {
                            let q = t[((vp >> (4 * j)) & 0xf) as usize] as f32;
                            sqx_p += wv[j] * q * xv[j];
                            sq2_p += wv[j] * q * q;
                            let q = t[((vm >> (4 * j)) & 0xf) as usize] as f32;
                            sqx_m += wv[j] * q * xv[j];
                            sq2_m += wv[j] * q * q;
                        }
                    }
                    if sq2_p > 0.0 && sqx_p * sqx_p > best * sq2_p {
                        d = sqx_p / sq2_p;
                        best = d * sqx_p;
                    }
                    if sq2_m > 0.0 && sqx_m * sqx_m > best * sq2_m {
                        d = sqx_m / sq2_m;
                        best = d * sqx_m;
                    }
                }
            }
            all_scales[ibl * 8 + ib] = d;
            amax_scale = amax_scale.max(d.abs());
        }
    }
    let d = amax_scale / 127.0;
    out[..4].copy_from_slice(&(1.0 * d).to_le_bytes());
    if d == 0.0 {
        return out;
    }
    let id = 1.0 / d;
    let mut sumqx = 0.0f32;
    let mut sumq2 = 0.0f32;
    for ibl in 0..nblock {
        let xbl = &x[ibl * QK_K..(ibl + 1) * QK_K];
        let sigma2 = sigma2_of(xbl);
        for ib in 0..8 {
            let xb = &xbl[ib * BS..ib * BS + BS];
            set_weights(&mut weight, ibl, ib, sigma2);
            let mut l = nearest_int(0.5 * (id * all_scales[ibl * 8 + ib] + 127.0));
            l = (l.clamp(0, 127) << 1) - 127;
            let q16 = 4 + ibl * BYTES + 16 * ib;
            if l == 0 {
                l += 127;
                for k in 0..8 {
                    put_u16(&mut out, q16 + 2 * k, ((l >> k) & 1) as u16);
                }
                continue;
            }
            let dl = d * l as f32;
            let idl = 1.0 / dl;
            let (mut mse_p, mut mse_m) = (0.0f32, 0.0f32);
            let mut vps = [0u16; 8];
            let mut vms = [0u16; 8];
            for k in 0..BS / 4 {
                let xv = group(xb, k);
                let wv = group(&weight, k);
                let (mut vp, mut vm) = (0u16, 0u16);
                for j in 0..4 {
                    let al = idl * xv[j];
                    vp |= (best_index_iq4nl(values, al) << (4 * j)) as u16;
                    vm |= (best_index_iq4nl(shifted_values, al) << (4 * j)) as u16;
                }
                vp = prune(vp, values, &xv, &wv, dl);
                vm = prune(vm, shifted_values, &xv, &wv, dl);
                for j in 0..4 {
                    let q = values[((vp >> (4 * j)) & 0xf) as usize] as f32;
                    mse_p += wv[j] * (xv[j] - dl * q) * (xv[j] - dl * q);
                    let q = shifted_values[((vm >> (4 * j)) & 0xf) as usize] as f32;
                    mse_m += wv[j] * (xv[j] - dl * q) * (xv[j] - dl * q);
                }
                vps[k] = vp;
                vms[k] = vm;
            }
            let use_m = mse_m < mse_p;
            let (v, bv) = if use_m {
                (&vms, shifted_values)
            } else {
                (&vps, values)
            };
            for k in 0..BS / 4 {
                let xv = group(xb, k);
                let wv = group(&weight, k);
                for j in 0..4 {
                    let q = (bv[((v[k] >> (4 * j)) & 0xf) as usize] as i32 * l) as f32;
                    sumqx += wv[j] * q * xv[j];
                    sumq2 += wv[j] * q * q;
                }
            }
            l += 127;
            if use_m {
                l |= 1;
            }
            for k in 0..8 {
                let val = scramble(v[k] & 0x7fff);
                put_u16(&mut out, q16 + 2 * k, (val << 1) | ((l >> k) & 1) as u16);
            }
        }
    }
    if sumq2 > 0.0 {
        out[..4].copy_from_slice(&(1.0 * sumqx / sumq2).to_le_bytes());
    }
    out
}

/// The 8 u16 of block ib unscrambled to 4 nibbles each, and its scale byte.
fn unpack(blk: &[u8], ib: usize) -> ([u16; 8], i32) {
    let mut aux = [0u16; 8];
    let mut ls = 0i32;
    for k in 0..8 {
        let q = get_u16(blk, 16 * ib + 2 * k);
        let a = q & 0xfffe;
        aux[k] = a ^ (a >> 1);
        ls |= ((q & 1) as i32) << k;
    }
    (aux, ls)
}

/// Index of weight j (0..32) of a block from the unscrambled u16s.
fn nibble(aux: &[u16; 8], j: usize) -> usize {
    let byte = aux[(j % 16) / 2].to_le_bytes()[(j % 16) % 2];
    if j < 16 {
        (byte & 0xf) as usize
    } else {
        (byte >> 4) as usize
    }
}

/// Port of dequantize_row_iq4_kss.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = row_scale(b);
    for (ibl, blk) in b[4..].chunks(BYTES).enumerate() {
        for ib in 0..8 {
            let (aux, ls) = unpack(blk, ib);
            let v = table(ls & 1 != 0);
            let dl = d * ((ls & 254) - 127) as f32;
            for j in 0..32 {
                y[ibl * QK_K + 32 * ib + j] = dl * v[nibble(&aux, j)] as f32;
            }
        }
    }
}

pub fn block_scales(b: &[u8], ibl: usize) -> Vec<(f32, bool)> {
    let d = row_scale(b);
    let blk = &b[4 + ibl * BYTES..4 + (ibl + 1) * BYTES];
    (0..8)
        .map(|ib| {
            let (_, ls) = unpack(blk, ib);
            (d * ((ls & 254) - 127) as f32, ls & 1 != 0)
        })
        .collect()
}

pub fn decode(b: &[u8], i: usize) -> KDecode {
    let d = row_scale(b);
    let blk = &b[4 + (i / QK_K) * BYTES..4 + (i / QK_K + 1) * BYTES];
    let j = i % QK_K;
    let ib = j / 32;
    let (aux, ls) = unpack(blk, ib);
    let shifted = ls & 1 != 0;
    let idx = nibble(&aux, j % 32);
    let value = table(shifted)[idx] as f32;
    let f = ls & 254;
    KDecode::new(
        d,
        true,
        f,
        7,
        d * (f - 127) as f32,
        "({s}&minus;127)",
        shifted,
    )
    .level(idx, 4, value, &IQ4K_VALUES, 16)
    .math(format!(
        "{:.6}&middot;({}&minus;127) &times; {}",
        d, f, value
    ))
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">32b</span></span> &times; (<span class=\"eq-pill\">s<span class=\"bits\">7b</span></span> &minus; 127) &times; <span class=\"eq-pill\">iq4k[sh][q]<span class=\"bits\">1b+3.75b</span></span>",
        "IQ4_KS squeezed to 4 bpw: 4 indices share 15 bits, scale bits ride along.",
        "Same levels, shifted copy and scale as IQ4_KS, but 4 indices only get 15 bits: their nibbles must have even parity, a group with odd parity gets one index moved to a neighbouring level. The freed bit of every u16 carries one bit of the block's scale byte. The shifted table is picked per 32 by comparing the error of both. Exactly 4 bpw plus 32 bits per row.",
    )
}
