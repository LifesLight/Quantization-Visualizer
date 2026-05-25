//! Contains all implemented quantization schemes and shared types.

pub mod asym;
pub mod iq;
pub mod kquant;
pub mod mxfp;
pub mod nvfp4;
pub mod primitive;
pub mod sym;
pub mod trellis;
pub mod turbo;

use serde::{Deserialize, Serialize};

pub struct ImportanceResult<'a> {
    pub raw: &'a [f32],
    pub intensity: &'a [f32],
    pub block_stats: &'a [ImpBlockStat],
    pub block_size: usize,
}

#[derive(Clone, Copy)]
pub struct ImpBlockStat {
    pub sum: f32,
    pub max: f32,
}

/// Identical functional signatures enforced across all quantizer algorithms.
pub type QuantizeFn = fn(&[f32], Option<&ImportanceResult>, &Settings) -> QuantizeOutput;
pub type FormatInspectorFn = fn(usize, &[f32], &QuantizeOutput, &Settings) -> InspectorData;

/// Global quantization settings payload passed directly from the JavaScript frontend.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub q_type: String,
    pub weight_bits: u32,
    pub block_size: usize,
    pub sb_size: usize,
    pub sub_size: usize,
    pub sub_bits: u32,
    pub has_offset: bool,
    pub turbo_bits: u32,
    pub turbo_block_size: usize,
    pub use_wht: bool,
    pub use_qjl: bool,
    pub turbo_sign_seed: u32,
    pub trellis_bits: u32,
    pub trellis_block_size: usize,
    pub trellis_states: usize,
    pub trellis_cb_type: String,
    pub trellis_use_wht: bool,
    pub trellis_wht_scope: String,
    pub trellis_opt_iters: usize,
    pub trellis_sign_seed: u32,
    pub iq_type: String,
    pub iq_scale_iters: usize,
    pub mxfp_format: String,
    pub primitive_format: String,
    pub centering_mode: String,
    pub axis_ignore_outliers: bool,
    pub axis_outlier_pct: f64,
    pub axis_manual_min: f32,
    pub axis_manual_max: f32,
    pub use_importance: bool,
}

// Specify importance normalization for quants
pub fn get_importance_block_size(q_type: &str, settings: &Settings) -> usize {
    match q_type {
        "kquant" => settings.sub_size,
        "sym" | "asym" => settings.block_size,
        "iq" => {
            if settings.iq_type == "iq4_nl" {
                32
            } else {
                256
            }
        }
        _ => 0,
    }
}

/// The standardized output containing the quantized data arrays and computed stats.
pub struct QuantizeOutput {
    pub q_floats: Vec<f32>,
    pub t_floats: Option<Vec<f32>>,
    pub t_q_floats: Option<Vec<f32>>,
    pub bpw: f32,
    pub formula_html: String,
    pub block_size: usize,
    pub super_block_size: usize,
    pub meta: QuantMeta,
}

/// Holds all possible underlying metadata implementations for specific quantization schemes.
pub enum QuantMeta {
    Primitive,
    Sym(Vec<SymBlockMeta>),
    Asym(Vec<AsymBlockMeta>),
    KQuant(Vec<KBlockMeta>, Vec<KSuperMeta>),
    Nvfp4(Vec<Nvfp4BlockMeta>, f32, f64, f64), // Nvfp4BlockMeta, global_scale, gmse, gmae
    Mxfp(Vec<MxfpBlockMeta>),
    Turbo(Vec<TurboBlockMeta>),
    Trellis(Vec<TrellisBlockMeta>),
    Iq(Vec<IqBlockMeta>),
}

/// Individual block inspection data retrieved via UI interactions. Serialized to JS Object.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InspectorData {
    pub math_str: Option<String>,
    pub block_idx: Option<usize>,
    pub super_idx: Option<usize>,
    pub mse: Option<f64>,
    pub mae: Option<f64>,
    pub scale: Option<f32>,
    pub min: Option<f32>,
    pub scale_e: Option<i32>,
    pub qjl_scale: Option<f32>,
    pub super_scale: Option<f32>,
    pub super_min: Option<f32>,
    pub super_mse: Option<f64>,
    pub super_mae: Option<f64>,
    pub global_scale: Option<f32>,
    pub global_mse: Option<f64>,
    pub global_mae: Option<f64>,
    pub trellis_json: Option<String>,
    pub iq_html: Option<String>,
    pub importance_raw: Option<f32>,
    pub importance_pct_sum: Option<f32>,
    pub importance_pct_max: Option<f32>,
}

// Specific metadata block structs for various algorithms.

pub struct SymBlockMeta {
    pub idx: usize,
    pub size: usize,
    pub scale: f32,
    pub mse: f64,
    pub mae: f64,
}
pub struct AsymBlockMeta {
    pub idx: usize,
    pub size: usize,
    pub scale: f32,
    pub min: f32,
    pub mse: f64,
    pub mae: f64,
}
pub struct KBlockMeta {
    pub idx: usize,
    pub sb_idx: usize,
    pub size: usize,
    pub q_scale: f32,
    pub q_min: f32,
    pub mse: f64,
    pub mae: f64,
}
pub struct KSuperMeta {
    pub idx: usize,
    pub size: usize,
    pub super_scale: f32,
    pub super_min_scale: f32,
    pub mse: f64,
    pub mae: f64,
}
pub struct Nvfp4BlockMeta {
    pub idx: usize,
    pub size: usize,
    pub scale: f32,
    pub mse: f64,
    pub mae: f64,
}
pub struct MxfpBlockMeta {
    pub idx: usize,
    pub size: usize,
    pub scale_e: i32,
    pub scale: f32,
    pub mse: f64,
    pub mae: f64,
}
pub struct TurboBlockMeta {
    pub idx: usize,
    pub size: usize,
    pub scale: f32,
    pub qjl_scale: f32,
    pub mse: f64,
    pub mae: f64,
}
pub struct TrellisBlockMeta {
    pub idx: usize,
    pub size: usize,
    pub scale: f32,
    pub mse: f64,
    pub mae: f64,
    pub chunk_w: Vec<f64>,
}
pub struct IqBlockMeta {
    pub idx: usize,
    pub size: usize,
    pub block_scale: f32,
    pub mse: f64,
    pub mae: f64,
    pub scales: Vec<f32>,
    pub aux8: Vec<i8>,
    pub grids: Vec<usize>,
    pub signs: Vec<u8>,
}
