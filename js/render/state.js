/**
 * Render State
 * Centralizes all mutable rendering data and raw WASM memory access.
 */
import { elements } from '../ui.js';
import { getF32Array } from '../wasmWrapper.js';

export const state = {
    backend: null,
    currentRenderData: null,
    lastHoveredIdx: 0,
    zoomRange: null,
    showSRHT: false,
    clipStart: null,
    clipEnd: null,
    hasWarnedLargeData: false,
    userModifiedClip: false,
    hoveredIdx: null,
    dragStartIdx: null,
    dragCurrentIdx: null,
    rawFloatsStr: "",
    lastScale: null,
    lastOffset: null
};

// --- Simple State Setters & Getters ---
export function setBackend(b) { state.backend = b; }
export function setHasWarnedLargeData(val) { state.hasWarnedLargeData = val; }
export function getHasWarnedLargeData() { return state.hasWarnedLargeData; }
export function getClipRange() { return { start: state.clipStart, end: state.clipEnd }; }
export function setUserModifiedClip(val) { state.userModifiedClip = val; }

/**
 * Parses and returns the base float array directly from input/WASM.
 * Applies scale/offset only if changed.
 */
export function getBaseFloats() {
    if (!state.backend) return new Float32Array();
    const text = elements.inputEl.value;

    const scale = parseFloat(elements.dataScaleEl.value);
    const finalScale = isNaN(scale) ? 1.0 : scale;

    const offset = parseFloat(elements.dataOffsetEl.value);
    const finalOffset = isNaN(offset) ? 0.0 : offset;

    // Reparse only if raw text changed
    if (text !== state.rawFloatsStr) {
        state.rawFloatsStr = text;
        state.backend.parse_floats(text);
        state.lastScale = null;
    }

    // Apply scaling if changed
    if (finalScale !== state.lastScale || finalOffset !== state.lastOffset) {
        state.backend.set_scale_offset(finalScale, finalOffset);
        state.lastScale = finalScale;
        state.lastOffset = finalOffset;
    }

    return getF32Array(state.backend.get_base_floats_ptr(), state.backend.get_base_floats_len());
}