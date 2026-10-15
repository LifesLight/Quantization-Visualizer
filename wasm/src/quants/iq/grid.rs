//! Lattice codebooks for the IQ types and the neighbour tables llama.cpp builds around them.
//! Port of iq2xs_init_impl / iq3xs_init_impl from ggml-quants.c.

use std::sync::OnceLock;

use crate::quants::iq::lut;

/// One codebook of lattice points plus the lookups used while quantizing.
/// Points keep the odd coordinates (2*l + 1) the C code works with.
pub struct IqGrid<const N: usize> {
    pub points: Vec<[i8; N]>,
    /// Packed per-coordinate levels -> grid index, -1 if that point is not in the codebook.
    pub map: Vec<i32>,
    /// For off-grid points: every grid entry in the `nwant` closest distance shells,
    /// ordered by (distance, index). Empty for points that are on the grid.
    pub neighbours: Vec<Vec<u16>>,
}

impl<const N: usize> IqGrid<N> {
    fn build(kgrid: &[u16], bits: usize, max_level: usize, nwant: usize) -> Self {
        let mask = (1usize << bits) - 1;

        let points: Vec<[i8; N]> = kgrid
            .iter()
            .map(|&v| {
                let mut p = [0i8; N];
                for i in 0..N {
                    p[i] = (2 * ((v as usize >> (bits * i)) & mask) + 1) as i8;
                }
                p
            })
            .collect();

        let map_size = 1usize << (bits * N);
        let mut map = vec![-1i32; map_size];
        for (i, p) in points.iter().enumerate() {
            let mut index = 0usize;
            for k in 0..N {
                index |= (((p[k] - 1) / 2) as usize) << (bits * k);
            }
            map[index] = i as i32;
        }

        // The quantizers clamp every level to max_level, so only those packed
        // indices can ever be looked up. The C version also fills the rest, but
        // the lists of reachable points come out identical.
        let mut neighbours = vec![Vec::new(); map_size];
        let mut dist2 = vec![0i32; points.len()];
        let mut shells: Vec<i32> = Vec::with_capacity(nwant);

        for u in 0..map_size {
            if map[u] >= 0 {
                continue;
            }
            let mut pos = [0i32; N];
            let mut reachable = true;
            for k in 0..N {
                let l = (u >> (bits * k)) & mask;
                if l > max_level {
                    reachable = false;
                    break;
                }
                pos[k] = 2 * l as i32 + 1;
            }
            if !reachable {
                continue;
            }

            for (j, p) in points.iter().enumerate() {
                let mut d2 = 0;
                for k in 0..N {
                    let d = p[k] as i32 - pos[k];
                    d2 += d * d;
                }
                dist2[j] = d2;
            }

            // Same as qsort by (d2, j) and stopping after nwant distinct distances
            shells.clear();
            for &d2 in &dist2 {
                if shells.contains(&d2) {
                    continue;
                }
                if shells.len() < nwant {
                    shells.push(d2);
                    shells.sort_unstable();
                } else if d2 < shells[nwant - 1] {
                    shells[nwant - 1] = d2;
                    shells.sort_unstable();
                }
            }
            let list = &mut neighbours[u];
            for &s in &shells {
                for (j, &d2) in dist2.iter().enumerate() {
                    if d2 == s {
                        list.push(j as u16);
                    }
                }
            }
        }

        IqGrid {
            points,
            map,
            neighbours,
        }
    }
}

pub fn iq2xxs() -> &'static IqGrid<8> {
    static G: OnceLock<IqGrid<8>> = OnceLock::new();
    G.get_or_init(|| IqGrid::build(&lut::KGRID_2BIT_256, 2, 2, 2))
}

pub fn iq2xs() -> &'static IqGrid<8> {
    static G: OnceLock<IqGrid<8>> = OnceLock::new();
    G.get_or_init(|| IqGrid::build(&lut::KGRID_2BIT_512, 2, 2, 2))
}

pub fn iq2s() -> &'static IqGrid<8> {
    static G: OnceLock<IqGrid<8>> = OnceLock::new();
    G.get_or_init(|| IqGrid::build(&lut::KGRID_2BIT_1024, 2, 2, 1))
}

/// Shared by IQ1_S and IQ1_M. Levels 0/1/2 stand for -1/0/+1.
pub fn iq1() -> &'static IqGrid<8> {
    static G: OnceLock<IqGrid<8>> = OnceLock::new();
    G.get_or_init(|| IqGrid::build(&lut::KGRID_1BIT_2048, 2, 2, 3))
}

pub fn iq3xxs() -> &'static IqGrid<4> {
    static G: OnceLock<IqGrid<4>> = OnceLock::new();
    G.get_or_init(|| IqGrid::build(&lut::KGRID_3BIT_256, 3, 7, 2))
}

pub fn iq3s() -> &'static IqGrid<4> {
    static G: OnceLock<IqGrid<4>> = OnceLock::new();
    G.get_or_init(|| IqGrid::build(&lut::KGRID_3BIT_512, 3, 7, 3))
}
