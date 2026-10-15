//! Bitshift trellis from lib/codebook/bitshift.py: the Viterbi search, the two pass tail-biting
//! and the packed bitstream.

use super::codes::{self, Code};
use std::cell::RefCell;
use std::rc::Rc;

pub struct Codebook {
    pub code: Code,
    /// State bits.
    pub l: u32,
    /// Bits per weight.
    pub k: u32,
    /// Weights per step.
    pub v: usize,
    /// What every state decodes to, lut[v][state] like the torch buffer.
    pub lut: Vec<Vec<f32>>,
    /// RMS over the whole lut, the scale is set against it.
    pub rms: f32,
}

impl Codebook {
    fn new(code: Code, l: u32, k: u32) -> Codebook {
        let v = code.v();
        let n = 1usize << l;
        let mut lut = vec![vec![0.0f32; n]; v];
        for s in 0..n {
            match code {
                Code::OneMad => lut[0][s] = codes::one_mad(s as u32),
                Code::ThreeInst => lut[0][s] = codes::three_inst(s as u32),
                Code::Hyb => {
                    let [a, b] = codes::hyb(s as u32);
                    lut[0][s] = a;
                    lut[1][s] = b;
                }
            }
        }
        let sq: f64 = lut.iter().flatten().map(|&x| x as f64 * x as f64).sum();
        let rms = (sq / (n * v) as f64).sqrt() as f32;
        Codebook {
            code,
            l,
            k,
            v,
            lut,
            rms,
        }
    }

    /// Bits a step adds, K * V.
    pub fn kv(&self) -> u32 {
        self.k * self.v as u32
    }

    pub fn value(&self, state: u32, v: usize) -> f32 {
        self.lut[v][state as usize]
    }
}

thread_local! {
    static CACHE: RefCell<Option<Rc<Codebook>>> = const { RefCell::new(None) };
}

pub fn codebook(code: Code, l: u32, k: u32) -> Rc<Codebook> {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        match c.as_ref() {
            Some(cb) if cb.code == code && cb.l == l && cb.k == k => cb.clone(),
            _ => {
                let cb = Rc::new(Codebook::new(code, l, k));
                *c = Some(cb.clone());
                cb
            }
        }
    })
}

/// Squared error of every state against one step of x (summed over V like state_err), plus the
/// best cost of the group the state belongs to. best is one value per run of nt states.
fn step_cost(cb: &Codebook, x: &[f32], best: Option<&[f32]>, nt: usize, out: &mut [f32]) {
    let zero = [0.0f32];
    let add = |j: usize| best.map_or(zero[0], |b| b[j]);
    if cb.v == 1 {
        let x0 = x[0];
        for (j, (o, r)) in out
            .chunks_exact_mut(nt)
            .zip(cb.lut[0].chunks_exact(nt))
            .enumerate()
        {
            let b = add(j);
            for (o, &r) in o.iter_mut().zip(r) {
                let d = r - x0;
                *o = d * d + b;
            }
        }
    } else {
        let (x0, x1) = (x[0], x[1]);
        let groups = out
            .chunks_exact_mut(nt)
            .zip(cb.lut[0].chunks_exact(nt))
            .zip(cb.lut[1].chunks_exact(nt));
        for (j, ((o, r0), r1)) in groups.enumerate() {
            let b = add(j);
            for ((o, &r0), &r1) in o.iter_mut().zip(r0).zip(r1) {
                let d0 = r0 - x0;
                let d1 = r1 - x1;
                *o = (d0 * d0 + d1 * d1) + b;
            }
        }
    }
}

/// Port of bitshift_codebook.viterbi for one sequence. With an overlap the first state has to
/// start with those L - KV bits and the last one end with them.
pub fn viterbi(cb: &Codebook, x: &[f32], overlap: Option<u32>) -> Vec<u32> {
    let kv = cb.kv() as usize;
    let ns = 1usize << cb.l;
    let ng = 1usize << (cb.l as usize - kv);
    let nt = 1usize << kv;
    let steps = x.len() / cb.v;

    let mut cost = vec![0.0f32; ns];
    let mut best = vec![0.0f32; ng];
    // predecessor choice per step and group, the state is group + choice * ng
    let mut from = vec![0u8; steps * ng];

    step_cost(cb, &x[..cb.v], None, nt, &mut cost);
    if let Some(o) = overlap {
        for (s, c) in cost.iter_mut().enumerate() {
            if (s >> kv) as u32 != o {
                *c = f32::INFINITY;
            }
        }
    }

    for i in 1..steps {
        // the states that shift into group j are j + t * ng, first minimum wins like torch.min
        best.copy_from_slice(&cost[..ng]);
        for t in 1..nt {
            for (b, &c) in best.iter_mut().zip(&cost[t * ng..(t + 1) * ng]) {
                *b = if c < *b { c } else { *b };
            }
        }
        // the first candidate equal to the minimum, two plain passes vectorize where one doesn't
        let f = &mut from[i * ng..(i + 1) * ng];
        for t in (0..nt).rev() {
            for ((f, &b), &c) in f.iter_mut().zip(&best).zip(&cost[t * ng..(t + 1) * ng]) {
                *f = if c == b { t as u8 } else { *f };
            }
        }
        step_cost(cb, &x[i * cb.v..(i + 1) * cb.v], Some(&best), nt, &mut cost);
    }

    if let Some(o) = overlap {
        for (s, c) in cost.iter_mut().enumerate() {
            if (s & (ng - 1)) as u32 != o {
                *c = f32::INFINITY;
            }
        }
    }
    let mut last = 0;
    for s in 1..ns {
        if cost[s] < cost[last] {
            last = s;
        }
    }
    let mut states = vec![0u32; steps];
    states[steps - 1] = last as u32;
    for i in (1..steps).rev() {
        let j = (states[i] >> kv) as usize;
        states[i - 1] = (j + from[i * ng + j] as usize * ng) as u32;
    }
    states
}

/// Port of bitshift_codebook.quantize for one sequence (Algorithm 4 of the paper): Viterbi on
/// the sequence rolled by half, the state where the original starts gives the wrap bits, then
/// Viterbi again with start and end tied to them.
pub fn quantize_seq(cb: &Codebook, x: &[f32]) -> Vec<u32> {
    let t = x.len();
    let half = t / (2 * cb.v);
    let shift = half * cb.v;
    let rolled: Vec<f32> = (0..t).map(|i| x[(i + t - shift) % t]).collect();
    let first = viterbi(cb, &rolled, None);
    let overlap = first[half] >> cb.kv();
    viterbi(cb, x, Some(overlap))
}

/// pack_trellis: the first state in full, then the new KV bits of every step, minus the L - KV
/// bits that wrap around to the start. T * K bits, MSB first.
pub fn pack(cb: &Codebook, states: &[u32]) -> Vec<u8> {
    let kv = cb.kv();
    let nbits = states.len() * kv as usize;
    let mut out = vec![0u8; (nbits + 7) / 8];
    let mut pos = 0;
    let mut push = |v: u32, n: u32| {
        for b in (0..n).rev() {
            if pos < nbits && (v >> b) & 1 == 1 {
                out[pos / 8] |= 0x80 >> (pos % 8);
            }
            pos += 1;
        }
    };
    push(states[0], cb.l);
    for &s in &states[1..] {
        push(s & ((1 << kv) - 1), kv);
    }
    out
}

pub fn bit(stream: &[u8], nbits: usize, pos: usize) -> u32 {
    let p = pos % nbits;
    ((stream[p / 8] >> (7 - p % 8)) & 1) as u32
}

/// State of step i: the L bits of the circular stream starting at i * KV.
pub fn state_at(cb: &Codebook, stream: &[u8], steps: usize, i: usize) -> u32 {
    let nbits = steps * cb.kv() as usize;
    let start = i * cb.kv() as usize;
    (0..cb.l as usize).fold(0, |s, b| (s << 1) | bit(stream, nbits, start + b))
}

/// unpack_trellis.
pub fn unpack(cb: &Codebook, stream: &[u8], steps: usize) -> Vec<u32> {
    (0..steps).map(|i| state_at(cb, stream, steps, i)).collect()
}
