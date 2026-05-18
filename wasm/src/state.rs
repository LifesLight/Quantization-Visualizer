use crate::math_utils::get_err_stats;
use crate::quants::*;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
pub struct QuantStats {
    pub bpw: f32,
    pub formula_html: String,
    pub block_size: usize,
    pub super_block_size: usize,
    pub has_srht: bool,
    pub global_mse: f64,
    pub global_mae: f64,
    pub global_variance: f64,
    pub global_sum_abs: f64,
}

#[wasm_bindgen]
pub struct AppBackend {
    raw_floats: Vec<f32>,
    base_floats: Vec<f32>,
    active_floats: Vec<f32>,
    last_output: Option<QuantizeOutput>,
    clip_start: usize,
    clip_end: usize,
}

#[wasm_bindgen]
impl AppBackend {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            raw_floats: vec![],
            base_floats: vec![],
            active_floats: vec![],
            last_output: None,
            clip_start: 0,
            clip_end: 0,
        }
    }

    pub fn parse_floats(&mut self, text: &str) -> usize {
        self.raw_floats = text
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<f32>().ok())
            .collect();
        self.raw_floats.len()
    }

    pub fn set_scale_offset(&mut self, scale: f32, offset: f32) {
        self.base_floats = self
            .raw_floats
            .iter()
            .map(|&v| v * scale + offset)
            .collect();
    }

    pub fn set_clip(&mut self, start: usize, end: usize) {
        let n = self.base_floats.len();
        self.clip_start = start.min(n.saturating_sub(1));
        self.clip_end = end.min(n.saturating_sub(1)).max(self.clip_start);
        self.active_floats = self.base_floats[self.clip_start..=self.clip_end].to_vec();
    }

    pub fn quantize(&mut self, settings_js: JsValue) -> JsValue {
        let settings: Settings = serde_wasm_bindgen::from_value(settings_js).unwrap();
        let output = match settings.q_type.as_str() {
            "primitive" => primitive::quantize(&self.active_floats, &settings),
            "sym" => sym::quantize(&self.active_floats, &settings),
            "asym" => asym::quantize(&self.active_floats, &settings),
            "kquant" => kquant::quantize(&self.active_floats, &settings),
            "nvfp4" => nvfp4::quantize(&self.active_floats, &settings),
            "mxfp" => mxfp::quantize(&self.active_floats, &settings),
            "turbo" => turbo::quantize(&self.active_floats, &settings),
            "trellis" => trellis::quantize(&self.active_floats, &settings),
            _ => primitive::quantize(&self.active_floats, &settings),
        };

        let (global_mse, global_mae) = get_err_stats(&self.active_floats, &output.q_floats);
        let mut sig_power = 0.0_f64;
        let mut sum_abs = 0.0_f64;
        for &v in &self.active_floats {
            let vf = v as f64;
            sig_power += vf * vf;
            sum_abs += vf.abs();
        }
        let global_variance = if self.active_floats.is_empty() {
            0.0
        } else {
            sig_power / self.active_floats.len() as f64
        };

        let stats = QuantStats {
            bpw: output.bpw,
            formula_html: output.formula_html.clone(),
            block_size: output.block_size,
            super_block_size: output.super_block_size,
            has_srht: output.t_floats.is_some(),
            global_mse,
            global_mae,
            global_variance,
            global_sum_abs: sum_abs,
        };

        self.last_output = Some(output);
        serde_wasm_bindgen::to_value(&stats).unwrap()
    }

    pub fn get_base_floats_ptr(&self) -> *const f32 {
        self.base_floats.as_ptr()
    }
    pub fn get_base_floats_len(&self) -> usize {
        self.base_floats.len()
    }
    pub fn get_active_floats_ptr(&self) -> *const f32 {
        self.active_floats.as_ptr()
    }
    pub fn get_active_floats_len(&self) -> usize {
        self.active_floats.len()
    }
    pub fn get_q_floats_ptr(&self) -> *const f32 {
        self.last_output
            .as_ref()
            .map(|o| o.q_floats.as_ptr())
            .unwrap_or(std::ptr::null())
    }
    pub fn get_t_floats_ptr(&self) -> *const f32 {
        self.last_output
            .as_ref()
            .and_then(|o| o.t_floats.as_ref())
            .map(|v| v.as_ptr())
            .unwrap_or(std::ptr::null())
    }
    pub fn get_t_q_floats_ptr(&self) -> *const f32 {
        self.last_output
            .as_ref()
            .and_then(|o| o.t_q_floats.as_ref())
            .map(|v| v.as_ptr())
            .unwrap_or(std::ptr::null())
    }

    pub fn get_inspector_data(&self, idx: usize, settings_js: JsValue) -> JsValue {
        let settings: Settings = serde_wasm_bindgen::from_value(settings_js).unwrap();
        if let Some(out) = &self.last_output {
            let data = match &out.meta {
                QuantMeta::Primitive => primitive::format_inspector(idx, out, &settings),
                QuantMeta::Sym(m) => sym::format_inspector(idx, out, m, &settings),
                QuantMeta::Asym(m) => asym::format_inspector(idx, out, m, &settings),
                QuantMeta::KQuant(b, s) => {
                    kquant::format_inspector(idx, &self.active_floats, out, b, s, &settings)
                }
                QuantMeta::Nvfp4(b, gs, mse, mae) => nvfp4::format_inspector(
                    idx,
                    &self.active_floats,
                    out,
                    b,
                    *gs,
                    *mse,
                    *mae,
                    &settings,
                ),
                QuantMeta::Mxfp(b) => {
                    mxfp::format_inspector(idx, &self.active_floats, out, b, &settings)
                }
                QuantMeta::Turbo(b) => {
                    turbo::format_inspector(idx, &self.active_floats, out, b, &settings)
                }
                QuantMeta::Trellis(b) => {
                    trellis::format_inspector(idx, &self.active_floats, out, b, &settings)
                }
            };
            serde_wasm_bindgen::to_value(&data).unwrap()
        } else {
            JsValue::NULL
        }
    }
}
