pub fn fp32(val: f32) -> f32 {
    val
}

pub fn fp16(val: f32) -> f32 {
    if val == 0.0 {
        return 0.0;
    }
    let abs = val.abs();
    if abs >= 65504.0 {
        return val.signum() * 65504.0;
    }
    if abs < 5.96046e-8 {
        return 0.0;
    }
    if abs < 0.000061035 {
        return val.signum() * (abs / 5.96046e-8).round() * 5.96046e-8;
    }
    let exp = abs.log2().floor();
    let mut m = ((abs / exp.exp2() - 1.0) * 1024.0).round();
    let mut exp_adj = exp;
    if m == 1024.0 {
        m = 0.0;
        exp_adj += 1.0;
    }
    if exp_adj > 15.0 {
        return val.signum() * 65504.0;
    }
    val.signum() * exp_adj.exp2() * (1.0 + m / 1024.0)
}

pub fn bf16(val: f32) -> f32 {
    if val == 0.0 {
        return 0.0;
    }
    let abs = val.abs();
    if abs >= 3.389531389251535e38 {
        return val.signum() * 3.389531389251535e38;
    }
    if abs < 9.18355e-41 {
        return 0.0;
    }
    if abs < 1.1754943508222875e-38 {
        return val.signum() * (abs / 9.18355e-41).round() * 9.18355e-41;
    }
    let exp = abs.log2().floor();
    let mut m = ((abs / exp.exp2() - 1.0) * 128.0).round();
    let mut exp_adj = exp;
    if m == 128.0 {
        m = 0.0;
        exp_adj += 1.0;
    }
    if exp_adj > 127.0 {
        return val.signum() * 3.389531389251535e38;
    }
    val.signum() * exp_adj.exp2() * (1.0 + m / 128.0)
}

pub fn fp8_e5m2(val: f32) -> f32 {
    if val == 0.0 {
        return 0.0;
    }
    let abs = val.abs();
    if abs >= 57344.0 {
        return val.signum() * 57344.0;
    }
    if abs < 1.5258789e-5 {
        return 0.0;
    }
    if abs < 6.1035156e-5 {
        return val.signum() * (abs / 1.5258789e-5).round() * 1.5258789e-5;
    }
    let exp = abs.log2().floor();
    let mut m = ((abs / exp.exp2() - 1.0) * 4.0).round();
    let mut exp_adj = exp;
    if m == 4.0 {
        m = 0.0;
        exp_adj += 1.0;
    }
    if exp_adj > 15.0 {
        return val.signum() * 57344.0;
    }
    val.signum() * exp_adj.exp2() * (1.0 + m / 4.0)
}

pub fn fp8_e4m3(val: f32) -> f32 {
    if val == 0.0 {
        return 0.0;
    }
    let abs = val.abs();
    if abs >= 448.0 {
        return val.signum() * 448.0;
    }
    if abs < 0.015625 {
        return val.signum() * (abs / 0.001953).round() * 0.001953;
    }
    let exp = abs.log2().floor();
    let mut m = ((abs / exp.exp2() - 1.0) * 8.0).round();
    let mut exp_adj = exp;
    if m == 8.0 {
        m = 0.0;
        exp_adj += 1.0;
    }
    if exp_adj > 8.0 || (exp_adj == 8.0 && m > 6.0) {
        return val.signum() * 448.0;
    }
    val.signum() * exp_adj.exp2() * (1.0 + m / 8.0)
}

pub fn get_err_stats(arr_o: &[f32], arr_q: &[f32]) -> (f64, f64) {
    let mut se = 0.0;
    let mut ae = 0.0;
    // .zip() elides bounds checks for massive Wasm speedups
    for (&o, &q) in arr_o.iter().zip(arr_q.iter()) {
        let diff = (o - q) as f64;
        se += diff * diff;
        ae += diff.abs();
    }
    let n = arr_o.len().max(1) as f64;
    (se / n, ae / n)
}

// In-place zero-allocation WHT for hot loops
pub fn fwht_f32_inplace(res: &mut [f32]) {
    let p2 = res.len();
    let mut h = 1;
    while h < p2 {
        let mut i = 0;
        while i < p2 {
            for j in i..i + h {
                let x = res[j];
                let y = res[j + h];
                res[j] = x + y;
                res[j + h] = x - y;
            }
            i += h * 2;
        }
        h *= 2;
    }
    let scale = 1.0 / (p2 as f32).sqrt();
    for x in res.iter_mut() {
        *x *= scale;
    }
}

// Kept original for backward compatibility with other unpasted modules
pub fn fwht_f32(data: &[f32]) -> Vec<f32> {
    let n = data.len();
    let mut p2 = 1;
    while p2 < n {
        p2 *= 2;
    }
    let mut res = vec![0.0; p2];
    res[..n].copy_from_slice(data);
    fwht_f32_inplace(&mut res);
    res.truncate(n);
    res
}

pub fn get_sign_flip(index: usize, seed: u32) -> f32 {
    let h = ((index as f64) * 12.9898 + (seed as f64) * 78.233 + 1.0).sin() * 43758.5453;
    // Keeping JS exact match (-h.floor() instead of .fract() due to negative fraction logic)
    if (h - h.floor()) >= 0.5 {
        1.0
    } else {
        -1.0
    }
}

pub fn get_lloyd_max_centroids(bits: u32, dist: &str) -> Vec<f32> {
    let levels = 1_usize << bits;
    let mut centroids = vec![0.0; levels];
    for i in 0..levels {
        centroids[i] = -3.0 + (6.0 * (i as f64 + 0.5)) / (levels as f64);
    }
    let is_laplace = dist == "laplace";
    let pdf = |x: f64| -> f64 {
        if is_laplace {
            0.5 * (-x.abs()).exp()
        } else {
            (-x * x / 2.0).exp()
        }
    };
    let steps = 200;
    for _ in 0..100 {
        let mut thresholds = vec![0.0; levels + 1];
        thresholds[0] = -10.0;
        for i in 0..levels - 1 {
            thresholds[i + 1] = (centroids[i] + centroids[i + 1]) / 2.0;
        }
        thresholds[levels] = 10.0;
        for i in 0..levels {
            let mut num = 0.0;
            let mut den = 0.0;
            let t_start = thresholds[i];
            let t_end = thresholds[i + 1];
            let dt = (t_end - t_start) / (steps as f64);
            for j in 0..steps {
                let x = t_start + (j as f64 + 0.5) * dt;
                let p = pdf(x);
                num += x * p;
                den += p;
            }
            if den > 1e-9 {
                centroids[i] = num / den;
            }
        }
    }
    centroids.iter().map(|&x| x as f32).collect()
}

pub fn snap_to_codebook(val: f32, cb: &[f32]) -> f32 {
    let abs_val = val.abs();
    let mut best = cb[0];
    let mut best_dist = (abs_val - best).abs();
    for &c in &cb[1..] {
        let dist = (abs_val - c).abs();
        if dist < best_dist {
            best_dist = dist;
            best = c;
        }
    }
    if val >= 0.0 {
        best
    } else {
        -best
    }
}
