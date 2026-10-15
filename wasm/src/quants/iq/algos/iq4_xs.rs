//! IQ4_XS: block_iq4_xs { fp16 d; u16 scales_h; u8 scales_l[4]; u8 qs[128] } -> 136 bytes per 256.
//! IQ4_NL levels with a 6 bit signed scale per 32 weights (low 4 bits in scales_l, high 2 in scales_h).

use super::iq4_nl::{nibble, quantize_impl};
use super::*;
use crate::quants::iq::lut;

pub static TYPE: IqType = IqType {
    name: "IQ4_XS",
    super_size: QK_K,
    sub_size: 32,
    group_size: 1,
    bytes: 136,
    needs_imatrix: false,
    quantize,
    dequantize,
    decode,
    sub_scales,
    formula_html,
};

const SCALES_H: usize = 2;
const SCALES_L: usize = 4;
const QS: usize = 8;

/// Port of quantize_iq4_xs for one super-block (ntry = 7).
pub fn quantize(x: &[f32], qw: Option<&[f32]>) -> Encoded {
    let r = quantize_impl(QK_K, x, qw, 7);
    let mut bytes = vec![0u8; TYPE.bytes];
    put_u16(&mut bytes, 0, r.dh);
    put_u16(&mut bytes, SCALES_H, r.scales_h);
    bytes[SCALES_L..QS].copy_from_slice(&r.scales_l);
    bytes[QS..].copy_from_slice(&r.q4);
    Encoded {
        bytes,
        probe: vec![],
        note: None,
    }
}

fn ls(b: &[u8], ib: usize) -> i32 {
    let lo = (b[SCALES_L + ib / 2] >> (4 * (ib % 2))) & 0xf;
    let hi = (get_u16(b, SCALES_H) >> (2 * ib)) & 3;
    lo as i32 | ((hi as i32) << 4)
}

/// Port of dequantize_row_iq4_xs.
pub fn dequantize(b: &[u8], y: &mut [f32]) {
    let d = f16_val(get_u16(b, 0));
    for ib in 0..QK_K / 32 {
        let dl = d * (ls(b, ib) - 32) as f32;
        let qs = &b[QS + 16 * ib..QS + 16 * ib + 16];
        for j in 0..16 {
            y[32 * ib + j] = dl * lut::KVALUES_IQ4NL[(qs[j] & 0xf) as usize] as f32;
            y[32 * ib + j + 16] = dl * lut::KVALUES_IQ4NL[(qs[j] >> 4) as usize] as f32;
        }
    }
}

pub fn sub_scales(b: &[u8]) -> Vec<f32> {
    let d = f16_val(get_u16(b, 0));
    (0..QK_K / 32)
        .map(|ib| d * (ls(b, ib) - 32) as f32)
        .collect()
}

pub fn decode(b: &[u8], j: usize) -> IqDecode {
    let ib = j / 32;
    let d = f16_val(get_u16(b, 0));
    let l = ls(b, ib);
    let q = nibble(&b[QS + 16 * ib..QS + 16 * ib + 16], j % 32);
    IqDecode {
        d,
        sub_field: l as u32,
        sub_bits: 6,
        dl: d * (l - 32) as f32,
        dl_expr: "d&middot;({s}&minus;32)".into(),
        math: format!(
            "{:.5}&middot;({}&minus;32) &times; {}",
            d,
            l,
            lut::KVALUES_IQ4NL[q]
        ),
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
        "<span class=\"eq-pill\">d<span class=\"bits\">16b</span></span> &times; (<span class=\"eq-pill\">ls<span class=\"bits\">6b</span></span> &minus; 32) &times; <span class=\"eq-pill\">kvalues[q]<span class=\"bits\">4b</span></span>",
        "16-level non-linear table, 6-bit scale per 32, one FP16 scale per 256.",
        "Fixed 16-level table, dense near zero and sparse in the tails. Every 32 weights share one signed 6-bit scale (4 bits in scales_l, 2 in scales_h), every 256 share one FP16 scale. 4.25 bpw.",
    )
}
