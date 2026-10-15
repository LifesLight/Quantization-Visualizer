//! Trellis (KT) quants from ik_llama.cpp: IQ1_KT, IQ2_KT, IQ3_KT, IQ4_KT.
//! Groups of 4 or 8 weights store an index into a computed codebook (see codebook.rs),
//! blocks of 32 a small scale, and the whole row one f32 scale. Every type quantizes into its
//! real block layout, so what's shown is what iqk_quantize.cpp would store.

pub mod codebook;
pub mod common;
pub mod iq1_kt;
pub mod iq2_kt;
pub mod iq3_kt;
pub mod iq4_kt;
mod view;

use common::{KtDecode, KtRow, QK_K};

pub use crate::quants::iq::algos::formula;

use crate::math_utils::*;
use crate::quants::{ImportanceResult, InspectorData, QuantMeta, QuantizeOutput, Settings};

/// Static description of one KT format.
pub struct KtType {
    pub name: &'static str,
    /// Weights sharing one codebook index.
    pub group_size: usize,
    /// Bytes per 256 weights, without the f32 row scale.
    pub bytes: usize,
    /// IQ2_KT fills a partial super-block with copies of the end of the row, the others with zeros.
    pub copy_pad: bool,
    pub quantize: fn(&[f32], usize, Option<&[f32]>) -> KtRow,
    pub dequantize: fn(&KtRow, &mut [f32]),
    pub decode: fn(&KtRow, usize) -> KtDecode,
    pub block_scales: fn(&KtRow, usize) -> Vec<f32>,
    pub formula_html: fn() -> String,
}

impl KtType {
    pub fn bpw(&self, n: usize) -> f32 {
        (self.bytes * 8) as f32 / QK_K as f32 + 32.0 / n.max(1) as f32
    }
}

pub struct TrellisMeta {
    pub row: KtRow,
    /// MSE and MAE per super-block over the real weights.
    pub err: Vec<(f64, f64)>,
}

fn kt_type(name: &str) -> &'static KtType {
    match name {
        "iq1_kt" => &iq1_kt::TYPE,
        "iq3_kt" => &iq3_kt::TYPE,
        "iq4_kt" => &iq4_kt::TYPE,
        _ => &iq2_kt::TYPE,
    }
}

pub fn quantize(
    floats: &[f32],
    importance: Option<&ImportanceResult>,
    settings: &Settings,
) -> QuantizeOutput {
    let t = kt_type(settings.trellis_type.as_str());
    let n = floats.len();
    let (row, y) = quantize_row(t, floats, importance.map(|imp| &imp.raw[..n]));

    let mut err = Vec::new();
    for b in (0..n).step_by(QK_K) {
        let e = (b + QK_K).min(n);
        err.push(get_err_stats(&floats[b..e], &y[b..e]));
    }

    QuantizeOutput {
        q_floats: y[..n].to_vec(),
        t_floats: None,
        t_q_floats: None,
        bpw: t.bpw(n),
        formula_html: (t.formula_html)(),
        block_size: 32,
        super_block_size: QK_K,
        meta: QuantMeta::Trellis(TrellisMeta { row, err }),
    }
}

/// Quantizes the data as one row and returns it with its dequantized values (padded length).
/// Upstream rows are a multiple of 32, a shorter tail is zero padded here, then the last
/// super-block is filled the way iqk_quantize.cpp does it.
pub fn quantize_row(t: &KtType, floats: &[f32], qw: Option<&[f32]>) -> (KtRow, Vec<f32>) {
    let n = floats.len();
    let n32 = (n + 31) / 32 * 32;
    let npad = ((n32 + QK_K - 1) / QK_K * QK_K).max(QK_K);
    let mut x = vec![0.0f32; npad];
    x[..n].copy_from_slice(floats);
    let mut w = qw.map(|qw| {
        let mut w = vec![0.0f32; npad];
        w[..n].copy_from_slice(qw);
        w
    });
    let mut n_row = n32.max(32);
    if t.copy_pad && npad != n32 && n32 > 0 {
        let n_extra = npad - n32;
        let fill = |v: &mut Vec<f32>| {
            if n32 >= n_extra {
                v.copy_within(n32 - n_extra..n32, n32);
            } else {
                for ib in 0..n_extra / 32 {
                    v.copy_within(n32 - 32..n32, n32 + 32 * ib);
                }
            }
        };
        fill(&mut x);
        if let Some(w) = w.as_mut() {
            fill(w);
        }
        n_row = npad;
    }

    let row = (t.quantize)(&x, n_row, w.as_deref());
    let mut y = vec![0.0f32; npad];
    (t.dequantize)(&row, &mut y);
    (row, y)
}

pub fn format_inspector(
    idx: usize,
    active: &[f32],
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::Trellis(meta) = &out.meta else {
        return InspectorData::default();
    };
    let t = kt_type(settings.trellis_type.as_str());
    let mut data = InspectorData::default();
    let ibl = idx / QK_K;
    if ibl >= meta.err.len() {
        return data;
    }
    let dec = (t.decode)(&meta.row, idx);

    let sub_start = idx / 32 * 32;
    let sub_end = (sub_start + 32).min(active.len());
    let (mse, mae) = get_err_stats(
        &active[sub_start..sub_end],
        &out.q_floats[sub_start..sub_end],
    );

    data.math_str = Some(dec.math.clone());
    data.block_idx = Some(idx / 32);
    data.super_idx = Some(ibl);
    data.mse = Some(mse);
    data.mae = Some(mae);
    data.scale = Some(dec.dl);
    data.scale_label = Some("Sub-scale:".into());
    let base = ibl * QK_K;
    let end = (base + QK_K).min(active.len());
    data.detail_html = Some(view::render(
        t,
        meta,
        &dec,
        idx,
        &active[base..end],
        &out.q_floats[base..end],
    ));
    data
}
