const lloydMaxCache = {};

export function getLloydMaxCentroids(bits) {
    if (lloydMaxCache[bits]) return lloydMaxCache[bits];
    const levels = Math.pow(2, bits);
    const centroids = new Float64Array(levels);

    for (let i = 0; i < levels; i++) centroids[i] = -3 + (6 * (i + 0.5)) / levels;

    const pdf = (x) => Math.exp(-x * x / 2);

    for (let iter = 0; iter < 100; iter++) {
        const thresholds = new Float64Array(levels + 1);
        thresholds[0] = -10;
        for (let i = 0; i < levels - 1; i++) thresholds[i + 1] = (centroids[i] + centroids[i + 1]) / 2;
        thresholds[levels] = 10;

        for (let i = 0; i < levels; i++) {
            let num = 0, den = 0;
            const tStart = thresholds[i];
            const tEnd = thresholds[i + 1];
            const steps = 200;
            const dt = (tEnd - tStart) / steps;

            for (let j = 0; j < steps; j++) {
                const x = tStart + (j + 0.5) * dt;
                const p = pdf(x);
                num += x * p;
                den += p;
            }
            if (den > 1e-9) centroids[i] = num / den;
        }
    }
    lloydMaxCache[bits] = Array.from(centroids);
    return lloydMaxCache[bits];
}

export function fwht(data) {
    let n = data.length;
    let p2 = 1; while (p2 < n) p2 *= 2;
    let res = new Float64Array(p2);
    for (let i = 0; i < n; i++) res[i] = data[i];

    for (let h = 1; h < p2; h *= 2) {
        for (let i = 0; i < p2; i += h * 2) {
            for (let j = i; j < i + h; j++) {
                let x = res[j];
                let y = res[j + h];
                res[j] = x + y;
                res[j + h] = x - y;
            }
        }
    }

    let scale = 1 / Math.sqrt(p2);
    for (let i = 0; i < p2; i++) res[i] *= scale;
    return Array.from(res).slice(0, n);
}

export function getSignFlip(index) {
    let h = Math.sin(index * 12.9898 + 1) * 43758.5453;
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