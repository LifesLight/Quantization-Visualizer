use crate::math_utils::*;
use crate::quants::{InspectorData, QuantMeta, QuantizeOutput, Settings};

/// Applies baseline data-type truncations mirroring industry primitive sizes.
pub fn quantize(floats: &[f32], settings: &Settings) -> QuantizeOutput {
    let (bpw, format_name) = match settings.primitive_format.as_str() {
        "fp16" | "bf16" => (16.0, settings.primitive_format.to_uppercase()),
        "fp8_e4m3" => (8.0, "FP8 (E4M3)".to_string()),
        "fp8_e5m2" => (8.0, "FP8 (E5M2)".to_string()),
        _ => (32.0, "FP32".to_string()),
    };

    let q_floats: Vec<f32> = floats
        .iter()
        .map(|&v| match settings.primitive_format.as_str() {
            "fp16" => fp16(v),
            "bf16" => bf16(v),
            "fp8_e4m3" => fp8_e4m3(v),
            "fp8_e5m2" => fp8_e5m2(v),
            _ => v,
        })
        .collect();

    QuantizeOutput {
        q_floats,
        t_floats: None,
        t_q_floats: None,
        bpw,
        formula_html: format!("Weights in {} precision.", format_name),
        block_size: 0,
        super_block_size: 0,
        meta: QuantMeta::Primitive,
    }
}

/// Primitive quantizer has no underlying block parameters to format.
pub fn format_inspector(
    _idx: usize,
    _active: &[f32],
    _out: &QuantizeOutput,
    _settings: &Settings,
) -> InspectorData {
    InspectorData::default()
}
