//! The QTIP codes, what a trellis state decodes to (decode_1mad, decode_3inst and quantlut_sym in
//! lib/codebook/bitshift.py).

use super::lut::HYB_TLUT;
use half::f16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Code {
    OneMad,
    ThreeInst,
    Hyb,
}

impl Code {
    pub fn from_name(s: &str) -> Code {
        match s {
            "1mad" => Code::OneMad,
            "3inst" => Code::ThreeInst,
            _ => Code::Hyb,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Code::OneMad => "1MAD",
            Code::ThreeInst => "3INST",
            Code::Hyb => "HYB",
        }
    }

    /// Weights per trellis step.
    pub fn v(self) -> usize {
        if self == Code::Hyb {
            2
        } else {
            1
        }
    }
}

pub const MAD_A: u32 = 34038481;
pub const MAD_B: u32 = 76625530;
pub const INST_A: u32 = 89226354;
pub const INST_B: u32 = 64248484;
/// Sign bit, bottom two exponent bits and the mantissa of each half.
pub const INST_MASK: u32 = 0x8fff_8fff;
/// The magic FP16 0.922 (0x3b60) in both halves.
pub const INST_M: u32 = 0x3b60_3b60;
/// Q, the HYB table has 2^Q entries.
pub const HYB_Q: u32 = 9;

pub fn mad_word(s: u32) -> u32 {
    s.wrapping_mul(MAD_A).wrapping_add(MAD_B)
}

/// LCG, the four bytes summed is about Gaussian, shifted and scaled to unit variance.
pub fn one_mad(s: u32) -> f32 {
    let x = mad_word(s);
    let y = (x & 255) + (x >> 8 & 255) + (x >> 16 & 255) + (x >> 24);
    (y as i32 - 510) as f32 / 147.800537109375
}

/// LCG, then each 16-bit half xored into the magic number and read as FP16 (top, bottom).
pub fn inst_halves(s: u32) -> (f16, f16) {
    let x = s.wrapping_mul(INST_A).wrapping_add(INST_B);
    let r = (x & INST_MASK) ^ INST_M;
    (f16::from_bits((r >> 16) as u16), f16::from_bits(r as u16))
}

/// The two halves summed in FP16.
pub fn three_inst(s: u32) -> f32 {
    let (a, b) = inst_halves(s);
    f16::from_f32(a.to_f32() + b.to_f32()).to_f32()
}

/// Hash s^2 + s, bits 6..14 pick the table entry, bit 15 flips the sign of its first value.
pub fn hyb_index(s: u32) -> (u64, usize, bool) {
    let x = (s as u64 + 1) * s as u64;
    let idx = (x >> (16 - HYB_Q - 1)) & ((1 << HYB_Q) - 1);
    (x, idx as usize, (x >> 15) & 1 == 1)
}

pub fn hyb_entry(idx: usize) -> [f32; 2] {
    let e = HYB_TLUT[idx];
    [f16::from_bits(e[0]).to_f32(), f16::from_bits(e[1]).to_f32()]
}

pub fn hyb(s: u32) -> [f32; 2] {
    let (_, idx, flip) = hyb_index(s);
    let [a, b] = hyb_entry(idx);
    [if flip { -a } else { a }, b]
}
