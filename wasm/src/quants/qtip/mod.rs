//! QTIP (arXiv:2406.11235), ported from Cornell-RelaxML/qtip: a bitshift trellis over tiles of 256
//! weights decoded by the 1MAD, 3INST or HYB code, with the per tensor scale of finetune.py.
//! BlockLDLQ is left out, it needs a full Hessian and does nothing with a diagonal one.

pub mod codes;
#[rustfmt::skip]
pub mod lut;
pub mod trellis;
mod view;

use crate::math_utils::*;
use crate::quants::iq::algos::formula;
use crate::quants::{ImportanceResult, InspectorData, QuantMeta, QuantizeOutput, Settings};
use codes::Code;
use half::f16;
use trellis::Codebook;

/// Weights per trellis, one 16x16 tile in the paper.
pub const T: usize = 256;

pub struct QtipMeta {
    /// T * K bits per tile, MSB first.
    pub stream: Vec<u8>,
    pub wscale: f32,
    /// MSE and MAE per tile over the real weights.
    pub err: Vec<(f64, f64)>,
}

pub struct Params {
    pub code: Code,
    pub l: u32,
    pub k: u32,
    pub scale_override: f32,
}

impl Params {
    pub fn from_settings(s: &Settings) -> Params {
        let code = Code::from_name(&s.qtip_code);
        let k = s.qtip_bits.clamp(1, 4);
        let kv = k * code.v() as u32;
        Params {
            code,
            l: s.qtip_l.clamp(kv.max(8), 16),
            k,
            scale_override: if s.qtip_scale > 0.0 {
                s.qtip_scale
            } else {
                0.9
            },
        }
    }
}

pub fn tile_bytes(cb: &Codebook) -> usize {
    T * cb.k as usize / 8
}

/// What bitshift_codebook.quantize gets: W / Wscale in FP16, zero padded to whole tiles.
pub fn scaled_input(floats: &[f32], wscale: f32) -> Vec<f32> {
    let npad = ((floats.len() + T - 1) / T).max(1) * T;
    let mut x = vec![0.0f32; npad];
    if wscale > 0.0 {
        for (o, &v) in x.iter_mut().zip(floats) {
            *o = f16::from_f32(v / wscale).to_f32();
        }
    }
    x
}

/// Wscale from finetune.py: RMS of the weights over (RMS of the code * scale_override).
pub fn wscale(cb: &Codebook, floats: &[f32], scale_override: f32) -> f32 {
    if floats.is_empty() {
        return 0.0;
    }
    let sq: f64 = floats.iter().map(|&v| v as f64 * v as f64).sum();
    let rms = (sq / floats.len() as f64).sqrt() as f32;
    rms / (cb.rms * scale_override)
}

/// Quantizes the data tile by tile, returns the stream and the dequantized values (padded).
pub fn quantize_data(
    cb: &Codebook,
    floats: &[f32],
    scale_override: f32,
) -> (Vec<u8>, f32, Vec<f32>) {
    let ws = wscale(cb, floats, scale_override);
    let x = scaled_input(floats, ws);
    let steps = T / cb.v;
    let mut stream = Vec::with_capacity(x.len() / T * tile_bytes(cb));
    let mut y = vec![0.0f32; x.len()];
    for (tile, out) in x.chunks(T).zip(y.chunks_mut(T)) {
        let states = trellis::quantize_seq(cb, tile);
        stream.extend(trellis::pack(cb, &states));
        for i in 0..steps {
            for v in 0..cb.v {
                out[i * cb.v + v] = cb.value(states[i], v) * ws;
            }
        }
    }
    (stream, ws, y)
}

pub fn quantize(
    floats: &[f32],
    _importance: Option<&ImportanceResult>,
    settings: &Settings,
) -> QuantizeOutput {
    let p = Params::from_settings(settings);
    let cb = trellis::codebook(p.code, p.l, p.k);
    let n = floats.len();
    let (stream, wscale, y) = quantize_data(&cb, floats, p.scale_override);
    let err = floats
        .chunks(T)
        .zip(y.chunks(T))
        .map(|(a, b)| get_err_stats(a, &b[..a.len()]))
        .collect();
    let padded = y.len();
    QuantizeOutput {
        q_floats: y[..n].to_vec(),
        bpw: (padded as f32 * p.k as f32 + 32.0) / n.max(1) as f32,
        formula_html: formula_html(&cb, p.scale_override),
        block_size: T,
        super_block_size: T,
        meta: QuantMeta::Qtip(QtipMeta {
            stream,
            wscale,
            err,
        }),
    }
}

fn formula_html(cb: &Codebook, scale_override: f32) -> String {
    let code = match cb.code {
        Code::OneMad => "1MAD: x = 34038481&middot;s + 76625530 mod 2^32, value = (sum of the 4 bytes of x &minus; 510) / 147.8.",
        Code::ThreeInst => "3INST: x = 89226354&middot;s + 64248484 mod 2^32, each 16-bit half keeps its sign, 2 exponent and 10 mantissa bits, is xored into FP16 0.922 and the two halves are added.",
        Code::Hyb => "HYB: x = s&sup2; + s, bits 6..14 pick one of 512 FP16 pairs (k-means of a 2D Gaussian), bit 15 flips the sign of the first value. Two weights per state.",
    };
    let pills = format!(
        "<span class=\"eq-pill\">d<span class=\"bits\">32b</span></span> &times; <span class=\"eq-pill\">{}(s)<span class=\"bits\">{}b window</span></span>",
        cb.code.name(),
        cb.l
    );
    formula(
        &pills,
        &if cb.v == 1 {
            format!(
                "Weights read {}-bit windows of one bitstream, {} new bits each. Viterbi picks the stream.",
                cb.l, cb.k
            )
        } else {
            format!(
                "Weight pairs read {}-bit windows of one bitstream, {} new bits each. Viterbi picks the stream.",
                cb.l,
                cb.kv()
            )
        },
        &format!(
            "Trellis with 2^{} states over tiles of 256 weights. Every step shifts {} new bits into the state, so neighbouring weights share most of their bits and the search has to be done jointly (Viterbi). Tail-biting: the stream wraps around, a tile costs exactly 256&middot;{} bits. Viterbi runs twice, first on the tile rotated by half to find the bits at the wrap. {} One FP32 scale, RMS of the weights over {} &times; RMS of the code. LDLQ is not done, so importance is not used.",
            cb.l,
            cb.kv(),
            cb.k,
            code,
            scale_override
        ),
    )
}

pub fn format_inspector(
    idx: usize,
    active: &[f32],
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::Qtip(meta) = &out.meta else {
        return InspectorData::default();
    };
    let p = Params::from_settings(settings);
    let cb = trellis::codebook(p.code, p.l, p.k);
    let mut data = InspectorData::default();
    let tile = idx / T;
    if tile >= meta.err.len() {
        return data;
    }
    let tb = tile_bytes(&cb);
    let stream = &meta.stream[tile * tb..(tile + 1) * tb];
    let j = idx % T;
    let steps = T / cb.v;
    let step = j / cb.v;
    let state = trellis::state_at(&cb, stream, steps, step);
    let value = cb.value(state, j % cb.v);
    let (mse, mae) = meta.err[tile];

    data.math_str = Some(format!(
        "{:.5} &times; {}",
        meta.wscale,
        view::signed(value, 4)
    ));
    data.block_idx = Some(tile);
    data.super_idx = Some(tile);
    data.mse = Some(mse);
    data.mae = Some(mae);
    data.scale = Some(meta.wscale);
    data.scale_label = Some("Scale:".into());
    let target = f16::from_f32(if meta.wscale > 0.0 {
        active[idx] / meta.wscale
    } else {
        0.0
    })
    .to_f32();
    data.detail_html = Some(view::render(
        &cb,
        stream,
        tile,
        j,
        target,
        meta.wscale,
        out.q_floats[idx],
        settings.rotation_type != "none",
    ));
    data
}
