import { elements } from './ui.js';
import { getWasm } from './wasmWrapper.js';

export function generateData() {
    const dist = elements.distEl.value;
    const count = parseInt(elements.genCountEl.value) || 256;

    const uniRange = parseFloat(elements.genUniRangeEl.value) || 10.0;
    const lapScale = parseFloat(elements.genScaleEl.value) || 1.0;
    const bimDist = parseFloat(elements.genBimodalDistEl.value) || 4.0;
    const bimSpread = parseFloat(elements.genBimodalSpreadEl.value) || 1.2;
    const outProb = parseFloat(elements.genOutlierProbEl.value) || 0.02;
    const outMult = parseFloat(elements.genOutlierMultEl.value) || 5.0;
    const normStd = parseFloat(elements.genStdEl.value) || 1.0;

    const wasm = getWasm();

    const resultStr = wasm.generate_dataset(
        dist, count, uniRange, lapScale, bimDist, bimSpread, outProb, outMult, normStd
    );

    elements.inputEl.value = resultStr;
}