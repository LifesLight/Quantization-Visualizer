pub mod iq1_m;
pub mod iq1_s;
pub mod iq2_s;
pub mod iq2_xs;
pub mod iq2_xxs;
pub mod iq3_s;
pub mod iq3_xxs;
pub mod iq4_nl;
pub mod iq4_xs;

use half::f16;

use crate::quants::iq::grid::IqGrid;

pub const QK_K: usize = 256;

pub const GROUP_MAX_EPS: f32 = 1e-15;
pub const GROUP_MAX_EPS_IQ3_XXS: f32 = 1e-8;
pub const GROUP_MAX_EPS_IQ2_S: f32 = 1e-8;
pub const GROUP_MAX_EPS_IQ1_M: f32 = 1e-7;
pub const GROUP_MAX_EPS_IQ1_S: f32 = 1e-12;

/// Static description of one IQ format, so the dispatcher doesn't need a match per call.
pub struct IqType {
    pub name: &'static str,
    /// Weights per stored block (256, or 32 for IQ4_NL).
    pub super_size: usize,
    /// Weights sharing one sub-block scale.
    pub sub_size: usize,
    /// Weights sharing one codebook index.
    pub group_size: usize,
    pub bytes: usize,
    /// llama.cpp refuses to quantize these without an imatrix.
    pub needs_imatrix: bool,
    pub quantize: fn(&[f32], Option<&[f32]>) -> Encoded,
    pub dequantize: fn(&[u8], &mut [f32]),
    pub decode: fn(&[u8], usize) -> IqDecode,
    pub sub_scales: fn(&[u8]) -> Vec<f32>,
    pub formula_html: fn() -> String,
}

impl IqType {
    pub fn bpw(&self) -> f32 {
        (self.bytes * 8) as f32 / self.super_size as f32
    }
}

/// One quantized block in the exact llama.cpp byte layout.
pub struct Encoded {
    pub bytes: Vec<u8>,
    /// Per codebook group: 0 if the rounded lattice point was in the grid,
    /// otherwise the number of neighbours that had to be scanned for it.
    pub probe: Vec<u16>,
    /// Anything odd the reference did with this block, shown in the inspector.
    pub note: Option<(String, String)>,
}

pub enum Signs {
    None,
    /// 8 signs per group, 7 stored, the 8th implied by an even number of negatives.
    Parity {
        neg: u8,
    },
    /// All 8 sign bits are stored.
    Full {
        neg: u8,
    },
}

pub enum Codebook {
    /// A lattice entry covering `levels.len()` weights.
    Grid {
        idx: usize,
        size: usize,
        levels: Vec<f32>,
    },
    /// Per-weight index into a non-linear 16 entry table (IQ4).
    NonLinear { idx: usize, values: &'static [i8] },
}

/// Everything the inspector needs to explain how one weight was decoded.
pub struct IqDecode {
    pub d: f32,
    pub sub_field: u32,
    pub sub_bits: u32,
    /// Effective multiplier applied to the codebook value.
    pub dl: f32,
    /// How dl is built from d, with {s} standing for the stored sub-block field.
    pub dl_expr: String,
    pub codebook: Codebook,
    pub signs: Signs,
    /// IQ1 shift added to the grid value before scaling.
    pub delta: Option<f32>,
    pub math: String,
}

/// Formula line plus a one line summary. The longer explanation lives in the hover tooltip.
pub fn formula(pills: &str, short: &str, long: &str) -> String {
    format!(
        "<span>Weight = {}</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;cursor:help;text-decoration:underline dotted;\" title=\"{}\">{}</span>",
        pills, long, short
    )
}

/// Port of ggml's nearest_int, rounds half to even through the 1.5*2^23 trick.
pub fn nearest_int(fval: f32) -> i32 {
    let val = fval + 12582912.0;
    (val.to_bits() & 0x007fffff) as i32 - 0x00400000
}

pub fn best_index_int8(values: &[i8], x: f32) -> usize {
    let n = values.len();
    if x <= values[0] as f32 {
        return 0;
    }
    if x >= values[n - 1] as f32 {
        return n - 1;
    }
    let mut ml = 0;
    let mut mu = n - 1;
    while mu - ml > 1 {
        let mav = (ml + mu) / 2;
        if x < values[mav] as f32 {
            mu = mav;
        } else {
            ml = mav;
        }
    }
    if x - (values[mu - 1] as f32) < (values[mu] as f32) - x {
        mu - 1
    } else {
        mu
    }
}

pub fn f16_bits(v: f32) -> u16 {
    f16::from_f32(v).to_bits()
}

pub fn f16_val(bits: u16) -> f32 {
    f16::from_bits(bits).to_f32()
}

pub fn get_u16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

pub fn put_u16(b: &mut [u8], off: usize, v: u16) {
    b[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

pub fn get_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

pub fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// Byte `j` of a packed grid row (u64 rows for the 8-wide grids, u32 for the 4-wide ones).
pub fn grid_byte(row: u64, j: usize) -> u8 {
    (row >> (8 * j)) as u8
}

/// Splits a group of 8 into magnitudes and sign bits. With an odd number of negatives the
/// element with the smallest w*x^2 gets its sign flipped, so only 7 bits need storing.
/// The flipped element keeps a negative "magnitude" and gets quantized as such.
pub fn even_signs(xb: &[f32], weight: &[f32], xval: &mut [f32]) -> u8 {
    let mut nflip = 0;
    let mut s = 0u8;
    for i in 0..8 {
        if xb[i] >= 0.0 {
            xval[i] = xb[i];
        } else {
            xval[i] = -xb[i];
            nflip += 1;
            s |= 1 << i;
        }
    }
    if nflip % 2 != 0 {
        let mut imin = 0;
        let mut min = weight[0] * xb[0] * xb[0];
        for i in 1..8 {
            let ax = weight[i] * xb[i] * xb[i];
            if ax < min {
                min = ax;
                imin = i;
            }
        }
        xval[imin] = -xval[imin];
        s ^= 1 << imin;
    }
    s & 127
}

/// Port of make_qp_quants: best non-negative integer levels 0..nmax for x >= 0.
pub fn make_qp_quants(nmax: i32, x: &[f32], l: &mut [u8], w: &[f32]) -> f32 {
    let n = x.len();
    let mut max = 0.0f32;
    for &v in x {
        if v > max {
            max = v;
        }
    }
    if max < GROUP_MAX_EPS {
        l[..n].fill(0);
        return 0.0;
    }
    let mut iscale = nmax as f32 / max;
    for i in 0..n {
        l[i] = nearest_int(iscale * x[i]) as u8;
    }
    let scale = 1.0 / iscale;
    let mut best_mse = 0.0f32;
    for i in 0..n {
        let diff = x[i] - scale * l[i] as f32;
        best_mse += w[i] * diff * diff;
    }
    for is in -4..=4 {
        if is == 0 {
            continue;
        }
        let iscale_is = (0.1 * is as f32 + nmax as f32) / max;
        let scale_is = 1.0 / iscale_is;
        let mut mse = 0.0f32;
        for i in 0..n {
            let li = nearest_int(iscale_is * x[i]).min(nmax);
            let diff = x[i] - scale_is * li as f32;
            mse += w[i] * diff * diff;
        }
        if mse < best_mse {
            best_mse = mse;
            iscale = iscale_is;
        }
    }
    let mut sumlx = 0.0f32;
    let mut suml2 = 0.0f32;
    for i in 0..n {
        let li = nearest_int(iscale * x[i]).min(nmax);
        l[i] = li as u8;
        sumlx += w[i] * x[i] * li as f32;
        suml2 += w[i] * li as f32 * li as f32;
    }
    for _ in 0..5 {
        let mut n_changed = 0;
        for i in 0..n {
            let wi = w[i];
            let li = l[i] as f32;
            let mut slx = sumlx - wi * x[i] * li;
            let mut sl2 = suml2 - wi * li * li;
            if slx > 0.0 && sl2 > 0.0 {
                let new_l = nearest_int(x[i] * sl2 / slx).min(nmax);
                if new_l != l[i] as i32 {
                    slx += wi * x[i] * new_l as f32;
                    sl2 += wi * new_l as f32 * new_l as f32;
                    if slx * slx * suml2 > sumlx * sumlx * sl2 {
                        l[i] = new_l as u8;
                        sumlx = slx;
                        suml2 = sl2;
                        n_changed += 1;
                    }
                }
            }
        }
        if n_changed == 0 {
            break;
        }
    }
    if suml2 > 0.0 {
        sumlx / suml2
    } else {
        0.0
    }
}

/// Port of iq2_find_best_neighbour / iq3_find_best_neighbour.
/// Picks the candidate with the lowest weighted error and writes its levels into `l`.
pub fn find_best_neighbour<const N: usize>(
    neighbours: &[u16],
    grid: &IqGrid<N>,
    xval: &[f32],
    weight: &[f32],
    scale: f32,
    l: &mut [i8],
) -> usize {
    let mut best_d2 = f32::MAX;
    let mut grid_index = neighbours[0] as usize;
    for &nb in neighbours {
        let pg = &grid.points[nb as usize];
        let mut d2 = 0.0f32;
        for i in 0..N {
            let q = pg[i] as f32;
            let diff = scale * q - xval[i];
            d2 += weight[i] * diff * diff;
        }
        if d2 < best_d2 {
            best_d2 = d2;
            grid_index = nb as usize;
        }
    }
    let pg = &grid.points[grid_index];
    for i in 0..N {
        l[i] = (pg[i] - 1) / 2;
    }
    grid_index
}

/// Port of iq1_find_best_neighbour2. `xg` holds the three shifted levels (-1, 0, 1) + delta.
pub fn iq1_find_best_neighbour2(
    neighbours: &[u16],
    grid: &IqGrid<8>,
    xval: &[f32],
    weight: &[f32],
    scale: f32,
    xg: &[f32; 3],
    l: &mut [i8],
) -> usize {
    let mut best_score = f32::MAX;
    let mut grid_index: i32 = -1;
    for &nb in neighbours {
        let pg = &grid.points[nb as usize];
        let mut d2 = 0.0f32;
        for i in 0..8 {
            let q = xg[((pg[i] - 1) / 2) as usize];
            let w = weight[i];
            let diff = scale * q - xval[i];
            d2 += w * diff * diff;
        }
        if d2 < best_score {
            best_score = d2;
            grid_index = nb as i32;
        }
    }
    if grid_index < 0 {
        // Only reachable with non-finite inputs. The C fallback indexes xval with the
        // grid index here, which reads out of bounds, so this uses the element index.
        for (i, pg) in grid.points.iter().enumerate() {
            let mut d2 = 0.0f32;
            for j in 0..8 {
                let w = weight[j];
                let q = xg[((pg[j] - 1) / 2) as usize];
                let diff = scale * q - xval[j];
                d2 += w * diff * diff;
            }
            if d2 < best_score {
                best_score = d2;
                grid_index = i as i32;
            }
        }
    }
    let grid_index = grid_index.max(0) as usize;
    let pg = &grid.points[grid_index];
    for i in 0..8 {
        l[i] = (pg[i] - 1) / 2;
    }
    grid_index
}

/// Packs per-coordinate levels into the kmap index.
pub fn pack_levels(l: &[i8], bits: usize) -> usize {
    let mut u = 0usize;
    for (i, &v) in l.iter().enumerate() {
        u |= (v as usize) << (bits * i);
    }
    u
}

/// Indices of `xb` sorted by value, used by the IQ1 split search.
pub fn sort_pairs(xb: &[f32], idx: &mut [usize]) {
    for (j, v) in idx.iter_mut().enumerate() {
        *v = j;
    }
    // qsort with a value-only comparator, glibc's merge sort keeps ties in order
    idx.sort_by(|&a, &b| {
        xb[a]
            .partial_cmp(&xb[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

pub fn sign_char(neg: bool) -> &'static str {
    if neg {
        "&minus;"
    } else {
        "+"
    }
}
