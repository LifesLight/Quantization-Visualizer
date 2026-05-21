/**
 * Data Generation
 * Handles reading distribution parameters from the UI and passing them to the WASM backend
 * to generate synthetic weight data.
 */
import { elements } from './ui.js';
import { getWasm } from './wasmWrapper.js';
import { getBaseFloats } from './render/state.js';

/**
 * Generates a new dataset based on the current UI parameters and updates the input text area.
 */
export function generateData() {
    // 1. Base Parameters
    const dist = elements.distEl.value;
    const count = parseInt(elements.genCountEl.value) || 256;

    // 2. Distribution-specific Parameters
    const uniRange = parseFloat(elements.genUniRangeEl.value) || 10.0;
    const lapScale = parseFloat(elements.genScaleEl.value) || 1.0;
    const bimDist = parseFloat(elements.genBimodalDistEl.value) || 4.0;
    const bimSpread = parseFloat(elements.genBimodalSpreadEl.value) || 1.2;
    const outProb = parseFloat(elements.genOutlierProbEl.value) || 0.02;
    const outMult = parseFloat(elements.genOutlierMultEl.value) || 5.0;
    const normStd = parseFloat(elements.genStdEl.value) || 1.0;

    // 3. Generate via WASM
    const wasm = getWasm();
    const resultStr = wasm.generate_dataset(
        dist, count, uniRange, lapScale, bimDist, bimSpread, outProb, outMult, normStd
    );

    // 4. Update UI
    elements.inputEl.value = resultStr;
}

/**
 * Generates an importance dataset perfectly matching the current active data length.
 */
export function generateImportanceData() {
    const baseFloats = getBaseFloats();
    const count = baseFloats.length;
    if (count === 0) return;

    const dist = elements.impGenDist.value;
    const uniRange = parseFloat(elements.impGenUniRange.value) || 10.0;
    const lapScale = parseFloat(elements.impGenScale.value) || 1.0;
    const bimDist = parseFloat(elements.impGenBimodalDist.value) || 4.0;
    const bimSpread = parseFloat(elements.impGenBimodalSpread.value) || 1.2;
    const outProb = parseFloat(elements.impGenOutlierProb.value) || 0.02;
    const outMult = parseFloat(elements.impGenOutlierMult.value) || 5.0;
    const normStd = parseFloat(elements.impGenStd.value) || 1.0;

    const wasm = getWasm();
    const resultStr = wasm.generate_dataset(
        dist, count, uniRange, lapScale, bimDist, bimSpread, outProb, outMult, normStd
    );

    elements.inputImportanceEl.value = resultStr.split(', ').map(x => Math.abs(parseFloat(x)).toFixed(5)).join(', ');
}