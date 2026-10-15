//! Computed codebook of the KT types, port of QuantizerIQKT from ik_llama.cpp (iqk_quantize.cpp).
//! An index plus a fixed offset seeds an LCG. Every lane of a group advances it once
//! (x = 0xCBAC1FED * x) and takes the sum of the 4 bytes of x & 0x3f3f3f3f minus 126.
//! The clusters only exist to speed up the nearest point search while quantizing.
//!
//! The search mirrors the AVX2 path (the only one upstream supports): candidates are scored
//! 8 at a time with the same summation tree and the scale fit uses fused multiply-adds.

use std::sync::OnceLock;

const KA: u32 = 0xCBAC1FED;

/// LCG state of lane `lane` for a given index, after masking to 4 bytes of 0..63.
pub fn lane_state(idx: u32, offset: u32, lane: usize) -> u32 {
    let mut x = idx.wrapping_add(offset);
    for _ in 0..=lane {
        x = KA.wrapping_mul(x);
    }
    x & 0x3f3f3f3f
}

pub fn state_value(s: u32) -> f32 {
    let b = s.to_le_bytes();
    (b[0] as i32 + b[1] as i32 + b[2] as i32 + b[3] as i32) as f32 - 126.0
}

pub struct KtCodebook<const G: usize> {
    pub offset: u32,
    pub values: Vec<f32>,
    mid: [f32; 8],
    in_cluster: Vec<Vec<u32>>,
    c_values: Vec<Vec<f32>>,
}

impl<const G: usize> KtCodebook<G> {
    fn new(
        num_bits: u32,
        is_abs: bool,
        offset: u32,
        ncluster: usize,
        num_neighbours: usize,
    ) -> Self {
        let num_val = 1usize << num_bits;
        let mut values = vec![0.0f32; num_val * G];
        for i in 0..num_val {
            let mut x = (i as u32).wrapping_add(offset);
            for k in 0..G {
                x = KA.wrapping_mul(x);
                let v = state_value(x & 0x3f3f3f3f);
                values[G * i + k] = if is_abs { v.abs() } else { v };
            }
        }
        let mut mid = [0.0f32; 8];
        let clusters = cluster_points::<G>(&values, ncluster, is_abs, &mut mid);
        let (in_cluster, c_values) = finalize_clusters::<G>(num_neighbours, &values, &clusters);
        KtCodebook {
            offset,
            values,
            mid,
            in_cluster,
            c_values,
        }
    }

    fn cluster_of(&self, vx: &[f32; G]) -> usize {
        if G == 8 {
            let mut u = 0usize;
            for k in 0..8 {
                if vx[k] > self.mid[k] {
                    u |= 1 << k;
                }
            }
            u
        } else {
            let mut u = 0usize;
            let mut s = 1usize;
            for k in 0..G {
                u += s * bin5(vx[k]);
                s *= 5;
            }
            u
        }
    }

    /// Port of find_best_match: nearest codebook entry per group for the scale d.
    pub fn find_best_match(&self, d: f32, xb: &[f32], weight: &[f32], best_idx: &mut [u32]) {
        let ng = 32 / G;
        if d == 0.0 {
            best_idx[..ng].fill(0);
            return;
        }
        let id = 1.0 / d;
        for l in 0..ng {
            let mut vx = [0.0f32; G];
            let mut vw = [0.0f32; G];
            for k in 0..G {
                vx[k] = id * xb[G * l + k];
                vw[k] = weight[G * l + k];
            }
            let jc = self.cluster_of(&vx);
            let points = &self.in_cluster[jc];
            let values = if points.is_empty() {
                &self.values[..]
            } else {
                &self.c_values[jc][..]
            };
            let best = lane_search::<G>(values, &vx, &vw);
            best_idx[l] = if points.is_empty() {
                best as u32
            } else {
                points[best]
            };
        }
    }

    /// Port of find_best_scale: least squares scale and its score for the chosen entries.
    pub fn find_best_scale(&self, xb: &[f32], weight: &[f32], best_idx: &[u32]) -> (f32, f32) {
        let mut vqx = [0.0f32; 8];
        let mut vq2 = [0.0f32; 8];
        for l in (0..32).step_by(8) {
            for k in 0..8 {
                let pos = l + k;
                let q = self.values[G * best_idx[pos / G] as usize + pos % G];
                let qw = q * weight[pos];
                vqx[k] = qw.mul_add(xb[pos], vqx[k]);
                vq2[k] = qw.mul_add(q, vq2[k]);
            }
        }
        let sumqx = hsum8(&vqx);
        let sumq2 = hsum8(&vq2);
        if sumq2 > 0.0 {
            (sumqx / sumq2, sumqx * sumqx / sumq2)
        } else {
            (0.0, 0.0)
        }
    }

    pub fn group(&self, idx: u32) -> &[f32] {
        &self.values[G * idx as usize..G * idx as usize + G]
    }
}

/// _mm256 horizontal sum order: ((v0+v4)+(v2+v6))+((v1+v5)+(v3+v7))
fn hsum8(v: &[f32; 8]) -> f32 {
    ((v[0] + v[4]) + (v[2] + v[6])) + ((v[1] + v[5]) + (v[3] + v[7]))
}

fn bin5(x: f32) -> usize {
    if x < -48.0 {
        0
    } else if x < -16.0 {
        1
    } else if x < 16.0 {
        2
    } else if x < 48.0 {
        3
    } else {
        4
    }
}

/// Scores 8 candidates per step like the AVX2 code: one lane per candidate, each lane keeps its
/// first minimum, then the lanes are compared in order. Returns the position in `values`.
fn lane_search<const G: usize>(values: &[f32], vx: &[f32; G], vw: &[f32; G]) -> usize {
    // which candidate of a step of 8 sits in which lane
    const LANES8: [usize; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
    const LANES4: [usize; 8] = [0, 2, 4, 6, 1, 3, 5, 7];
    let lanes = if G == 8 { &LANES8 } else { &LANES4 };

    let npoint = values.len() / G;
    let mut lane_best = [f32::INFINITY; 8];
    let mut lane_idx = [usize::MAX; 8];
    for j in (0..npoint).step_by(8) {
        for i in 0..8 {
            let p = j + lanes[i];
            let q = &values[G * p..G * p + G];
            let score = if G == 8 {
                let mut s = [0.0f32; 8];
                for k in 0..8 {
                    let diff = q[k] - vx[k];
                    s[k] = vw[k] * (diff * diff);
                }
                hsum8(&s)
            } else {
                let mut s = [0.0f32; 4];
                for k in 0..4 {
                    let diff = q[k] - vx[k];
                    s[k] = vw[k] * (diff * diff);
                }
                (s[0] + s[2]) + (s[1] + s[3])
            };
            if score < lane_best[i] {
                lane_idx[i] = p;
            }
            lane_best[i] = if lane_best[i] < score {
                lane_best[i]
            } else {
                score
            };
        }
    }
    let mut best = f32::INFINITY;
    let mut jbest = 0;
    for i in 0..8 {
        if lane_best[i] < best {
            best = lane_best[i];
            jbest = lane_idx[i];
        }
    }
    jbest
}

/// Port of cluster_points for the two configurations in use: 256 clusters over 8 dims split at
/// the per-dim midpoint (median for the abs codebook), 625 clusters over 4 dims with fixed bins.
fn cluster_points<const G: usize>(
    points: &[f32],
    ncluster: usize,
    is_abs: bool,
    mid: &mut [f32; 8],
) -> Vec<f32> {
    let npoint = points.len() / G;
    let mut lo = [f32::INFINITY; G];
    let mut hi = [f32::NEG_INFINITY; G];
    for i in 0..npoint {
        for k in 0..G {
            let v = points[i * G + k];
            if v < lo[k] {
                lo[k] = v;
            }
            if hi[k] < v {
                hi[k] = v;
            }
        }
    }
    if is_abs {
        let mut p = vec![0i32; npoint];
        for idim in 0..G {
            for ip in 0..npoint {
                p[ip] = points[ip * G + idim] as i32;
            }
            p.sort_unstable();
            mid[idim] = if npoint % 2 == 0 {
                0.5 * (p[npoint / 2] + p[npoint / 2 - 1]) as f32
            } else {
                p[npoint / 2] as f32
            };
        }
    } else {
        for k in 0..G {
            mid[k] = 0.5 * (lo[k] + hi[k]);
        }
    }

    let mut sump = vec![0.0f32; ncluster * G];
    let mut counts = vec![0u32; ncluster];
    for ip in 0..npoint {
        let vp = &points[G * ip..G * ip + G];
        let u = if G == 8 {
            let mut u = 0usize;
            for k in 0..8 {
                if vp[k] > mid[k] {
                    u |= 1 << k;
                }
            }
            u
        } else {
            let mut u = 0usize;
            let mut s = 1usize;
            for k in 0..G {
                u += s * bin5(vp[k]);
                s *= 5;
            }
            u
        };
        counts[u] += 1;
        for k in 0..G {
            sump[G * u + k] += vp[k];
        }
    }
    // Empty clusters keep a zero centroid, same as upstream (IQ4_KT has one)
    let mut result = vec![0.0f32; ncluster * G];
    for ic in 0..ncluster {
        if counts[ic] > 0 {
            for k in 0..G {
                result[ic * G + k] = sump[ic * G + k] / counts[ic] as f32;
            }
        }
    }
    result
}

/// Port of finalize_clusters: every point joins its `nn` nearest clusters, then each cluster
/// is topped up to a multiple of 8 with the closest points that aren't in it yet.
fn finalize_clusters<const G: usize>(
    nn: usize,
    values: &[f32],
    clusters: &[f32],
) -> (Vec<Vec<u32>>, Vec<Vec<f32>>) {
    let ncluster = clusters.len() / G;
    let npoint = values.len() / G;
    let mut in_cluster: Vec<Vec<u32>> = vec![Vec::new(); ncluster];
    let mut best = vec![0.0f32; nn];
    let mut ibest = vec![0u32; nn];

    let dist2 = |p: usize, c: usize| -> f32 {
        let mut d2 = 0.0f32;
        for k in 0..G {
            let d = values[G * p + k] - clusters[G * c + k];
            d2 += d * d;
        }
        d2
    };

    for ip in 0..npoint {
        best.fill(f32::INFINITY);
        for ic in 0..ncluster {
            let d2 = dist2(ip, ic);
            // best is sorted, so nothing can be inserted past the last entry
            if !(d2 < best[nn - 1]) {
                continue;
            }
            for j in 0..nn {
                if d2 < best[j] {
                    for k in (j + 1..nn).rev() {
                        best[k] = best[k - 1];
                        ibest[k] = ibest[k - 1];
                    }
                    best[j] = d2;
                    ibest[j] = ic as u32;
                    break;
                }
            }
        }
        for j in 0..nn {
            in_cluster[ibest[j] as usize].push(ip as u32);
        }
    }

    // Upstream sorts all points outside the cluster by (distance, index) and takes the first
    // few, keeping only the nadd smallest gives the same picks.
    let mut member = vec![false; npoint];
    for ic in 0..ncluster {
        let len = in_cluster[ic].len();
        if len % 8 == 0 {
            continue;
        }
        let nadd = 8 * ((len + 7) / 8) - len;
        for &p in &in_cluster[ic] {
            member[p as usize] = true;
        }
        let mut extra: Vec<(f32, u32)> = Vec::with_capacity(nadd + 1);
        for ip in 0..npoint {
            if member[ip] {
                continue;
            }
            let d2 = dist2(ip, ic);
            if extra.len() == nadd && !(d2 < extra[nadd - 1].0) {
                continue;
            }
            let pos = extra.partition_point(|&(d, i)| d < d2 || (d == d2 && i < ip as u32));
            extra.insert(pos, (d2, ip as u32));
            extra.truncate(nadd);
        }
        for &p in &in_cluster[ic] {
            member[p as usize] = false;
        }
        for &(_, p) in &extra {
            in_cluster[ic].push(p);
        }
    }

    let c_values = in_cluster
        .iter()
        .map(|pts| {
            let mut v = Vec::with_capacity(pts.len() * G);
            for &p in pts {
                v.extend_from_slice(&values[G * p as usize..G * p as usize + G]);
            }
            v
        })
        .collect();
    (in_cluster, c_values)
}

pub fn iq1kt() -> &'static KtCodebook<8> {
    static C: OnceLock<KtCodebook<8>> = OnceLock::new();
    C.get_or_init(|| KtCodebook::new(13, false, 4096, 256, 32))
}

pub fn iq2kt() -> &'static KtCodebook<8> {
    static C: OnceLock<KtCodebook<8>> = OnceLock::new();
    C.get_or_init(|| KtCodebook::new(16, false, 4096, 256, 8))
}

pub fn iq3kt() -> &'static KtCodebook<8> {
    static C: OnceLock<KtCodebook<8>> = OnceLock::new();
    C.get_or_init(|| KtCodebook::new(16, true, 4096, 256, 8))
}

/// IQ4_KT has two banks, the second one seeds with an extra 32768.
pub fn iq4kt(bank: usize) -> &'static KtCodebook<4> {
    static C0: OnceLock<KtCodebook<4>> = OnceLock::new();
    static C1: OnceLock<KtCodebook<4>> = OnceLock::new();
    if bank == 0 {
        C0.get_or_init(|| KtCodebook::new(15, false, 4096, 625, 6))
    } else {
        C1.get_or_init(|| KtCodebook::new(15, false, 4096 + 32768, 625, 6))
    }
}
