import { getWasm } from './wasmWrapper.js';

const lloydMaxCache = {};

export function getLloydMaxCentroids(bits, dist = 'normal') {
    const cacheKey = `${bits}_${dist}`;
    if (lloydMaxCache[cacheKey]) return lloydMaxCache[cacheKey];

    try {
        const wasm = getWasm();
        const res = wasm.wasm_get_lloyd_max_centroids(bits, dist);
        lloydMaxCache[cacheKey] = Array.from(res);
        return lloydMaxCache[cacheKey];
    } catch (e) {
        throw e;
    }
}

export function fwht(data) {
    const wasm = getWasm();
    return Array.from(wasm.wasm_fwht(new Float64Array(data)));
}

export function getSignFlip(index, seed = 0) {
    let h = Math.sin(index * 12.9898 + seed * 78.233 + 1) * 43758.5453;
    return (h - Math.floor(h)) >= 0.5 ? 1 : -1;
}

export function getErrStats(arrO, arrQ) {
    let se = 0, ae = 0;
    for (let i = 0; i < arrO.length; i++) {
        se += Math.pow(arrO[i] - arrQ[i], 2);
        ae += Math.abs(arrO[i] - arrQ[i]);
    }
    return { mse: se / arrO.length, mae: ae / arrO.length };
}

export function snapToCodebook(val, cb) {
    let absVal = Math.abs(val);
    let best = cb[0];
    let bestDist = Math.abs(absVal - best);
    for (let i = 1; i < cb.length; i++) {
        let dist = Math.abs(absVal - cb[i]);
        if (dist < bestDist) {
            bestDist = dist;
            best = cb[i];
        }
    }
    return val >= 0 ? best : -best;
}

export function fp32(val) {
    return Math.fround(val);
}

export function fp16(val) {
    if (val === 0) return 0;
    let abs = Math.abs(val);
    if (abs >= 65504) return Math.sign(val) * 65504;
    if (abs < 5.96046e-8) return 0;
    if (abs < 0.000061035) return Math.sign(val) * Math.round(abs / 5.96046e-8) * 5.96046e-8;
    let exp = Math.floor(Math.log2(abs)), m = Math.round((abs / Math.pow(2, exp) - 1) * 1024);
    if (m === 1024) { m = 0; exp += 1; }
    if (exp > 15) return Math.sign(val) * 65504;
    return Math.sign(val) * Math.pow(2, exp) * (1 + m / 1024);
}

export function bf16(val) {
    if (val === 0) return 0;
    let abs = Math.abs(val);
    if (abs >= 3.389531389251535e38) return Math.sign(val) * 3.389531389251535e38;
    if (abs < 9.18355e-41) return 0;
    if (abs < 1.1754943508222875e-38) return Math.sign(val) * Math.round(abs / 9.18355e-41) * 9.18355e-41;
    let exp = Math.floor(Math.log2(abs));
    let m = Math.round((abs / Math.pow(2, exp) - 1) * 128);
    if (m === 128) { m = 0; exp += 1; }
    if (exp > 127) return Math.sign(val) * 3.389531389251535e38;
    return Math.sign(val) * Math.pow(2, exp) * (1 + m / 128);
}

export function fp8_e5m2(val) {
    if (val === 0) return 0;
    let abs = Math.abs(val);
    if (abs >= 57344) return Math.sign(val) * 57344;
    if (abs < 1.5258789e-5) return 0;
    if (abs < 6.1035156e-5) return Math.sign(val) * Math.round(abs / 1.5258789e-5) * 1.5258789e-5;
    let exp = Math.floor(Math.log2(abs));
    let m = Math.round((abs / Math.pow(2, exp) - 1) * 4);
    if (m === 4) { m = 0; exp += 1; }
    if (exp > 15) return Math.sign(val) * 57344;
    return Math.sign(val) * Math.pow(2, exp) * (1 + m / 4);
}

export function fp8_e4m3(val) {
    if (val === 0) return 0;
    let abs = Math.abs(val);
    if (abs >= 448) return Math.sign(val) * 448;
    if (abs < 0.015625) return Math.sign(val) * Math.round(abs / 0.001953) * 0.001953;
    let exp = Math.floor(Math.log2(abs)), m = Math.round((abs / Math.pow(2, exp) - 1) * 8);
    if (m === 8) { m = 0; exp += 1; }
    if (exp > 8 || (exp === 8 && m > 6)) return Math.sign(val) * 448;
    return Math.sign(val) * Math.pow(2, exp) * (1 + m / 8);
}