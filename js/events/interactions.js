/**
 * UI Interactions
 * Manages pointer events, dragging, and hover states for the main chart and dataset minimap.
 */
import { elements } from '../ui.js';
import { state, getBaseFloats, getHasWarnedLargeData, setUserModifiedClip } from '../render/state.js';
import { setHoveredIdx, setDragState, setZoomRange, resetZoom, toggleSRHT, setClipRange } from '../render/actions.js';
import { requantize } from '../render/core.js';
import { updateDbBarVisibility } from '../render/components.js';

// --- Shared State ---
export let pendingAction = null;
export function clearPendingAction() { pendingAction = null; }

function getNearestBarIdx(clientX) {
    const baseFloats = getBaseFloats();
    const N = baseFloats.length;
    if (N === 0) return null;

    const rect = elements.chartArea.getBoundingClientRect();
    const px = Math.max(0, Math.min(rect.width, clientX - rect.left));
    const zS = state.zoomRange ? state.zoomRange.start : 0;
    const zE = state.zoomRange ? state.zoomRange.end : N - 1;
    const zCount = zE - zS + 1;

    let col = Math.floor((px / rect.width) * zCount);
    if (col >= zCount) col = zCount - 1;

    return Math.max(0, Math.min(N - 1, zS + col));
}

export function setupChartInteractions() {
    elements.chartArea.addEventListener('pointermove', (e) => {
        const idx = getNearestBarIdx(e.clientX);
        if (idx !== null && !isNaN(idx) && idx !== state.lastHoveredIdx) {
            setHoveredIdx(idx);
        }
    });

    elements.chartArea.addEventListener('pointerleave', () => setHoveredIdx(null));
    elements.btnResetZoom.addEventListener('click', () => resetZoom());
    elements.btnToggleSRHT.addEventListener('click', () => toggleSRHT());

    let dragStartIdxTemp = null;
    elements.chartArea.addEventListener('pointerdown', (e) => {
        if (e.button !== 0) return;
        e.preventDefault();

        elements.chartArea.setPointerCapture(e.pointerId);
        dragStartIdxTemp = getNearestBarIdx(e.clientX);

        if (dragStartIdxTemp === null) return;
        setDragState(dragStartIdxTemp, dragStartIdxTemp);

        const onPointerMove = (moveEvt) => {
            const currIdx = getNearestBarIdx(moveEvt.clientX);
            if (currIdx !== null) setDragState(dragStartIdxTemp, currIdx);
        };

        const onPointerUp = (upEvt) => {
            elements.chartArea.releasePointerCapture(upEvt.pointerId);
            elements.chartArea.removeEventListener('pointermove', onPointerMove);
            elements.chartArea.removeEventListener('pointerup', onPointerUp);

            let startIdx = dragStartIdxTemp;
            let endIdx = getNearestBarIdx(upEvt.clientX);
            dragStartIdxTemp = null;

            if (startIdx !== null && endIdx !== null && startIdx !== endIdx) {
                setZoomRange(Math.min(startIdx, endIdx), Math.max(startIdx, endIdx));
            }

            setHoveredIdx(getNearestBarIdx(upEvt.clientX));
            setDragState(null, null);
        };

        elements.chartArea.addEventListener('pointermove', onPointerMove);
        elements.chartArea.addEventListener('pointerup', onPointerUp);
    });

    elements.chartArea.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        resetZoom();
    });
}

export function setupDatasetBarInteractions() {
    if (elements.dbToggleBtn) {
        elements.dbToggleBtn.addEventListener('click', () => {
            const isHidden = elements.dbToolsPanel.style.display === 'none';
            elements.dbToolsPanel.style.display = isHidden ? 'flex' : 'none';
            elements.dbToggleBtn.classList.toggle('open', isHidden);
        });
    }

    if (elements.dbModeClip) {
        elements.dbModeClip.addEventListener('change', (e) => {
            if (elements.dbClipParams) elements.dbClipParams.style.display = e.target.checked ? 'flex' : 'none';

            const N = getBaseFloats().length;
            if (!e.target.checked && N > 1048576 && !getHasWarnedLargeData()) {
                e.preventDefault();
                e.target.checked = true;
                if (elements.dbClipParams) elements.dbClipParams.style.display = 'flex';
                pendingAction = { type: 'uncheck_clip' };
                elements.modalLargeData.style.display = 'flex';
            } else {
                if (!e.target.checked) setUserModifiedClip(false);
                updateDbBarVisibility();
                requantize(true);
            }
        });
    }

    [elements.dbModeMinMax, elements.dbModeHotspots].forEach(el => {
        if (el) el.addEventListener('change', updateDbBarVisibility);
    });

    const applyClipInputs = () => {
        const N = getBaseFloats().length;
        if (N === 0) return;

        let start = parseInt(elements.dbClipStart.value) || 0;
        let width = parseInt(elements.dbClipWidth.value) || 1;
        start = Math.max(0, Math.min(start, N - 1));
        width = Math.max(1, width);
        let end = Math.min(start + width - 1, N - 1);
        setUserModifiedClip(true);

        if (end - start + 1 > 1048576 && !getHasWarnedLargeData()) {
            pendingAction = { type: 'slider', start, end };
            elements.modalLargeData.style.display = 'flex';
        } else {
            setClipRange(start, end, false);
        }
    };

    if (elements.dbClipStart) elements.dbClipStart.addEventListener('change', applyClipInputs);
    if (elements.dbClipWidth) elements.dbClipWidth.addEventListener('change', applyClipInputs);

    let isDraggingLeft = false, isDraggingRight = false, isDraggingCenter = false;
    let initialClipStart = 0, initialClipEnd = 0, dragStartXRatio = 0, hasMoved = false, initialMouseX = 0;
    const DRAG_THRESHOLD = 4;

    const onDbPointerMove = (e) => {
        if (!isDraggingLeft && !isDraggingRight && !isDraggingCenter) return;
        if (!hasMoved) {
            if (Math.abs(e.clientX - initialMouseX) >= DRAG_THRESHOLD) hasMoved = true;
            else return;
        }

        const rect = elements.dbBar.getBoundingClientRect();
        const N = getBaseFloats().length;
        if (rect.width <= 0 || N === 0) return;

        const zS = state.zoomRange ? state.zoomRange.start : 0;
        const zE = state.zoomRange ? state.zoomRange.end : N - 1;
        const zCount = zE - zS + 1;
        let pxRatio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
        const clampToZoom = (idx) => Math.max(zS, Math.min(zE, idx));

        if (isDraggingCenter) {
            let deltaIdx = Math.round((pxRatio - dragStartXRatio) * (zCount - 1));
            const minDelta = zS - initialClipStart;
            const maxDelta = zE - initialClipEnd;
            deltaIdx = Math.max(minDelta, Math.min(maxDelta, deltaIdx));
            setClipRange(initialClipStart + deltaIdx, initialClipEnd + deltaIdx, true);
            return;
        }

        let targetIdx = clampToZoom(Math.round(zS + pxRatio * (zCount - 1)));
        const cS = clampToZoom(state.clipStart);
        const cE = clampToZoom(state.clipEnd);

        if (isDraggingLeft) setClipRange(Math.min(targetIdx, cE), cE, true);
        else if (isDraggingRight) setClipRange(cS, Math.max(targetIdx, cS), true);
    };

    const onDbPointerUp = (e) => {
        if (isDraggingLeft || isDraggingRight || isDraggingCenter) {
            isDraggingLeft = isDraggingRight = isDraggingCenter = false;
            if (e.target.releasePointerCapture) e.target.releasePointerCapture(e.pointerId);
            e.target.removeEventListener('pointermove', onDbPointerMove);
            e.target.removeEventListener('pointerup', onDbPointerUp);

            if (hasMoved) {
                setUserModifiedClip(true);
                if (state.clipEnd - state.clipStart + 1 > 1048576 && !getHasWarnedLargeData()) {
                    pendingAction = { type: 'slider', start: state.clipStart, end: state.clipEnd };
                    setClipRange(initialClipStart, initialClipEnd, true);
                    elements.modalLargeData.style.display = 'flex';
                } else {
                    requantize(true);
                }
            } else {
                setClipRange(initialClipStart, initialClipEnd, true);
            }
        }
    };

    const attachDrag = (el, type) => {
        if (!el) return;
        el.addEventListener('pointerdown', (e) => {
            if (type === 'center' && (e.target === elements.dbHandleLeft || e.target === elements.dbHandleRight)) return;
            e.preventDefault(); e.stopPropagation();
            el.setPointerCapture(e.pointerId);
            isDraggingLeft = type === 'left'; isDraggingRight = type === 'right'; isDraggingCenter = type === 'center';
            hasMoved = false; initialMouseX = e.clientX;
            initialClipStart = state.clipStart; initialClipEnd = state.clipEnd;
            if (type === 'center') {
                const barRect = elements.dbBar.getBoundingClientRect();
                dragStartXRatio = Math.max(0, Math.min(1, (e.clientX - barRect.left) / barRect.width));
            }
            el.addEventListener('pointermove', onDbPointerMove);
            el.addEventListener('pointerup', onDbPointerUp);
        });
    };

    attachDrag(elements.dbActiveRegion, 'center');
    attachDrag(elements.dbHandleLeft, 'left');
    attachDrag(elements.dbHandleRight, 'right');

    if (elements.dbActiveRegion) {
        elements.dbActiveRegion.addEventListener('dblclick', (e) => {
            e.preventDefault(); e.stopPropagation();
            if (state.clipStart !== null && state.clipEnd !== null) setZoomRange(state.clipStart, state.clipEnd);
        });
    }
}