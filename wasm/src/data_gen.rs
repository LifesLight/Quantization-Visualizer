use std::f64::consts::PI;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Math)]
    fn random() -> f64;
}

/// Generates a random non-zero float
fn rand_nonzero() -> f64 {
    let mut r = random();
    while r == 0.0 {
        r = random();
    }
    r
}

#[wasm_bindgen]
pub fn generate_dataset(
    dist: &str,
    count: usize,
    uni_range: f64,
    lap_scale: f64,
    bim_dist: f64,
    bim_spread: f64,
    out_prob: f64,
    out_mult: f64,
    norm_std: f64,
) -> String {
    let mut arr = Vec::with_capacity(count);

    for _ in 0..count {
        let val = match dist {
            "uniform" => (random() * uni_range) - (uni_range / 2.0),
            "laplace" => {
                let u = random() - 0.5;
                -lap_scale * u.signum() * (1.0 - 2.0 * u.abs()).ln()
            }
            "bimodal" => {
                let peak = if random() > 0.5 { bim_dist } else { -bim_dist };
                let u = rand_nonzero();
                let v = rand_nonzero();
                // Box-Muller transform
                let norm = (-2.0 * u.ln()).sqrt() * (2.0 * PI * v).cos();
                peak + norm * bim_spread
            }
            "outliers" => {
                let u = rand_nonzero();
                let v = rand_nonzero();
                let mut norm = (-2.0 * u.ln()).sqrt() * (2.0 * PI * v).cos();
                if random() < out_prob {
                    norm *= if random() > 0.5 { out_mult } else { -out_mult };
                }
                norm
            }
            _ => {
                // Standard normal distribution via Box-Muller
                let u = rand_nonzero();
                let v = rand_nonzero();
                let norm = (-2.0 * u.ln()).sqrt() * (2.0 * PI * v).cos();
                norm * norm_std
            }
        };
        arr.push(format!("{:.5}", val));
    }

    arr.join(", ")
}
