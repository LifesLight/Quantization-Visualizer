//! TurboQuant (Zandieh et al., arXiv:2504.19874), Algorithm 1 (TurboQuant_mse) and
//! Algorithm 2 (TurboQuant_prod). The paper's Π is the global rotation (Random Orthogonal on the
//! quant block), so this only sees rotated vectors of d weights: each one is divided by its norm
//! and every coordinate snaps to the nearest Lloyd-Max level.
//! _prod spends one of the b bits on QJL of the residual: sign(S·r) and ‖r‖. Doing that on the
//! rotated residual is the paper's algorithm with S·Π in place of S, which is just as Gaussian.
//! The paper assumes unit vectors, the norm (and ‖r‖) is stored as FP16 per vector here.

pub mod codebook;
pub mod qjl;
mod view;

use crate::math_utils::*;
use crate::quants::iq::algos::formula;
use crate::quants::{ImportanceResult, InspectorData, QuantMeta, QuantizeOutput, Settings};
use crate::rotation::{Kind, Rotation};

pub struct TurboVec {
    /// ‖x‖, FP16.
    pub norm: f32,
    /// ‖r‖ of the residual, FP16, only for _prod.
    pub gamma: f32,
    /// Level index of every coordinate.
    pub idx: Vec<u8>,
    /// QJL signs, sign(S·r) >= 0.
    pub z: Vec<bool>,
    pub mse: f64,
    pub mae: f64,
}

pub struct TurboMeta {
    pub vecs: Vec<TurboVec>,
}

struct Params {
    d: usize,
    bits: u32,
    prod: bool,
}

fn params(settings: &Settings) -> Params {
    Params {
        d: settings.turbo_block_size.max(4),
        bits: settings.turbo_bits.clamp(1, 8),
        prod: settings.use_qjl,
    }
}

impl Params {
    /// _prod quantizes with TurboQuant_mse at b - 1 bits (0 bits is the single level 0).
    fn mse_bits(&self) -> u32 {
        if self.prod {
            self.bits - 1
        } else {
            self.bits
        }
    }
}

/// Expected ‖x−x̃‖²/‖x‖² of _prod: QJL leaves (π/2 − 1/d)·‖r‖² of the residual's error.
pub fn prod_distortion(d: usize, mse_distortion: f64) -> f64 {
    (std::f64::consts::FRAC_PI_2 - 1.0 / d as f64) * mse_distortion
}

/// √(π/2)/d of DeQuant_prod, d being the length of this vector.
fn qjl_scale(len: usize) -> f32 {
    (std::f32::consts::PI / 2.0).sqrt() / len as f32
}

pub fn quantize(
    floats: &[f32],
    _importance: Option<&ImportanceResult>,
    settings: &Settings,
) -> QuantizeOutput {
    let p = params(settings);
    let d = p.d;
    let s = if p.prod {
        Some(qjl::matrix(d, settings.turbo_seed))
    } else {
        None
    };

    let mut q_floats = vec![0.0f32; floats.len()];
    let mut vecs = Vec::with_capacity((floats.len() + d - 1) / d);

    for (v, chunk) in floats.chunks(d).enumerate() {
        let len = chunk.len();
        let base = v * d;
        // a short last vector gets the levels for its own length
        let cb = codebook::get(len, p.mse_bits());
        let norm = chunk
            .iter()
            .map(|&x| x as f64 * x as f64)
            .sum::<f64>()
            .sqrt() as f32;
        let norm_q = fp16(norm);
        let u: Vec<f32> = chunk
            .iter()
            .map(|&x| if norm > 0.0 { x / norm } else { 0.0 })
            .collect();

        // Quant_mse: nearest level per coordinate
        let idx: Vec<u8> = u.iter().map(|&y| cb.index(y) as u8).collect();
        let mut w: Vec<f32> = idx.iter().map(|&i| cb.levels[i as usize]).collect();

        let mut gamma = 0.0f32;
        let mut z = Vec::new();
        if let Some(s) = &s {
            // Quant_prod: r = u − x̃_mse, z = sign(S·r), keep ‖r‖
            let r: Vec<f32> = u.iter().zip(&w).map(|(&a, &b)| a - b).collect();
            gamma = fp16(r.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt() as f32);
            z = (0..len)
                .map(|k| {
                    let row = &s[k * d..k * d + len];
                    row.iter().zip(&r).map(|(a, b)| a * b).sum::<f32>() >= 0.0
                })
                .collect();
            // DeQuant_prod: √(π/2)/d·γ·Sᵀ·z
            let mut sz = vec![0.0f32; len];
            for k in 0..len {
                let sign = if z[k] { 1.0 } else { -1.0 };
                for (acc, &sv) in sz.iter_mut().zip(&s[k * d..k * d + len]) {
                    *acc += sign * sv;
                }
            }
            for j in 0..len {
                w[j] += qjl_scale(len) * gamma * sz[j];
            }
        }

        for j in 0..len {
            q_floats[base + j] = norm_q * w[j];
        }

        let (mse, mae) = get_err_stats(chunk, &q_floats[base..base + len]);
        vecs.push(TurboVec {
            norm: norm_q,
            gamma,
            idx,
            z,
            mse,
            mae,
        });
    }

    let side = if p.prod { 32 } else { 16 };
    let mut formula_html = formula_html(&p, codebook::get(d, p.mse_bits()).distortion);
    if p.prod {
        formula_html.push_str(&format!(
            " <span style=\"color:var(--text-muted);font-size:0.8rem;cursor:help;text-decoration:underline dotted;\" title=\"QJL makes inner products unbiased, the weights get worse: it leaves about &pi;/2 times the error of the residual it encodes. Expected &Vert;x&minus;x&#771;&Vert;&sup2;/&Vert;x&Vert;&sup2; &asymp; {:.3} here, TurboQuant_mse at {} bits &asymp; {:.3}.\">Higher MSE by design.</span>",
            prod_distortion(d, codebook::get(d, p.mse_bits()).distortion),
            p.bits,
            codebook::get(d, p.bits).distortion
        ));
    }
    if let Some(note) = rotation_note(&p, settings) {
        formula_html.push_str(&format!(
            " <span style=\"color:var(--text-muted);font-size:0.8rem;cursor:help;text-decoration:underline dotted;\" title=\"{}\">Not the paper rotation.</span>",
            note
        ));
    }
    QuantizeOutput {
        q_floats,
        bpw: p.bits as f32 + side as f32 / d as f32,
        formula_html,
        block_size: d,
        super_block_size: 0,
        meta: QuantMeta::Turbo(TurboMeta { vecs }),
    }
}

/// The paper rotates every d-vector with a random orthogonal Π, say so when that's not the case.
fn rotation_note(p: &Params, settings: &Settings) -> Option<String> {
    match Rotation::from_settings(settings) {
        None => Some("No rotation, the levels assume rotated coordinates. Rotation: Random Orthogonal is the paper setup.".into()),
        Some(r) if r.kind != Kind::Orthogonal || r.size != p.d => Some(format!(
            "The paper uses a random orthogonal rotation of size d = {}, this is {} {}.",
            p.d,
            r.name(),
            r.size
        )),
        _ => None,
    }
}

/// The level and the QJL part of coefficient j of a vector, before the norm.
/// Summed in the same order as quantize so the result matches q_floats.
fn parts(p: &Params, tv: &TurboVec, settings: &Settings, len: usize, j: usize) -> (f32, f32) {
    let level = codebook::get(len, p.mse_bits()).levels[tv.idx[j] as usize];
    if !p.prod {
        return (level, 0.0);
    }
    let s = qjl::matrix(p.d, settings.turbo_seed);
    let mut sz = 0.0f32;
    for k in 0..len {
        let sign = if tv.z[k] { 1.0 } else { -1.0 };
        sz += sign * s[k * p.d + j];
    }
    (level, qjl_scale(len) * tv.gamma * sz)
}

pub fn format_inspector(
    idx: usize,
    active: &[f32],
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::Turbo(meta) = &out.meta else {
        return InspectorData::default();
    };
    let p = params(settings);
    let v = idx / p.d;
    let j = idx % p.d;
    let mut data = InspectorData::default();
    let Some(tv) = meta.vecs.get(v) else {
        return data;
    };
    let base = v * p.d;
    let end = (base + p.d).min(active.len());
    let (level, qjl_part) = parts(&p, tv, settings, end - base, j);

    data.math_str = Some(if p.prod {
        format!(
            "{:.5} &times; ({:.5} {} {:.5})",
            tv.norm,
            level,
            if qjl_part < 0.0 { "&minus;" } else { "+" },
            qjl_part.abs()
        )
    } else {
        format!("{:.5} &times; {:.5}", tv.norm, level)
    });
    data.block_idx = Some(v);
    data.super_idx = Some(v);
    data.mse = Some(tv.mse);
    data.mae = Some(tv.mae);
    data.scale = Some(tv.norm);
    data.scale_label = Some("Norm (FP16):".into());
    if p.prod {
        data.qjl_scale = Some(tv.gamma);
    }

    let cb = codebook::get(end - base, p.mse_bits());
    data.detail_html = Some(view::render(
        &view::Hover {
            v,
            j,
            d: p.d,
            bits: p.bits,
            prod: p.prod,
            level,
            qjl_part,
            recon: out.q_floats[idx],
            rotated: settings.rotation_type != "none",
        },
        tv,
        &cb,
        &active[base..end],
        &out.q_floats[base..end],
    ));
    data
}

fn formula_html(p: &Params, distortion: f64) -> String {
    let b = p.bits;
    if p.prod {
        formula(
            &format!(
                "<span class=\"eq-pill\">&Vert;x&Vert;<span class=\"bits\">16b</span></span> &times; ( <span class=\"eq-pill\">c[idx]<span class=\"bits\">{}b</span></span> + &radic;(&pi;/2)/d &middot; <span class=\"eq-pill\">&gamma;<span class=\"bits\">16b</span></span> &middot; <span class=\"eq-pill\">S&#7488;&middot;qjl<span class=\"bits\">1b</span></span> )",
                b - 1
            ),
            &if b == 1 {
                format!("Only the 1-bit QJL, FP16 norm and &gamma; per {}.", p.d)
            } else {
                format!(
                    "{} levels plus 1-bit QJL of the residual, FP16 norm and &gamma; per {}.",
                    1u32 << (b - 1),
                    p.d
                )
            },
            &format!(
                "Algorithm 2 of the paper. The first {} bits are TurboQuant_mse, the residual r = x/&Vert;x&Vert; &minus; x&#771;_mse gets qjl = sign(S&middot;r) with S a seeded Gaussian d&times;d, decoded as &radic;(&pi;/2)/d&middot;&Vert;r&Vert;&middot;S&#7488;&middot;qjl. This makes inner products unbiased, not the weights more accurate. Norm and &gamma; = &Vert;r&Vert; are FP16 per vector. {:.3} bpw.",
                b - 1,
                b as f32 + 32.0 / p.d as f32
            ),
        )
    } else {
        formula(
            &format!(
                "<span class=\"eq-pill\">&Vert;x&Vert;<span class=\"bits\">16b</span></span> &times; <span class=\"eq-pill\">c[idx]<span class=\"bits\">{}b</span></span>",
                b
            ),
            &format!(
                "{} Lloyd-Max levels per coordinate, FP16 norm per {}.",
                1u32 << b,
                p.d
            ),
            &format!(
                "Algorithm 1 of the paper. Every {} weights are divided by their norm. After the random rotation each coordinate follows a Beta distribution close to N(0, 1/d) and snaps to the nearest of {} Lloyd-Max levels for it. Expected &Vert;x&minus;x&#771;&Vert;&sup2;/&Vert;x&Vert;&sup2; = {:.4} with the rotation. {:.3} bpw.",
                p.d,
                1u32 << b,
                distortion,
                b as f32 + 16.0 / p.d as f32
            ),
        )
    }
}
