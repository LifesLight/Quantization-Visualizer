use crate::math_utils::get_err_stats;
use crate::quants::*;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Output statistics sent back to the frontend after quantization.
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
    pub max_error: f64,
    pub has_importance: bool,
    pub weighted_mse: f64,
    pub weighted_mae: f64,
    pub weighted_snr: f64,
    pub max_weighted_error: f64,
    pub imp_block_size: usize,
}

#[derive(Serialize, Clone)]
pub struct RangeStats {
    pub render_min: f32,
    pub render_max: f32,
}

#[derive(Serialize, Clone)]
pub struct MinMaxInfo {
    pub min_idx: usize,
    pub max_idx: usize,
    pub min_v: f32,
    pub max_v: f32,
}

/// A unique signature for a specific render payload, preventing identical render requests.
#[derive(PartialEq)]
struct RenderCacheKey {
    data_version: u64,
    zs: usize,
    ze: usize,
    width_bits: u64,
    effective_srht: bool,
    clip_start: usize,
    clip_end: usize,
    centering_mode: String,
    ignore_outliers: bool,
    outlier_pct_bits: u64,
    manual_min_bits: u32,
    manual_max_bits: u32,
}

/// The core application state object maintained between Javascript boundaries.
#[wasm_bindgen]
pub struct AppBackend {
    raw_floats: Vec<f32>,
    base_floats: Vec<f32>,
    active_floats: Vec<f32>,

    raw_importance: Vec<f32>,
    base_importance: Vec<f32>,
    active_importance: Vec<f32>,
    active_importance_intensity: Vec<f32>,
    imp_block_stats: Vec<ImpBlockStat>,
    imp_block_size: usize,

    last_output: Option<QuantizeOutput>,
    clip_start: usize,
    clip_end: usize,

    data_version: u64,
    minmax_version: u64,
    minmax_cache: Option<MinMaxInfo>,

    best_indices: Vec<i32>,
    best_key: Option<RenderCacheKey>,
    best_range_cache: Option<RangeStats>,
}

#[wasm_bindgen]
impl AppBackend {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            raw_floats: vec![],
            base_floats: vec![],
            active_floats: vec![],

            raw_importance: vec![],
            base_importance: vec![],
            active_importance: vec![],
            active_importance_intensity: vec![],
            imp_block_stats: vec![],
            imp_block_size: 0,

            last_output: None,
            clip_start: 0,
            clip_end: 0,

            data_version: 1,
            minmax_version: 0,
            minmax_cache: None,

            best_indices: vec![],
            best_key: None,
            best_range_cache: None,
        }
    }

    /// Invalidates all render and minmax caches indicating underlying data mutated.
    fn bump_version(&mut self) {
        self.data_version = self.data_version.wrapping_add(1);
        self.best_key = None;
        self.best_range_cache = None;
        self.minmax_cache = None;
        self.minmax_version = 0;
    }

    pub fn parse_floats(&mut self, text: &str) -> usize {
        self.raw_floats = text
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<f32>().ok())
            .collect();

        self.bump_version();
        self.raw_floats.len()
    }

    pub fn parse_importance(&mut self, text: &str) -> usize {
        let raw: Vec<f32> = text
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<f32>().ok())
            .map(|v| v.max(0.0))
            .collect();

        self.raw_importance = raw;
        self.base_importance = self.raw_importance.clone();
        self.bump_version();
        self.base_importance.len()
    }

    pub fn set_scale_offset(&mut self, scale: f32, offset: f32) {
        self.base_floats = self
            .raw_floats
            .iter()
            .map(|&v| v * scale + offset)
            .collect();

        self.bump_version();
    }

    pub fn set_clip(&mut self, start: usize, end: usize) {
        let n = self.base_floats.len();
        if n == 0 {
            self.clip_start = 0;
            self.clip_end = 0;
            self.active_floats.clear();
            self.active_importance.clear();

            self.best_key = None;
            self.best_range_cache = None;
            return;
        }

        self.clip_start = start.min(n.saturating_sub(1));
        self.clip_end = end.min(n.saturating_sub(1)).max(self.clip_start);

        self.active_floats.clear();
        self.active_floats
            .extend_from_slice(&self.base_floats[self.clip_start..=self.clip_end]);

        self.active_importance.clear();
        if self.base_importance.len() == self.base_floats.len() {
            self.active_importance
                .extend_from_slice(&self.base_importance[self.clip_start..=self.clip_end]);
        }

        self.best_key = None;
        self.best_range_cache = None;
    }

    fn prepare_importance(&mut self, imp_block_size: usize) {
        let n = self.active_importance.len();
        if n == 0 {
            return;
        }

        let b_size = if imp_block_size == 0 {
            n
        } else {
            imp_block_size
        };
        self.imp_block_size = b_size;

        self.active_importance_intensity.resize(n, 0.0);
        let num_blocks = (n + b_size - 1) / b_size;
        self.imp_block_stats.clear();
        self.imp_block_stats.reserve(num_blocks);

        for i in (0..n).step_by(b_size) {
            let chunk_len = (n - i).min(b_size);

            let mut max = 0.0_f32;
            let mut sum = 0.0_f32;
            for j in 0..chunk_len {
                let v = self.active_importance[i + j];
                if v > max {
                    max = v;
                }
                sum += v;
            }

            if sum == 0.0 {
                max = 1.0;
                sum = chunk_len as f32;
                for j in 0..chunk_len {
                    self.active_importance[i + j] = 1.0;
                    self.active_importance_intensity[i + j] = 1.0;
                }
            } else {
                for j in 0..chunk_len {
                    self.active_importance_intensity[i + j] = self.active_importance[i + j] / max;
                }
            }
            self.imp_block_stats.push(ImpBlockStat { sum, max });
        }
    }

    /// Primary entry point that dynamically routes and triggers specific algorithm quantization.
    pub fn quantize(&mut self, settings_js: JsValue) -> JsValue {
        let settings: Settings = serde_wasm_bindgen::from_value(settings_js).unwrap();

        let imp_block_size = get_importance_block_size(&settings.q_type, &settings);
        if settings.use_importance && self.active_importance.len() == self.active_floats.len() {
            self.prepare_importance(imp_block_size);
        }

        let mut has_importance = false;
        let mut weighted_mse = 0.0;
        let mut weighted_mae = 0.0;
        let mut weighted_snr = f64::INFINITY;
        let mut max_weighted_error = 0.0;

        let importance_opt = if settings.use_importance
            && self.active_importance.len() == self.active_floats.len()
        {
            has_importance = true;
            Some(ImportanceResult {
                raw: &self.active_importance,
                intensity: &self.active_importance_intensity,
                block_stats: &self.imp_block_stats,
                block_size: self.imp_block_size,
            })
        } else {
            None
        };

        let output = match settings.q_type.as_str() {
            "primitive" => {
                primitive::quantize(&self.active_floats, importance_opt.as_ref(), &settings)
            }
            "sym" => sym::quantize(&self.active_floats, importance_opt.as_ref(), &settings),
            "asym" => asym::quantize(&self.active_floats, importance_opt.as_ref(), &settings),
            "kquant" => kquant::quantize(&self.active_floats, importance_opt.as_ref(), &settings),
            "nvfp4" => nvfp4::quantize(&self.active_floats, importance_opt.as_ref(), &settings),
            "mxfp" => mxfp::quantize(&self.active_floats, importance_opt.as_ref(), &settings),
            "turbo" => turbo::quantize(&self.active_floats, importance_opt.as_ref(), &settings),
            "trellis" => trellis::quantize(&self.active_floats, importance_opt.as_ref(), &settings),
            _ => primitive::quantize(&self.active_floats, importance_opt.as_ref(), &settings),
        };

        let (global_mse, global_mae) = get_err_stats(&self.active_floats, &output.q_floats);
        let mut sig_power = 0.0_f64;
        let mut sum_abs = 0.0_f64;
        let mut max_error = 0.0_f64;

        for i in 0..self.active_floats.len() {
            let vf = self.active_floats[i] as f64;
            let qf = output.q_floats[i] as f64;
            sig_power += vf * vf;
            sum_abs += vf.abs();
            let err = (vf - qf).abs();
            if err > max_error {
                max_error = err;
            }
        }

        let global_variance = if self.active_floats.is_empty() {
            0.0
        } else {
            sig_power / self.active_floats.len() as f64
        };

        if has_importance {
            let mut w_mse = 0.0;
            let mut w_mae = 0.0;
            let mut w_sum = 0.0;
            let mut w_sig_power = 0.0;
            for i in 0..self.active_floats.len() {
                let diff = (self.active_floats[i] - output.q_floats[i]) as f64;
                let w = self.active_importance[i] as f64;
                let val = self.active_floats[i] as f64;

                let err_sq = diff * diff;
                let w_err_sq = err_sq * w;
                let w_err_abs = diff.abs() * w;

                if w_err_abs > max_weighted_error {
                    max_weighted_error = w_err_abs;
                }

                w_mse += w_err_sq;
                w_mae += w_err_abs;
                w_sig_power += w * val * val;
                w_sum += w;
            }
            if w_sum > 1e-12 {
                weighted_mse = w_mse / w_sum;
                weighted_mae = w_mae / w_sum;
                let weighted_variance = w_sig_power / w_sum;
                if weighted_mse > 0.0 {
                    weighted_snr = 10.0 * (weighted_variance / weighted_mse).log10();
                }
            }
        }

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
            max_error,
            has_importance,
            weighted_mse,
            weighted_mae,
            weighted_snr,
            max_weighted_error,
            imp_block_size: if has_importance {
                self.imp_block_size
            } else {
                0
            },
        };

        self.last_output = Some(output);
        self.best_key = None;
        self.best_range_cache = None;

        serde_wasm_bindgen::to_value(&stats).unwrap()
    }

    fn get_val_for_global_idx(&self, g: usize, use_srht: bool) -> f32 {
        if use_srht {
            if let Some(out) = &self.last_output {
                if let Some(t) = out.t_floats.as_ref() {
                    if g >= self.clip_start && g <= self.clip_end {
                        return t[g - self.clip_start];
                    }
                }
            }
        }
        self.base_floats[g]
    }

    /// Evaluates and filters min-max limits of bounds, managing artificial dataset extremes.
    fn calculate_bounds(
        &self,
        zs: usize,
        ze: usize,
        effective_srht: bool,
        settings: &Settings,
    ) -> (f32, f32) {
        if settings.centering_mode == "manual" {
            return (settings.axis_manual_min, settings.axis_manual_max);
        }

        let n = ze.saturating_sub(zs) + 1;
        if n == 0 {
            return (-1.0, 1.0);
        }

        let mut d_min = f32::INFINITY;
        let mut d_max = f32::NEG_INFINITY;

        if settings.axis_ignore_outliers && settings.axis_outlier_pct > 0.0 && n > 2 {
            let mut vals = Vec::with_capacity(n);
            for i in zs..=ze {
                vals.push(self.get_val_for_global_idx(i, effective_srht));
            }

            let pct = (settings.axis_outlier_pct.clamp(0.0, 49.9) / 100.0) as f32;
            let mut k_low = ((n as f32) * pct).floor() as usize;
            let mut k_high = ((n as f32) * (1.0 - pct)).ceil() as usize;

            k_low = k_low.clamp(0, n.saturating_sub(1));
            k_high = k_high.clamp(0, n.saturating_sub(1)).max(k_low);

            if k_low == 0 && k_high >= n - 1 {
                d_min = *vals
                    .iter()
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(&0.0);
                d_max = *vals
                    .iter()
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(&0.0);
            } else {
                let (_, low_val, upper_slice) = vals.select_nth_unstable_by(k_low, |a, b| {
                    a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                });
                d_min = *low_val;

                let k_high_rel = k_high - k_low;
                if k_high_rel < upper_slice.len() {
                    let (_, high_val, _) = upper_slice
                        .select_nth_unstable_by(k_high_rel, |a, b| {
                            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    d_max = *high_val;
                } else {
                    d_max = *upper_slice
                        .iter()
                        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                        .unwrap_or(&d_min);
                }
            }
        } else {
            for i in zs..=ze {
                let v = self.get_val_for_global_idx(i, effective_srht);
                if v < d_min {
                    d_min = v;
                }
                if v > d_max {
                    d_max = v;
                }
            }
        }

        if d_min > d_max {
            d_min = 0.0;
            d_max = 0.0;
        }
        if d_min == d_max {
            d_min -= 0.1;
            d_max += 0.1;
        }

        match settings.centering_mode.as_str() {
            "data" => {
                let spread = d_max - d_min;
                (d_min - spread * 0.05, d_max + spread * 0.05)
            }
            "zero" | _ => {
                let mut abs_max = d_max.abs().max(d_min.abs());
                if abs_max == 0.0 {
                    abs_max = 0.1;
                }
                let spread = abs_max * 1.1;
                (-spread, spread)
            }
        }
    }

    pub fn get_range_stats(
        &self,
        z_start: usize,
        z_end: usize,
        use_srht: bool,
        settings_js: JsValue,
    ) -> JsValue {
        let n = self.base_floats.len();
        if n == 0 {
            let rs = RangeStats {
                render_min: -1.0,
                render_max: 1.0,
            };
            return serde_wasm_bindgen::to_value(&rs).unwrap();
        }

        let settings: Settings = serde_wasm_bindgen::from_value(settings_js).unwrap();

        let zs = z_start.min(n - 1);
        let ze = z_end.min(n - 1).max(zs);

        let effective_srht = use_srht
            && self
                .last_output
                .as_ref()
                .and_then(|o| o.t_floats.as_ref())
                .is_some();

        let (render_min, render_max) = self.calculate_bounds(zs, ze, effective_srht, &settings);

        let rs = RangeStats {
            render_min,
            render_max,
        };
        serde_wasm_bindgen::to_value(&rs).unwrap()
    }

    pub fn prepare_render(
        &mut self,
        z_start: usize,
        z_end: usize,
        width_css: f64,
        use_srht: bool,
        settings_js: JsValue,
    ) -> JsValue {
        let n = self.base_floats.len();
        if n == 0 || width_css <= 0.0 {
            self.best_indices.clear();
            self.best_key = None;
            self.best_range_cache = Some(RangeStats {
                render_min: -1.0,
                render_max: 1.0,
            });
            return serde_wasm_bindgen::to_value(self.best_range_cache.as_ref().unwrap()).unwrap();
        }

        let settings: Settings = serde_wasm_bindgen::from_value(settings_js).unwrap();

        let zs = z_start.min(n - 1);
        let ze = z_end.min(n - 1).max(zs);

        let effective_srht = use_srht
            && self
                .last_output
                .as_ref()
                .and_then(|o| o.t_floats.as_ref())
                .is_some();

        let w = width_css.ceil().max(1.0) as usize;
        let width_bits = width_css.to_bits();

        let key = RenderCacheKey {
            data_version: self.data_version,
            zs,
            ze,
            width_bits,
            effective_srht,
            clip_start: self.clip_start,
            clip_end: self.clip_end,
            centering_mode: settings.centering_mode.clone(),
            ignore_outliers: settings.axis_ignore_outliers,
            outlier_pct_bits: settings.axis_outlier_pct.to_bits(),
            manual_min_bits: settings.axis_manual_min.to_bits(),
            manual_max_bits: settings.axis_manual_max.to_bits(),
        };

        if self.best_key.as_ref() == Some(&key) {
            if let Some(rs) = &self.best_range_cache {
                return serde_wasm_bindgen::to_value(rs).unwrap();
            }
        }

        self.best_indices.resize(w, 0);
        let z_count = (ze - zs + 1) as f64;

        // Perform maximum magnitude mipmapping per pixel width.
        for x in 0..w {
            let fx0 = (x as f64) / width_css;
            let fx1 = ((x + 1) as f64) / width_css;

            let mut bin_s = zs + (fx0 * z_count).floor() as usize;
            let mut bin_e_excl = zs + (fx1 * z_count).floor() as usize;

            if bin_s > ze {
                bin_s = ze;
            }
            if bin_e_excl > (ze + 1) {
                bin_e_excl = ze + 1;
            }

            let mut bin_e = if bin_e_excl > zs {
                bin_e_excl.saturating_sub(1)
            } else {
                bin_s
            };
            if bin_e < bin_s {
                bin_e = bin_s;
            }
            if bin_e > ze {
                bin_e = ze;
            }

            let mut best_idx = bin_s;
            let mut max_mag = -1.0f32;

            for j in bin_s..=bin_e {
                let v = self.get_val_for_global_idx(j, effective_srht);
                let mag = v.abs();
                if mag > max_mag {
                    max_mag = mag;
                    best_idx = j;
                }
            }

            self.best_indices[x] = best_idx as i32;
        }

        let (render_min, render_max) = self.calculate_bounds(zs, ze, effective_srht, &settings);

        let rs = RangeStats {
            render_min,
            render_max,
        };
        self.best_range_cache = Some(rs.clone());
        self.best_key = Some(key);

        serde_wasm_bindgen::to_value(&rs).unwrap()
    }

    pub fn get_best_indices_ptr(&self) -> *const i32 {
        self.best_indices.as_ptr()
    }
    pub fn get_best_indices_len(&self) -> usize {
        self.best_indices.len()
    }

    pub fn get_global_minmax(&mut self) -> JsValue {
        if self.base_floats.is_empty() {
            let mm = MinMaxInfo {
                min_idx: 0,
                max_idx: 0,
                min_v: 0.0,
                max_v: 0.0,
            };
            return serde_wasm_bindgen::to_value(&mm).unwrap();
        }

        if self.minmax_version == self.data_version {
            if let Some(mm) = &self.minmax_cache {
                return serde_wasm_bindgen::to_value(mm).unwrap();
            }
        }

        let mut min_idx = 0usize;
        let mut max_idx = 0usize;
        let mut min_v = f32::INFINITY;
        let mut max_v = f32::NEG_INFINITY;

        for (i, &v) in self.base_floats.iter().enumerate() {
            if v < min_v {
                min_v = v;
                min_idx = i;
            }
            if v > max_v {
                max_v = v;
                max_idx = i;
            }
        }

        let mm = MinMaxInfo {
            min_idx,
            max_idx,
            min_v,
            max_v,
        };
        self.minmax_cache = Some(mm.clone());
        self.minmax_version = self.data_version;

        serde_wasm_bindgen::to_value(&mm).unwrap()
    }

    // Standardized memory-bridge getters for JavaScript interop.
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
    pub fn get_active_importance_ptr(&self) -> *const f32 {
        self.active_importance.as_ptr()
    }
    pub fn get_active_importance_intensity_ptr(&self) -> *const f32 {
        self.active_importance_intensity.as_ptr()
    }
    pub fn get_base_importance_ptr(&self) -> *const f32 {
        self.base_importance.as_ptr()
    }
    pub fn get_base_importance_len(&self) -> usize {
        self.base_importance.len()
    }
    pub fn get_global_max_importance(&self) -> f32 {
        self.base_importance.iter().cloned().fold(0.0_f32, f32::max)
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

    /// Uniform interface to fetch rendering metadata about specific index interactions.
    pub fn get_inspector_data(&self, idx: usize, settings_js: JsValue) -> JsValue {
        let settings: Settings = serde_wasm_bindgen::from_value(settings_js).unwrap();
        if let Some(out) = &self.last_output {
            let mut data = match settings.q_type.as_str() {
                "primitive" => {
                    primitive::format_inspector(idx, &self.active_floats, out, &settings)
                }
                "sym" => sym::format_inspector(idx, &self.active_floats, out, &settings),
                "asym" => asym::format_inspector(idx, &self.active_floats, out, &settings),
                "kquant" => kquant::format_inspector(idx, &self.active_floats, out, &settings),
                "nvfp4" => nvfp4::format_inspector(idx, &self.active_floats, out, &settings),
                "mxfp" => mxfp::format_inspector(idx, &self.active_floats, out, &settings),
                "turbo" => turbo::format_inspector(idx, &self.active_floats, out, &settings),
                "trellis" => trellis::format_inspector(idx, &self.active_floats, out, &settings),
                _ => primitive::format_inspector(idx, &self.active_floats, out, &settings),
            };
            if settings.use_importance && self.active_importance.len() == self.active_floats.len() {
                let b_size = self.imp_block_size;
                let b_idx = idx / b_size;
                if let Some(stats) = self.imp_block_stats.get(b_idx) {
                    let raw = self.active_importance[idx];
                    data.importance_raw = Some(raw);
                    data.importance_pct_sum = Some(if stats.sum > 0.0 {
                        (raw / stats.sum) * 100.0
                    } else {
                        0.0
                    });
                    data.importance_pct_max = Some(if stats.max > 0.0 {
                        (raw / stats.max) * 100.0
                    } else {
                        0.0
                    });
                }
            }
            serde_wasm_bindgen::to_value(&data).unwrap()
        } else {
            JsValue::NULL
        }
    }
}
