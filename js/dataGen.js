import { elements } from './ui.js';

export function generateData() {
    const dist = elements.distEl.value;
    const count = parseInt(elements.genCountEl.value) || 256;
    let arr = [];

    for (let i = 0; i < count; i++) {
        let val = 0;
        if (dist === 'uniform') {
            const range = parseFloat(elements.genUniRangeEl.value) || 10.0;
            val = (Math.random() * range) - (range / 2);
        } else if (dist === 'laplace') {
            const scale = parseFloat(elements.genScaleEl.value) || 1.0;
            const u = Math.random() - 0.5;
            val = -scale * Math.sign(u) * Math.log(1 - 2 * Math.abs(u));
        } else if (dist === 'bimodal') {
            const pDist = parseFloat(elements.genBimodalDistEl.value) || 4.0;
            const pSpread = parseFloat(elements.genBimodalSpreadEl.value) || 1.2;
            const peak = Math.random() > 0.5 ? pDist : -pDist;
            let u = 0, v = 0; while (u === 0) u = Math.random(); while (v === 0) v = Math.random();
            val = peak + Math.sqrt(-2.0 * Math.log(u)) * Math.cos(2.0 * Math.PI * v) * pSpread;
        } else if (dist === 'outliers') {
            const oProb = parseFloat(elements.genOutlierProbEl.value) || 0.02;
            const oMult = parseFloat(elements.genOutlierMultEl.value) || 5.0;
            let u = 0, v = 0; while (u === 0) u = Math.random(); while (v === 0) v = Math.random();
            val = Math.sqrt(-2.0 * Math.log(u)) * Math.cos(2.0 * Math.PI * v);
            if (Math.random() < oProb) val *= (Math.random() > 0.5 ? oMult : -oMult);
        } else {
            const std = parseFloat(elements.genStdEl.value) || 1.0;
            let u = 0, v = 0; while (u === 0) u = Math.random(); while (v === 0) v = Math.random();
            val = Math.sqrt(-2.0 * Math.log(u)) * Math.cos(2.0 * Math.PI * v) * std;
        }

        arr.push(val.toFixed(5));
    }
    elements.inputEl.value = arr.join(', ');
}