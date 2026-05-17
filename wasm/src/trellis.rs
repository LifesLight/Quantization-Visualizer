use serde::Serialize;
use wasm_bindgen::prelude::*;

// Internal Trellis Helpers
#[derive(Clone, Copy)]
struct Edge {
    from: usize,
    sub: usize,
}

fn get_transitions(states: usize) -> Option<Vec<Vec<Edge>>> {
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
            let sub_msb = input ^ (sub_msb_pop & 1);

            let sub = (sub_msb << 1) | sub_lsb;
            transitions[next_state].push(Edge {
                from: state,
                sub: sub as usize,
            });
        }
    }
    Some(transitions)
}

fn find_closest_idx(target: f64, subset_idxs: &[usize], levels: &[f64]) -> (f64, usize) {
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

// Data Transfer Objects
#[allow(non_snake_case)]
#[derive(Serialize, Clone, Copy)]
pub struct Candidate {
    pub prevState: isize,
    pub subset: usize,
    pub cbIdx: usize,
    pub cbVal: f64,
    pub dist: f64,
    pub cost: f64,
}

#[allow(non_snake_case)]
#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum NextState {
    State(usize),
    End(String),
}

#[allow(non_snake_case)]
#[derive(Serialize, Clone)]
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

#[allow(non_snake_case)]
#[derive(Serialize)]
pub struct TrellisBlockResult {
    pub chunkQ: Vec<f64>,
    pub pathData: Vec<StepInfo>,
    pub cost: f64,
}

#[derive(Clone)]
struct StateInfo {
    prev_state: isize,
    cb_val: f64,
    cb_idx: usize,
    subset: usize,
    cost: f64,
    candidates: Vec<Candidate>,
}

#[derive(Clone)]
struct StepData {
    costs: Vec<f64>,
    state_info: Vec<Option<StateInfo>>,
}

// Wasm Bindings
#[wasm_bindgen]
pub fn wasm_trellis_opt_scale(
    samples: &[f64],
    base_levels: &[f64],
    states: usize,
    mut a: f64,
    mut b: f64,
    iters: usize,
    block_size: usize,
) -> f64 {
    let mut scaled_buf = vec![0.0; base_levels.len()];
    let mut subset_indices = vec![vec![]; 4];
    for i in 0..base_levels.len() {
        subset_indices[i % 4].push(i);
    }

    let transitions = get_transitions(states);

    let eval = |scale: f64, scaled_buf: &mut [f64]| -> f64 {
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
                let trans = transitions.as_ref().unwrap();
                let mut prev_costs = vec![f64::INFINITY; states];
                prev_costs[0] = 0.0;

                for &x in chunk {
                    let mut next_costs = vec![f64::INFINITY; states];
                    for curr_state in 0..states {
                        let mut min_cost = f64::INFINITY;
                        for edge in &trans[curr_state] {
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
                    prev_costs = next_costs;
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

#[wasm_bindgen]
pub fn wasm_trellis_quantize_block(
    samples: &[f64],
    base_levels: &[f64],
    states: usize,
    scale: f64,
) -> JsValue {
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
                cost: cost,
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
        min_final_cost = cost_total;
    } else {
        let transitions = get_transitions(states).unwrap();
        let mut prev_costs = vec![f64::INFINITY; states];
        prev_costs[0] = 0.0;

        let mut steps_info = Vec::with_capacity(samples.len());

        for &x in samples {
            let mut next_costs = vec![f64::INFINITY; states];
            let mut per_state = vec![None; states];

            for curr_state in 0..states {
                let mut best_prev = -1;
                let mut best_cb_val = 0.0;
                let mut best_cb_idx = 0;
                let mut best_sub = 0;
                let mut min_cost = f64::INFINITY;
                let mut candidates = vec![];

                for edge in &transitions[curr_state] {
                    let prev_cost = prev_costs[edge.from];
                    if prev_cost.is_infinite() {
                        continue;
                    }

                    let (val, idx) = find_closest_idx(x, &subset_indices[edge.sub], &scaled_buf);
                    let err = x - val;
                    let total = prev_cost + err * err;

                    candidates.push(Candidate {
                        prevState: edge.from as isize,
                        subset: edge.sub,
                        cbIdx: idx,
                        cbVal: val,
                        dist: err.abs(),
                        cost: total,
                    });

                    if total < min_cost {
                        min_cost = total;
                        best_prev = edge.from as isize;
                        best_cb_val = val;
                        best_cb_idx = idx;
                        best_sub = edge.sub;
                    }
                }

                next_costs[curr_state] = min_cost;
                per_state[curr_state] = Some(StateInfo {
                    prev_state: best_prev,
                    cb_val: best_cb_val,
                    cb_idx: best_cb_idx,
                    subset: best_sub,
                    cost: min_cost,
                    candidates,
                });
            }
            steps_info.push(StepData {
                costs: next_costs.clone(),
                state_info: per_state,
            });
            prev_costs = next_costs;
        }

        let mut final_state = 0;
        let mut min_fc = f64::INFINITY;
        for s in 0..states {
            if prev_costs[s] < min_fc {
                min_fc = prev_costs[s];
                final_state = s;
            }
        }
        min_final_cost = min_fc;

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

        let mut curr_state = final_state;
        for t in (0..samples.len()).rev() {
            let step = &steps_info[t].state_info[curr_state];
            if step.is_none() || step.as_ref().unwrap().prev_state == -1 {
                let (val, idx) = find_closest_idx(samples[t], &all_idxs, &scaled_buf);
                chunk_q[t] = val;
                let next_st = if t == samples.len() - 1 {
                    NextState::End("End".to_string())
                } else {
                    NextState::State(path_data[t + 1].state)
                };

                path_data[t] = StepInfo {
                    state: curr_state,
                    subset: 0,
                    cbIdx: idx,
                    cwVal: val,
                    prevState: -1,
                    nextState: next_st,
                    input: samples[t],
                    error: samples[t] - val,
                    cost: f64::INFINITY,
                    stateCosts: steps_info[t].costs.clone(),
                    candidates: vec![],
                };
                curr_state = 0;
            } else {
                let st = step.as_ref().unwrap();
                chunk_q[t] = st.cb_val;
                let next_st = if t == samples.len() - 1 {
                    NextState::End("End".to_string())
                } else {
                    NextState::State(path_data[t + 1].state)
                };

                path_data[t] = StepInfo {
                    state: curr_state,
                    subset: st.subset,
                    cbIdx: st.cb_idx,
                    cwVal: st.cb_val,
                    prevState: st.prev_state,
                    nextState: next_st,
                    input: samples[t],
                    error: samples[t] - st.cb_val,
                    cost: st.cost,
                    stateCosts: steps_info[t].costs.clone(),
                    candidates: st.candidates.clone(),
                };
                curr_state = st.prev_state as usize;
            }
        }
    }

    serde_wasm_bindgen::to_value(&TrellisBlockResult {
        chunkQ: chunk_q,
        pathData: path_data,
        cost: min_final_cost,
    })
    .unwrap()
}
