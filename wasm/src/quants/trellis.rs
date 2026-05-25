use crate::math_utils::*;
use crate::quants::{
    ImportanceResult, InspectorData, QuantMeta, QuantizeOutput, Settings, TrellisBlockMeta,
};
use serde::Serialize;

#[derive(Clone, Copy)]
pub struct Edge {
    pub from: usize,
    pub sub: usize,
}

/// Generates valid transition edges mapping Trellis Coded Quantization (TCQ) state branches.
pub fn get_transitions(states: usize) -> Option<Vec<Vec<Edge>>> {
    if states == 1 {
        return None;
    }
    let k = (states as f64).log2() as usize;
    let (poly_lsb, poly_msb) = match states {
        4 => (0x02, 0x01),
        8 => (0x02, 0x05),
        16 => (0x04, 0x0B),
        64 => (0x10, 0x2D),
        256 => (0x40, 0x95),
        _ => return None,
    };
    let mut transitions = vec![vec![]; states];
    for state in 0..states {
        for input in 0..=1 {
            let next_state = (state >> 1) | (input << (k - 1));
            let mut sub_lsb = 0;
            let mut x = state & poly_lsb;
            while x > 0 {
                sub_lsb += x & 1;
                x >>= 1;
            }
            sub_lsb &= 1;
            let mut x2 = state & poly_msb;
            let mut sub_msb_pop = 0;
            while x2 > 0 {
                sub_msb_pop += x2 & 1;
                x2 >>= 1;
            }
            let sub = ((input ^ (sub_msb_pop & 1)) << 1) | sub_lsb;
            transitions[next_state].push(Edge {
                from: state,
                sub: sub as usize,
            });
        }
    }
    Some(transitions)
}

/// Helper function to locate the closest codeword index within a given state subset.
pub fn find_closest_idx(target: f64, subset_idxs: &[usize], levels: &[f64]) -> (f64, usize) {
    let mut best_idx = subset_idxs[0];
    let mut best_dist = (target - levels[best_idx]).abs();
    for &idx in subset_idxs.iter().skip(1) {
        let d = (target - levels[idx]).abs();
        if d < best_dist {
            best_dist = d;
            best_idx = idx;
        }
    }
    (levels[best_idx], best_idx)
}

#[derive(Serialize, Clone, Copy)]
#[allow(non_snake_case)]
pub struct Candidate {
    pub prevState: isize,
    pub subset: usize,
    pub cbIdx: usize,
    pub cbVal: f64,
    pub dist: f64,
    pub cost: f64,
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum NextState {
    State(usize),
    End(String),
}

#[derive(Serialize, Clone)]
#[allow(non_snake_case)]
pub struct StepInfo {
    pub state: usize,
    pub subset: usize,
    pub cbIdx: usize,
    pub cwVal: f64,
    pub prevState: isize,
    pub nextState: NextState,
    pub input: f64,
    pub error: f64,
    pub cost: f64,
    pub stateCosts: Vec<f64>,
    pub candidates: Vec<Candidate>,
}

pub struct TrellisBlockResult {
    pub chunk_q: Vec<f64>,
    pub path_data: Vec<StepInfo>,
    pub cost: f64,
}

#[derive(Clone, Copy)]
struct StateInfo {
    prev_state: isize,
    cb_val: f64,
    cb_idx: usize,
    subset: usize,
    cost: f64,
}

/// Optimizes scaling factor iteratively mimicking Golden Section Search logic.
pub fn trellis_opt_scale(
    samples: &[f64],
    base_levels: &[f64],
    states: usize,
    mut a: f64,
    mut b: f64,
    iters: usize,
    block_size: usize,
    transitions: &[Vec<Edge>],
) -> f64 {
    let mut scaled_buf = vec![0.0; base_levels.len()];
    let mut subset_indices = vec![vec![]; 4];
    for i in 0..base_levels.len() {
        subset_indices[i % 4].push(i);
    }

    let mut prev_costs = vec![f64::INFINITY; states];
    let mut next_costs = vec![0.0; states];

    let mut eval = |scale: f64, scaled_buf: &mut [f64]| -> f64 {
        for i in 0..base_levels.len() {
            scaled_buf[i] = base_levels[i] * scale;
        }
        let b_size = if block_size == 0 {
            samples.len()
        } else {
            block_size
        };
        let mut total_cost = 0.0;

        for chunk in samples.chunks(b_size) {
            if states == 1 {
                for &x in chunk {
                    let mut best_dist = f64::INFINITY;
                    for &l in scaled_buf.iter() {
                        let d = (x - l).abs();
                        if d < best_dist {
                            best_dist = d;
                        }
                    }
                    total_cost += best_dist * best_dist;
                }
            } else {
                prev_costs.fill(f64::INFINITY);
                prev_costs[0] = 0.0;

                for &x in chunk {
                    next_costs.fill(f64::INFINITY);
                    for curr_state in 0..states {
                        let mut min_cost = f64::INFINITY;
                        for edge in &transitions[curr_state] {
                            let prev_cost = prev_costs[edge.from];
                            if prev_cost.is_infinite() {
                                continue;
                            }
                            let (val, _) =
                                find_closest_idx(x, &subset_indices[edge.sub], scaled_buf);
                            let err = x - val;
                            let total = prev_cost + err * err;
                            if total < min_cost {
                                min_cost = total;
                            }
                        }
                        next_costs[curr_state] = min_cost;
                    }
                    std::mem::swap(&mut prev_costs, &mut next_costs);
                }

                let mut min_final = f64::INFINITY;
                for &c in &prev_costs {
                    if c < min_final {
                        min_final = c;
                    }
                }
                total_cost += min_final;
            }
        }
        total_cost
    };

    let resphi = 2.0 - 1.6180339887;
    let mut c = a + resphi * (b - a);
    let mut d = b - resphi * (b - a);
    let mut fc = eval(c, &mut scaled_buf);
    let mut fd = eval(d, &mut scaled_buf);

    for _ in 0..iters {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = a + resphi * (b - a);
            fc = eval(c, &mut scaled_buf);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = b - resphi * (b - a);
            fd = eval(d, &mut scaled_buf);
        }
    }
    if fc < fd {
        c
    } else {
        d
    }
}

/// Dispatches Viterbi-style dynamic programming decoding to map elements to the graph.
pub fn trellis_quantize_block(
    samples: &[f64],
    base_levels: &[f64],
    states: usize,
    scale: f64,
    keep_candidates: bool,
    transitions: &[Vec<Edge>],
) -> TrellisBlockResult {
    let mut scaled_buf = vec![0.0; base_levels.len()];
    for i in 0..base_levels.len() {
        scaled_buf[i] = base_levels[i] * scale;
    }

    let mut subset_indices = vec![vec![]; 4];
    for i in 0..base_levels.len() {
        subset_indices[i % 4].push(i);
    }

    let all_idxs: Vec<usize> = (0..base_levels.len()).collect();
    let mut chunk_q = vec![0.0; samples.len()];
    let mut path_data = vec![];
    let min_final_cost;

    if states == 1 {
        let mut cost_total = 0.0;
        for (t, &x) in samples.iter().enumerate() {
            let (val, idx) = find_closest_idx(x, &all_idxs, &scaled_buf);
            chunk_q[t] = val;
            let err = x - val;
            let cost = err * err;
            cost_total += cost;
            if keep_candidates {
                path_data.push(StepInfo {
                    state: 0,
                    subset: 0,
                    cbIdx: idx,
                    cwVal: val,
                    prevState: 0,
                    nextState: if t == samples.len() - 1 {
                        NextState::End("End".to_string())
                    } else {
                        NextState::State(0)
                    },
                    input: x,
                    error: err,
                    cost,
                    stateCosts: vec![cost],
                    candidates: vec![Candidate {
                        prevState: 0,
                        subset: 0,
                        cbIdx: idx,
                        cbVal: val,
                        dist: err.abs(),
                        cost,
                    }],
                });
            }
        }
        min_final_cost = cost_total;
    } else {
        let mut path_memory = vec![
            StateInfo {
                prev_state: -1,
                cb_val: 0.0,
                cb_idx: 0,
                subset: 0,
                cost: f64::INFINITY
            };
            samples.len() * states
        ];
        let mut step_costs = vec![];
        let mut all_candidates = vec![];

        if keep_candidates {
            step_costs = vec![vec![0.0; states]; samples.len()];
            all_candidates = vec![vec![]; samples.len() * states];
        }

        let mut prev_costs = vec![f64::INFINITY; states];
        let mut next_costs = vec![0.0; states];
        prev_costs[0] = 0.0;

        for (t, &x) in samples.iter().enumerate() {
            next_costs.fill(f64::INFINITY);
            let mem_offset = t * states;

            for curr_state in 0..states {
                let (mut best_prev, mut best_cb_val, mut best_cb_idx, mut best_sub, mut min_cost) =
                    (-1, 0.0, 0, 0, f64::INFINITY);
                let mut state_cands = vec![];

                for edge in &transitions[curr_state] {
                    let prev_cost = prev_costs[edge.from];
                    if prev_cost.is_infinite() {
                        continue;
                    }
                    let (val, idx) = find_closest_idx(x, &subset_indices[edge.sub], &scaled_buf);
                    let err = x - val;
                    let total = prev_cost + err * err;

                    if keep_candidates {
                        state_cands.push(Candidate {
                            prevState: edge.from as isize,
                            subset: edge.sub,
                            cbIdx: idx,
                            cbVal: val,
                            dist: err.abs(),
                            cost: total,
                        });
                    }

                    if total < min_cost {
                        min_cost = total;
                        best_prev = edge.from as isize;
                        best_cb_val = val;
                        best_cb_idx = idx;
                        best_sub = edge.sub;
                    }
                }

                next_costs[curr_state] = min_cost;
                path_memory[mem_offset + curr_state] = StateInfo {
                    prev_state: best_prev,
                    cb_val: best_cb_val,
                    cb_idx: best_cb_idx,
                    subset: best_sub,
                    cost: min_cost,
                };

                if keep_candidates {
                    all_candidates[mem_offset + curr_state] = state_cands;
                }
            }

            if keep_candidates {
                step_costs[t].copy_from_slice(&next_costs);
            }
            std::mem::swap(&mut prev_costs, &mut next_costs);
        }

        let (mut final_state, mut min_fc) = (0, f64::INFINITY);
        for s in 0..states {
            if prev_costs[s] < min_fc {
                min_fc = prev_costs[s];
                final_state = s;
            }
        }
        min_final_cost = min_fc;

        if keep_candidates {
            path_data.resize_with(samples.len(), || StepInfo {
                state: 0,
                subset: 0,
                cbIdx: 0,
                cwVal: 0.0,
                prevState: 0,
                nextState: NextState::End("End".to_string()),
                input: 0.0,
                error: 0.0,
                cost: 0.0,
                stateCosts: vec![],
                candidates: vec![],
            });
        }

        let mut curr_state = final_state;
        for t in (0..samples.len()).rev() {
            let st = &path_memory[t * states + curr_state];
            if st.prev_state == -1 {
                let (val, idx) = find_closest_idx(samples[t], &all_idxs, &scaled_buf);
                chunk_q[t] = val;
                if keep_candidates {
                    path_data[t] = StepInfo {
                        state: curr_state,
                        subset: 0,
                        cbIdx: idx,
                        cwVal: val,
                        prevState: -1,
                        nextState: if t == samples.len() - 1 {
                            NextState::End("End".to_string())
                        } else {
                            NextState::State(path_data[t + 1].state)
                        },
                        input: samples[t],
                        error: samples[t] - val,
                        cost: f64::INFINITY,
                        stateCosts: step_costs[t].clone(),
                        candidates: vec![],
                    };
                }
                curr_state = 0;
            } else {
                chunk_q[t] = st.cb_val;
                if keep_candidates {
                    path_data[t] = StepInfo {
                        state: curr_state,
                        subset: st.subset,
                        cbIdx: st.cb_idx,
                        cwVal: st.cb_val,
                        prevState: st.prev_state,
                        nextState: if t == samples.len() - 1 {
                            NextState::End("End".to_string())
                        } else {
                            NextState::State(path_data[t + 1].state)
                        },
                        input: samples[t],
                        error: samples[t] - st.cb_val,
                        cost: st.cost,
                        stateCosts: step_costs[t].clone(),
                        candidates: all_candidates[t * states + curr_state].clone(),
                    };
                }
                curr_state = st.prev_state as usize;
            }
        }
    }
    TrellisBlockResult {
        chunk_q,
        path_data,
        cost: min_final_cost,
    }
}

pub fn quantize(
    floats: &[f32],
    _importance: Option<&ImportanceResult>,
    settings: &Settings,
) -> QuantizeOutput {
    let (t_bits, t_bsize, states, cb_type, use_wht, is_global, iters, seed) = (
        settings.trellis_bits,
        settings.trellis_block_size.max(1),
        if vec![1, 4, 8, 16, 64, 256].contains(&settings.trellis_states) {
            settings.trellis_states
        } else {
            1
        },
        &settings.trellis_cb_type,
        settings.trellis_use_wht,
        settings.trellis_use_wht && settings.trellis_wht_scope == "global",
        settings.trellis_opt_iters,
        settings.trellis_sign_seed,
    );

    let base_levels: Vec<f64> = if cb_type == "uniform" {
        (0..(1 << (t_bits + (if states == 1 { 0 } else { 1 }))))
            .map(|i| {
                -3.0 + (6.0 * (i as f64 + 0.5))
                    / (1 << (t_bits + (if states == 1 { 0 } else { 1 }))) as f64
            })
            .collect()
    } else {
        get_lloyd_max_centroids(if states == 1 { t_bits } else { t_bits + 1 }, cb_type)
            .iter()
            .map(|&x| x as f64)
            .collect()
    };

    let transitions_opt = get_transitions(states);
    let dummy_trans = vec![];
    let transitions = transitions_opt.as_ref().unwrap_or(&dummy_trans);

    let mut process_floats = floats.to_vec();
    let mut global_pad_len = floats.len();
    let mut global_rms = 1e-5;

    let mut total_bits = 0.0;

    if is_global {
        global_pad_len = 1;
        while global_pad_len < floats.len() {
            global_pad_len <<= 1;
        }

        total_bits += 16.0; // One FP16 global scale
        total_bits += (global_pad_len * t_bits as usize) as f64; // Cost for all padded values

        let mut padded = vec![0.0; global_pad_len];
        padded[..floats.len()].copy_from_slice(floats);
        for j in 0..global_pad_len {
            padded[j] *= get_sign_flip(j, seed);
        }
        fwht_f32_inplace(&mut padded);
        process_floats = padded;
        global_rms = fp16(
            if process_floats.iter().map(|&x| x * x).sum::<f32>() == 0.0 {
                1e-5
            } else {
                (process_floats.iter().map(|&x| x * x).sum::<f32>() / global_pad_len as f32).sqrt()
            },
        );
        if iters > 0 {
            global_rms = fp16(trellis_opt_scale(
                &process_floats.iter().map(|&x| x as f64).collect::<Vec<_>>(),
                &base_levels,
                states,
                global_rms as f64 * 0.1,
                global_rms as f64 * 2.5,
                iters,
                t_bsize,
                transitions,
            ) as f32);
        }
    }

    let len = if is_global {
        global_pad_len
    } else {
        floats.len()
    };
    let mut q_process_out = vec![0.0; len];
    let mut t_floats_out = if use_wht {
        Some(vec![0.0; floats.len()])
    } else {
        None
    };
    let mut t_q_floats_out = if use_wht {
        Some(vec![0.0; floats.len()])
    } else {
        None
    };
    let mut blocks = Vec::new();

    let max_pad_len = if use_wht && !is_global {
        t_bsize.next_power_of_two()
    } else {
        t_bsize
    };
    let mut chunk_wht_buf = vec![0.0; max_pad_len];
    let mut chunk_out_buf = vec![0.0; max_pad_len];

    for i in (0..len).step_by(t_bsize) {
        let actual_len = (len - i).min(t_bsize);
        let chunk = &process_floats[i..i + actual_len];

        let mut pad_len = actual_len;
        if use_wht && !is_global {
            pad_len = 1;
            while pad_len < actual_len {
                pad_len <<= 1;
            }
            chunk_wht_buf[..actual_len].copy_from_slice(chunk);
            chunk_wht_buf[actual_len..pad_len].fill(0.0);
            for j in 0..pad_len {
                chunk_wht_buf[j] *= get_sign_flip(i + j, seed);
            }
            fwht_f32_inplace(&mut chunk_wht_buf[..pad_len]);
        } else {
            chunk_wht_buf[..actual_len].copy_from_slice(chunk);
        }

        if !is_global {
            total_bits += 16.0; // One FP16 optimal scale per block
            total_bits += (pad_len * t_bits as usize) as f64; // Cost for padded block elements
        }

        let chunk_w = &chunk_wht_buf[..pad_len];

        let mut opt_scale = global_rms;
        if !is_global {
            opt_scale = fp16(if chunk.iter().map(|&x| x * x).sum::<f32>() == 0.0 {
                1e-5
            } else {
                (chunk.iter().map(|&x| x * x).sum::<f32>() / actual_len as f32).sqrt()
            });
            if iters > 0 {
                opt_scale = fp16(trellis_opt_scale(
                    &chunk_w.iter().map(|&x| x as f64).collect::<Vec<_>>(),
                    &base_levels,
                    states,
                    opt_scale as f64 * 0.1,
                    opt_scale as f64 * 2.5,
                    iters,
                    0,
                    transitions,
                ) as f32);
            }
        }

        let res = trellis_quantize_block(
            &chunk_w.iter().map(|&x| x as f64).collect::<Vec<_>>(),
            &base_levels,
            states,
            opt_scale as f64,
            false,
            transitions,
        );

        if use_wht && !is_global {
            for t in 0..actual_len {
                if i + t < floats.len() {
                    t_floats_out.as_mut().unwrap()[i + t] = chunk_w[t];
                    t_q_floats_out.as_mut().unwrap()[i + t] = res.chunk_q[t] as f32;
                }
            }
        }

        for t in 0..pad_len {
            chunk_out_buf[t] = res.chunk_q[t] as f32;
        }

        if use_wht && !is_global {
            fwht_f32_inplace(&mut chunk_out_buf[..pad_len]);
            for j in 0..pad_len {
                chunk_out_buf[j] *= get_sign_flip(i + j, seed);
            }
        }

        for t in 0..actual_len {
            q_process_out[i + t] = chunk_out_buf[t];
        }

        if i < floats.len() {
            let (mse, mae) = get_err_stats(chunk, &chunk_out_buf[..actual_len]);
            blocks.push(TrellisBlockMeta {
                idx: i / t_bsize,
                size: actual_len,
                scale: opt_scale,
                mse,
                mae,
                chunk_w: chunk_w.iter().map(|&x| x as f64).collect(),
            });
        }
    }

    let mut q_floats = vec![0.0; floats.len()];
    if is_global {
        let mut inv = vec![0.0; global_pad_len];
        inv.copy_from_slice(&q_process_out);
        fwht_f32_inplace(&mut inv);
        for j in 0..global_pad_len {
            inv[j] *= get_sign_flip(j, seed);
        }
        for j in 0..floats.len() {
            q_floats[j] = inv[j];
            t_floats_out.as_mut().unwrap()[j] = process_floats[j];
            t_q_floats_out.as_mut().unwrap()[j] = q_process_out[j];
        }
    } else {
        q_floats.copy_from_slice(&q_process_out[..floats.len()]);
    }

    QuantizeOutput {
        q_floats, t_floats: t_floats_out, t_q_floats: t_q_floats_out, bpw: (total_bits / floats.len() as f64) as f32,
        formula_html: format!("<span>Weight = {}[ ( {} &times; <span class=\"eq-pill\">{}<span class=\"bits\">16b</span></span> ) ]</span><br><span style=\"color:var(--text-muted);font-size:0.8rem;\">Block size {}. {}</span>", if use_wht { "<span class=\"eq-pill\" title=\"Diagonal Random Sign Array\">D</span> &times; <span class=\"eq-pill\" title=\"Orthogonal Fast Walsh-Hadamard Transform\">FWHT</span> &times; " } else { "" }, if states > 1 { format!("<span class=\"eq-pill\" title=\"Trellis Coded Quantization via Viterbi\">TCQ_Path<span class=\"bits\">{}b</span></span>", t_bits) } else { format!("<span class=\"eq-pill\">Codeword<span class=\"bits\">{}b</span></span>", t_bits) }, if is_global { "Global_Scale" } else { "RMS_Scale" }, t_bsize, if use_wht { if is_global { format!("Global QuIP# SRHT applied. All weights share a single FP16 Global Scale.{}", if floats.len().count_ones() != 1 { " <span style=\"color:#f0b429\"><br>Weight count not a power of two: zero-padding inflates effective BPW.</span>" } else { "" }) } else { format!("Local Block SRHT applied. Every {} weights share one FP16 Optimal Scale.", t_bsize) } } else { format!("Every {} weights share one FP16 Optimal Scale.", t_bsize) }),
        block_size: t_bsize, super_block_size: 0, meta: QuantMeta::Trellis(blocks),
    }
}

pub fn format_inspector(
    idx: usize,
    _active: &[f32],
    out: &QuantizeOutput,
    settings: &Settings,
) -> InspectorData {
    let QuantMeta::Trellis(blocks) = &out.meta else {
        return InspectorData::default();
    };
    let t_bsize = settings.trellis_block_size.max(1);
    let b_idx = idx / t_bsize;
    let local_idx = idx % t_bsize;
    let mut data = InspectorData::default();

    if let Some(bm) = blocks.get(b_idx) {
        data.block_idx = Some(bm.idx);
        data.mse = Some(bm.mse);
        data.mae = Some(bm.mae);
        data.scale = Some(bm.scale);
        if settings.trellis_use_wht && settings.trellis_wht_scope == "global" {
            data.global_scale = Some(bm.scale);
        }

        let states = if vec![1, 4, 8, 16, 64, 256].contains(&settings.trellis_states) {
            settings.trellis_states
        } else {
            1
        };
        let cb_type = &settings.trellis_cb_type;
        let t_bits = settings.trellis_bits;
        let base_levels: Vec<f64> = if cb_type == "uniform" {
            (0..(1 << (t_bits + (if states == 1 { 0 } else { 1 }))))
                .map(|i| {
                    -3.0 + (6.0 * (i as f64 + 0.5))
                        / (1 << (t_bits + (if states == 1 { 0 } else { 1 }))) as f64
                })
                .collect()
        } else {
            get_lloyd_max_centroids(if states == 1 { t_bits } else { t_bits + 1 }, cb_type)
                .iter()
                .map(|&x| x as f64)
                .collect()
        };

        let transitions_opt = get_transitions(states);
        let transitions = transitions_opt.as_deref().unwrap_or(&[]);

        let res = trellis_quantize_block(
            &bm.chunk_w,
            &base_levels,
            states,
            bm.scale as f64,
            true,
            transitions,
        );

        if let Some(p) = res.path_data.get(local_idx) {
            let base_eq = if settings.trellis_states == 1 {
                format!("CW[{}]", p.cbIdx)
            } else {
                format!(
                    "S{} &rarr; S{} D{}[{}]",
                    p.prevState, p.state, p.subset, p.cbIdx
                )
            };
            data.math_str = Some(if settings.trellis_use_wht {
                format!(
                    "D({}) &times; FWHT( {} )[{}]",
                    if get_sign_flip(idx, settings.trellis_sign_seed) > 0.0 {
                        "+1"
                    } else {
                        "-1"
                    },
                    base_eq,
                    idx
                )
            } else {
                base_eq
            });

            let cands_str = p.candidates.iter().map(|c| {
                let cost = if c.cost.is_infinite() { 1e9 } else if c.cost.is_nan() { 0.0 } else { c.cost };
                let dist = if c.dist.is_infinite() { 1e9 } else if c.dist.is_nan() { 0.0 } else { c.dist };
                let val = if c.cbVal.is_infinite() { 1e9 } else if c.cbVal.is_nan() { 0.0 } else { c.cbVal };
                format!(r#"{{"prevState":{},"subset":{},"cbIdx":{},"cbVal":{},"dist":{},"cost":{}}}"#, c.prevState, c.subset, c.cbIdx, val, dist, cost)
            }).collect::<Vec<_>>().join(",");

            let p_cost = if p.cost.is_infinite() {
                1e9
            } else if p.cost.is_nan() {
                0.0
            } else {
                p.cost
            };
            let p_val = if p.cwVal.is_infinite() {
                1e9
            } else if p.cwVal.is_nan() {
                0.0
            } else {
                p.cwVal
            };

            data.trellis_json = Some(format!(
                r#"{{"cbIdx":{},"prevState":{},"state":{},"subset":{},"cbVal":{},"cost":{},"candidates":[{}]}}"#,
                p.cbIdx, p.prevState, p.state, p.subset, p_val, p_cost, cands_str
            ));
        }
    }
    data
}
