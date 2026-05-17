import init, * as wasm from '../wasm/pkg/quant_wasm.js';

let isReady = false;

export async function initWasm() {
    if (!isReady) {
        await init();
        isReady = true;
    }
}

export function getWasm() {
    if (!isReady) throw new Error("Wasm not initialized yet");
    return wasm;
}