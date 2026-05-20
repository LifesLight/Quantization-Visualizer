/**
 * WASM Wrapper
 * Handles initialization and safe memory access for the WebAssembly backend.
 */

import init, * as wasm from '../wasm/pkg/quant_wasm.js';

let isReady = false;
let wasmInstance = null;

/**
 * Initializes the WASM module if it hasn't been initialized yet.
 */
export async function initWasm() {
    if (!isReady) {
        wasmInstance = await init();
        isReady = true;
    }
}

/**
 * Returns the initialized WASM module exports.
 * @throws {Error} If called before initialization.
 */
export function getWasm() {
    if (!isReady) throw new Error("Wasm not initialized yet");
    return wasm;
}

/**
 * Safely extracts a Float32Array from WASM memory.
 * @param {number} ptr - Memory pointer
 * @param {number} len - Array length
 * @returns {Float32Array}
 */
export function getF32Array(ptr, len) {
    if (!isReady || !wasmInstance) throw new Error("Wasm not initialized yet");
    if (len === 0 || ptr === 0) return new Float32Array();

    return new Float32Array(wasmInstance.memory.buffer, ptr, len);
}

/**
 * Safely extracts an Int32Array from WASM memory.
 * @param {number} ptr - Memory pointer
 * @param {number} len - Array length
 * @returns {Int32Array}
 */
export function getI32Array(ptr, len) {
    if (!isReady || !wasmInstance) throw new Error("Wasm not initialized yet");
    if (len === 0 || ptr === 0) return new Int32Array();

    return new Int32Array(wasmInstance.memory.buffer, ptr, len);
}