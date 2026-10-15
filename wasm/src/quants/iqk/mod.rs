//! IQ*_K quants from ik_llama.cpp (iqk_quantize.cpp): IQ2_K .. IQ6_K and the row-scaled _KS
//! variants. All of them are scalar non-linear tables where every block of 16 or 32 weights can
//! switch to a copy of the table shifted by a small amount. Ported from the AVX2 code paths
//! llama-quantize runs on x86. The data is quantized as one row, zero padded to 256.

pub mod common;
pub mod iq2_k;
pub mod iq2_kl;
pub mod iq2_ks;
pub mod iq3_k;
pub mod iq3_ks;
pub mod iq4_k;
pub mod iq4_ks;
pub mod iq4_kss;
pub mod iq5_k;
pub mod iq5_ks;
pub mod iq6_k;
#[rustfmt::skip]
pub mod lut;
mod view;

use common::QK_K;

pub use crate::quants::iq::algos::formula;

use crate::math_utils::*;
use crate::quants::{ImportanceResult, InspectorData, QuantMeta, QuantizeOutput, Settings};

/// Static description of one IQ*_K format.
pub struct KType {
    pub name: &'static str,
    /// Bytes per 256 weights.
    pub block_bytes: usize,
    /// Row scale in front of the blocks: 0, 2 (FP16) or 4 (FP32) bytes.
    pub row_meta: usize,
    /// Weights per block scale.
    pub sub_size: usize,
    /// Block size QHelper cleans the imatrix with.
    pub imatrix_block: usize,
    pub quantize: fn(&[f32], Option<&[f32]>) -> Vec<u8>,
    pub dequantize: fn(&[u8], &mut [f32]),
    pub decode: fn(&[u8], usize) -> KDecode,
    /// (block scale, shifted table) of every block in super-block ibl.
    pub block_scales: fn(&[u8], usize) -> Vec<(f32, bool)>,
    pub formula_html: fn() -> String,
}

impl KType {
    pub fn bpw(&self, n: usize) -> f32 {
        (self.block_bytes * 8) as f32 / QK_K as f32 + (self.row_meta * 8) as f32 / n.max(1) as f32
    }
}

/// How one weight decodes, for the inspector.
pub struct KDecode {
    pub d: f32,
    /// d is the row scale, not one per 256.
    pub row: bool,
    pub field: i32,
    pub field_bits: u32,
    pub dl: f32,
    /// Block scale as a function of the stored field, "{s}" is replaced by it.
    pub dl_expr: &'static str,
    pub shifted: bool,
    pub idx: usize,
    pub idx_bits: u32,
    pub value: f32,
    pub base: Vec<f32>,
    pub up: Vec<f32>,
    /// Level of `base`/`up` to highlight, the index itself except for IQ2_KL.
    pub mark: usize,
    /// IQ2_KL: both values of the pair and which one this weight is.
    pub pair: Option<(f32, f32, usize)>,
    pub math: String,
}

impl KDecode {
    pub fn new(
        d: f32,
        row: bool,
        field: i32,
        field_bits: u32,
        dl: f32,
        dl_expr: &'static str,
        shifted: bool,
    ) -> KDecode {
        KDecode {
            d,
            row,
            field,
            field_bits,
            dl,
            dl_expr,
            shifted,
            idx: 0,
            idx_bits: 0,
            value: 0.0,
            base: Vec::new(),
            up: Vec::new(),
            mark: 0,
            pair: None,
            math: String::new(),
        }
    }

    /// Index into a table of `n` levels followed by its shifted copy.
    pub fn level(self, idx: usize, bits: u32, value: f32, table: &[i8], n: usize) -> KDecode {
        let base = table[..n].iter().map(|&v| v as f32).collect();
        let up = if table.len() >= 2 * n {
            table[n..2 * n].iter().map(|&v| v as f32).collect()
        } else {
            Vec::new()
        };
        self.levels(idx, bits, value, base, up)
    }

    pub fn levels(
        mut self,
        idx: usize,
        bits: u32,
        value: f32,
        base: Vec<f32>,
        up: Vec<f32>,
    ) -> KDecode {
        self.idx = idx;
        self.idx_bits = bits;
        self.value = value;
        self.mark = idx;
        self.base = base;
        self.up = up;
        self
    }

    pub fn math(mut self, math: String) -> KDecode {
        self.math = math;
        self
    }
}

pub struct IqkMeta {
    pub bytes: Vec<u8>,
    /// MSE and MAE per super-block over the real weights.
    pub err: Vec<(f64, f64)>,
}

pub fn k_type(name: &str) -> &'static KType {
    match name {
        "iq2_k" => &iq2_k::TYPE,
        "iq2_ks" => &iq2_ks::TYPE,
        "iq2_kl" => &iq2_kl::TYPE,
        "iq3_k" => &iq3_k::TYPE,
        "iq3_ks" => &iq3_ks::TYPE,
        "iq4_ks" => &iq4_ks::TYPE,
        "iq4_kss" => &iq4_kss::TYPE,
        "iq5_k" => &iq5_k::TYPE,
        "iq5_ks" => &iq5_ks::TYPE,
        "iq6_k" => &iq6_k::TYPE,
        _ => &iq4_k::TYPE,
    }
}

/// Quantizes the data as one row and returns the bytes with the dequantized values (padded).
pub fn quantize_row(t: &KType, floats: &[f32], qw: Option<&[f32]>) -> (Vec<u8>, Vec<f32>) {
    let n = floats.len();
    let npad = ((n + QK_K - 1) / QK_K * QK_K).max(QK_K);
    let mut x = vec![0.0f32; npad];
    x[..n].copy_from_slice(floats);
    let w = qw.map(|qw| {
        let mut w = vec![0.0f32; npad];
        w[..n].copy_from_slice(&qw[..n]);
        common::row_weights(&w, &x, t.imatrix_block)
    });
    let bytes = (t.quantize)(&x, w.as_deref());
    let mut y = vec![0.0f32; npad];
    (t.dequantize)(&bytes, &mut y);
    (bytes, y)
}

pub fn quantize(
    floats: &[f32],
    importance: Option<&ImportanceResult>,
    settings: &Settings,
) -> QuantizeOutput {
    let t = k_type(&settings.iqk_type);
    let n = floats.len();
    let (bytes, y) = quantize_row(t, floats, importance.map(|imp| imp.raw));
    let err = floats
        .chunks(QK_K)
        .zip(y.chunks(QK_K))
        .map(|(a, b)| get_err_stats(a, &b[..a.len()]))
        .collect();
    QuantizeOutput {
        q_floats: y[..n].to_vec(),
        bpw: t.bpw(n),
        formula_html: (t.formula_html)(),
        block_size: t.sub_size,
        super_block_size: QK_K,
        meta: QuantMeta::Iqk(IqkMeta { bytes, err }),
    }
}

pub fn format_inspector(
    idx: usize,
    active: &[f32],
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::Iqk(meta) = &out.meta else {
        return InspectorData::default();
    };
    let t = k_type(&settings.iqk_type);
    let mut data = InspectorData::default();
    let ibl = idx / QK_K;
    if ibl >= meta.err.len() {
        return data;
    }
    let dec = (t.decode)(&meta.bytes, idx);
    let sub = idx / t.sub_size * t.sub_size;
    let sub_end = (sub + t.sub_size).min(active.len());
    let (mse, mae) = get_err_stats(&active[sub..sub_end], &out.q_floats[sub..sub_end]);

    data.math_str = Some(dec.math.clone());
    data.block_idx = Some(idx / t.sub_size);
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
        &out.q_floats[base..end],
        settings.rotation_type != "none",
    ));
    data
}
