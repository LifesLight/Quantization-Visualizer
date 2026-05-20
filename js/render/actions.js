/**
 * Render Actions
 * Exported functions that external scripts call to safely mutate the view state.
 */
import { state } from './state.js';
import { updateInspector, updateOverlays, drawDatasetBar } from './components.js';
import { requantize, render } from './core.js';

export function setHoveredIdx(idx) {
    if (state.hoveredIdx === idx) return;
    state.hoveredIdx = idx;

    if (idx !== null) {
        state.lastHoveredIdx = idx;
        if (state.dragStartIdx === null) updateInspector(idx);
    }
    updateOverlays();
}

export function setDragState(startIdx, currentIdx) {
    if (state.dragStartIdx === startIdx && state.dragCurrentIdx === currentIdx) return;
    state.dragStartIdx = startIdx;
    state.dragCurrentIdx = currentIdx;
    updateOverlays();
}

export function setClipRange(start, end, previewOnly = false) {
    state.clipStart = start;
    state.clipEnd = end;
    if (previewOnly) drawDatasetBar();
    else requantize(true);
}

export function setZoomRange(start, end, doRender = true) {
    state.zoomRange = { start, end };
    if (doRender) render();
}

export function resetZoom(doRender = true) {
    state.zoomRange = null;
    if (doRender) render();
}

export function toggleSRHT() {
    state.showSRHT = !state.showSRHT;
    render();
}