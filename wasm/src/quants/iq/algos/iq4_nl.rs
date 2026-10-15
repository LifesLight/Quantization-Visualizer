//! IQ4_NL: block_iq4_nl { fp16 d; u8 qs[16] } -> 18 bytes per 32 weights.
//! Each weight is a 4 bit index into the fixed non-linear table kvalues_iq4nl.

use super::*;
use crate::quants::iq::lut;

pub static TYPE: IqType = IqType {
    name: "IQ4_NL",
    super_size: 32,
    sub_size: 32,
    group_size: 1,
    bytes: 18,
    needs_imatrix: false,
    quantize,
    dequantize,
    decode,
    sub_scales,
    formula_html,
};

pub struct Iq4Out {
    pub dh: u16,
    pub scales_h: u16,
    pub scales_l: [u8; QK_K / 64],
    pub q4: Vec<u8>,
}

/// Port of quantize_row_iq4_nl_impl, shared by IQ4_NL (one 32 block) and IQ4_XS (8 blocks of 32).
pub fn quantize_impl(super_size: usize, x: &[f32], qw: Option<&[f32]>, ntry: i32) -> Iq4Out {
    const BLOCK: usize = 32;
    let values = &lut::KVALUES_IQ4NL;
    let nb = super_size / BLOCK;

    let mut sigma2 = 0.0f32;
    for j in 0..super_size {
        sigma2 += x[j] * x[j];
    }
    sigma2 *= 2.0 / super_size as f32;

    let mut out = Iq4Out {
        dh: 0,
        scales_h: 0,
        scales_l: [0; QK_K / 64],
        q4: vec![0u8; super_size / 2],
    };
    let mut scales = vec![0.0f32; nb];
    let mut weight = [0.0f32; BLOCK];
    let mut ls = vec![0u8; super_size];

    let mut max_scale = 0.0f32;
    let mut amax_scale = 0.0f32;
    for ib in 0..nb {
        let xb = &x[ib * BLOCK..ib * BLOCK + BLOCK];
        match qw {
            Some(qw) => {
                for j in 0..BLOCK {
                    weight[j] = qw[ib * BLOCK + j] * (sigma2 + xb[j] * xb[j]).sqrt();
                }
            }
            None => {
                for j in 0..BLOCK {
                    weight[j] = xb[j] * xb[j];
                }
            }
        }
        let mut amax = 0.0f32;
        let mut max = 0.0f32;
        for j in 0..BLOCK {
            let ax = xb[j].abs();
            if ax > amax {
                amax = ax;
                max = xb[j];
            }
        }
        if amax < GROUP_MAX_EPS {
            scales[ib] = 0.0;
            continue;
        }
        let mut d = if ntry > 0 {
            -max / values[0] as f32
        } else {
            max / values[0] as f32
        };
        let mut id = 1.0 / d;
        let mut sumqx = 0.0f32;
        let mut sumq2 = 0.0f32;
        for j in 0..BLOCK {
            let al = id * xb[j];
            let l = best_index_int8(values, al);
            ls[ib * BLOCK + j] = l as u8;
            let q = values[l] as f32;
            let w = weight[j];
            sumqx += w * q * xb[j];
            sumq2 += w * q * q;
        }
        d = if sumq2 > 0.0 { sumqx / sumq2 } else { 0.0 };
        let mut best = d * sumqx;
        for itry in -ntry..=ntry {
            id = (itry + values[0] as i32) as f32 / max;
            sumqx = 0.0;
            sumq2 = 0.0;
            for j in 0..BLOCK {
                let al = id * xb[j];
                let l = best_index_int8(values, al);
                let q = values[l] as f32;
                let w = weight[j];
                sumqx += w * q * xb[j];
                sumq2 += w * q * q;
            }
            if sumq2 > 0.0 && sumqx * sumqx > best * sumq2 {
                d = sumqx / sumq2;
                best = d * sumqx;
            }
        }
        scales[ib] = d;
        let abs_d = d.abs();
        if abs_d > amax_scale {
            amax_scale = abs_d;
            max_scale = d;
        }
    }

    if nb > 1 {
        let d = -max_scale / 32.0;
        out.dh = f16_bits(d);
        let id = if d != 0.0 { 1.0 / d } else { 0.0 };
        for ib in 0..nb {
            let mut l = nearest_int(id * scales[ib]).clamp(-32, 31);
            let dl = d * l as f32;
            let idl = if dl != 0.0 { 1.0 / dl } else { 0.0 };
            for j in 0..BLOCK {
                ls[ib * BLOCK + j] = best_index_int8(values, idl * x[ib * BLOCK + j]) as u8;
            }
            l += 32;
            let l_l = (l & 0xf) as u8;
            let l_h = (l >> 4) as u16;
            if ib % 2 == 0 {
                out.scales_l[ib / 2] = l_l;
            } else {
                out.scales_l[ib / 2] |= l_l << 4;
            }
            out.scales_h |= l_h << (2 * (ib % 8));
        }
    } else {
        out.dh = f16_bits(scales[0]);
        if ntry > 0 {
            let id = if scales[0] != 0.0 {
                1.0 / scales[0]
            } else {
                0.0
            };
            for j in 0..super_size {
                ls[j] = best_index_int8(values, id * x[j]) as u8;
            }
        }
    }

    for i in 0..super_size / 32 {
        for j in 0..16 {
            out.q4[16 * i + j] = ls[32 * i + j] | (ls[32 * i + 16 + j] << 4);
        }
    }
    out
}

/// Port of quantize_iq4_nl for one 32 block (ntry = 7).
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Encoded {
    let r = quantize_impl(32, x, qw, 7);
    let mut bytes = vec![0u8; TYPE.bytes];
    put_u16(&mut bytes, 0, r.dh);
    bytes[2..18].copy_from_slice(&r.q4);
    Encoded {
        bytes,
        probe: vec![],
        note: None,
    }
}

/// Position of weight `j` (0..32) inside the packed nibbles of its 32 block.
pub fn nibble(qs: &[u8], j: usize) -> usize {
    if j < 16 {
        (qs[j] & 0xf) as usize
    } else {
        (qs[j - 16] >> 4) as usize
    }
}

/// Port of dequantize_row_iq4_nl.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(get_u16(b, 0));
    let qs = &b[2..18];
    for j in 0..16 {
        y[j] = d * lut::KVALUES_IQ4NL[(qs[j] & 0xf) as usize] as f32;
        y[j + 16] = d * lut::KVALUES_IQ4NL[(qs[j] >> 4) as usize] as f32;
    }
}

pub fn sub_scales(b: &[u8]) -> Vec<f32> {
    vec![f16_val(get_u16(b, 0))]
}

pub fn decode(b: &[u8], j: usize) -> IqDecode {
    let d = f16_val(get_u16(b, 0));
    let q = nibble(&b[2..18], j);
    IqDecode {
        d,
        sub_field: 0,
        sub_bits: 0,
        dl: d,
        dl_expr: "d".into(),
        math: format!("{:.5} &times; {}", d, lut::KVALUES_IQ4NL[q]),
        codebook: Codebook::NonLinear {
            idx: q,
            values: &lut::KVALUES_IQ4NL,
        },
        signs: Signs::None,
        delta: None,
    }
}

pub fn formula_html() -> String {
    formula(
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">kvalues[q]<span class=\"bits\">4b</span></span>",
        "16-level non-linear table, one FP16 scale per 32.",
        "Fixed 16-level table, dense near zero and sparse in the tails. Every 32 weights share one FP16 scale, searched over 15 candidates around max/127. No super-block. 4.5 bpw.",
    )
}
