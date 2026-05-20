import init, * as wasm from '../wasm/pkg/quant_wasm.js';

let isReady = false;
let wasmInstance = null;

export async function initWasm() {
    if (!isReady) {
        wasmInstance = await init();
        isReady = true;
    }
}

export function getWasm() {
    if (!isReady) throw new Error("Wasm not initialized yet");
    return wasm;
}

export function getF32Array(ptr, len) {
    if (!isReady || !wasmInstance) throw new Error("Wasm not initialized yet");
    if (len === 0 || ptr === 0) return new Float32Array();

    return new Float32Array(wasmInstance.memory.buffer, ptr, len);
}

export function getI32Array(ptr, len) {
    if (!isReady || !wasmInstance) throw new Error("Wasm not initialized yet");
    if (len === 0 || ptr === 0) return new Int32Array();

    return new Int32Array(wasmInstance.memory.buffer, ptr, len);
}