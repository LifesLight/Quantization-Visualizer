pub mod asym;
pub mod kquant;
pub mod mxfp;
pub mod nvfp4;
pub mod primitive;
pub mod sym;
pub mod trellis;
pub mod turbo;

use serde::{Deserialize, Serialize};

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
    pub mxfp_format: String,
    pub primitive_format: String,
    pub centering_mode: String,
}

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

pub enum QuantMeta {
    Primitive,
    Sym(Vec<SymBlockMeta>),
    Asym(Vec<AsymBlockMeta>),
    KQuant(Vec<KBlockMeta>, Vec<KSuperMeta>),
    Nvfp4(Vec<Nvfp4BlockMeta>, f32, f64, f64),
    Mxfp(Vec<MxfpBlockMeta>),
    Turbo(Vec<TurboBlockMeta>),
    Trellis(Vec<TrellisBlockMeta>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorData {
    pub math_str: String,
    pub block_html: String,
    pub block_idx_str: String,
    pub super_html: String,
    pub super_idx_str: String,
}

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
