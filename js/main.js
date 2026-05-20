import './theme.js';
import { elements, initUIListeners, updateUI, applyPreset, populateDynamicSelectors } from './ui.js';
import { generateData } from './dataGen.js';
import { render, requantize, updateInspector, setZoomRange, resetZoom, toggleSRHT, getBaseFloats, getClipRange, setClipRange, getHasWarnedLargeData, setHasWarnedLargeData, drawDatasetBar, zoomRange, setUserModifiedClip, updateDbBarVisibility, updateVisualsOnly, setBackend, setHoveredIdx, setDragState, updateOverlays } from './render.js';
import { initWasm, getWasm } from './wasmWrapper.js';

/**
 * Maps a horizontal screen coordinate to the nearest data index relative to current zoom bounds.
 * @param {number} clientX - The horizontal screen coordinate.
 * @returns {number|null} The mapped index.
 */
function getNearestBarIdx(clientX) {
    const baseFloats = getBaseFloats();
    const N = baseFloats.length;
    if (N === 0) return null;

    const rect = elements.chartArea.getBoundingClientRect();
    const px = Math.max(0, Math.min(rect.width, clientX - rect.left));
    const zS = zoomRange ? zoomRange.start : 0;
    const zE = zoomRange ? zoomRange.end : N - 1;
    const zCount = zE - zS + 1;

    let col = Math.floor((px / rect.width) * zCount);
    if (col >= zCount) col = zCount - 1;

    return Math.max(0, Math.min(N - 1, zS + col));
}

let requantizeTimeout = null;

/**
 * Debounces the requantization process.
 * @param {boolean} forceSync - Skips the debounce delay.
 */
const debouncedRequantize = (forceSync = false) => {
    clearTimeout(requantizeTimeout);
    if (forceSync) {
        requantize();
    } else {
        requantizeTimeout = setTimeout(() => requantize(), 100);
    }
};

document.addEventListener('DOMContentLoaded', async () => {
    await initWasm();
    const wasm = getWasm();
    setBackend(new wasm.AppBackend());

    populateDynamicSelectors();
    initUIListeners();

let resizeRaf = null;
let resizeDebounce = null;

const resizeObserver = new ResizeObserver(() => {
    // Keep hover/drag overlays aligned during resize without expensive redraw
    if (resizeRaf) cancelAnimationFrame(resizeRaf);
    resizeRaf = requestAnimationFrame(() => updateOverlays());

    // Do ONE expensive redraw after resize settles
    clearTimeout(resizeDebounce);
    resizeDebounce = setTimeout(() => {
        // Skip inspector updates here; hover will refresh it on next pointermove anyway
        render({ updateInspector: false });
    }, 150); // tune 100–250ms
});

resizeObserver.observe(elements.chartArea);
    resizeObserver.observe(elements.chartArea);

    const processFile = (file) => {
        if (!file) return;
        const reader = new FileReader();
        reader.onload = (evt) => {
            elements.inputEl.value = evt.target.result;
            resetZoom(false);
            debouncedRequantize(true);
        };
        reader.readAsText(file);
    };

    if (elements.fileDropZone && elements.fileInput) {
        elements.fileDropZone.addEventListener('click', () => elements.fileInput.click());

        elements.fileInput.addEventListener('change', (e) => {
            processFile(e.target.files[0]);
            e.target.value = '';
        });

        elements.fileDropZone.addEventListener('dragover', (e) => {
            e.preventDefault();
            elements.fileDropZone.style.borderColor = 'var(--accent-color, #007bff)';
            elements.fileDropZone.style.background = 'rgba(128, 128, 128, 0.1)';
        });

        elements.fileDropZone.addEventListener('dragleave', (e) => {
            e.preventDefault();
            elements.fileDropZone.style.borderColor = 'var(--border-color, #555)';
            elements.fileDropZone.style.background = 'transparent';
        });

        elements.fileDropZone.addEventListener('drop', (e) => {
            e.preventDefault();
            elements.fileDropZone.style.borderColor = 'var(--border-color, #555)';
            elements.fileDropZone.style.background = 'transparent';
            if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
                processFile(e.dataTransfer.files[0]);
            }
        });
    }

    elements.autoUpdateElements.forEach(el => el.addEventListener('change', (e) => {
        if (e.target.id.startsWith('gen-')) {
            generateData();
            resetZoom(false);
            debouncedRequantize();
        } else if (e.target.id === 'centering-mode') {
            updateVisualsOnly();
        } else {
            if (e.target.id !== 'data-scale' && e.target.id !== 'data-offset') {
                elements.presetEl.value = 'custom';
                updateUI();
            }
            debouncedRequantize();
        }
    }));

    elements.presetEl.addEventListener('change', () => {
        if (elements.presetEl.value !== 'custom') {
            applyPreset(elements.presetEl.value);
            updateUI();
            debouncedRequantize();
        }
    });

    elements.genBtn.addEventListener('click', () => {
        generateData();
        resetZoom(false);
        debouncedRequantize();
    });

    elements.inputEl.addEventListener('input', () => debouncedRequantize());

    let lastHoveredIdx = -1;
    elements.chartArea.addEventListener('pointermove', (e) => {
        const idx = getNearestBarIdx(e.clientX);
        if (idx !== null && !isNaN(idx) && idx !== lastHoveredIdx) {
            lastHoveredIdx = idx;
            setHoveredIdx(idx);
        }
    });

    elements.chartArea.addEventListener('pointerleave', () => {
        lastHoveredIdx = -1;
        setHoveredIdx(null);
    });

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
            if (currIdx !== null && !isNaN(currIdx)) {
                setDragState(dragStartIdxTemp, currIdx);
            }
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

            const newHoverIdx = getNearestBarIdx(upEvt.clientX);
            setHoveredIdx(newHoverIdx);
            setDragState(null, null);
        };

        elements.chartArea.addEventListener('pointermove', onPointerMove);
        elements.chartArea.addEventListener('pointerup', onPointerUp);
    });

    elements.chartArea.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        resetZoom();
    });

    if (elements.dbToggleBtn) {
        elements.dbToggleBtn.addEventListener('click', () => {
            const isHidden = elements.dbToolsPanel.style.display === 'none';
            elements.dbToolsPanel.style.display = isHidden ? 'flex' : 'none';
            elements.dbToggleBtn.classList.toggle('open', isHidden);
        });
    }

    let pendingAction = null;

    if (elements.dbModeClip) {
        elements.dbModeClip.addEventListener('change', (e) => {
            if (elements.dbClipParams) {
                elements.dbClipParams.style.display = e.target.checked ? 'flex' : 'none';
            }

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
        const baseFloats = getBaseFloats();
        const N = baseFloats.length;
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

    let isDraggingLeft = false;
    let isDraggingRight = false;
    let isDraggingCenter = false;
    let initialClipStart = 0;
    let initialClipEnd = 0;
    let dragStartXRatio = 0;
    let hasMoved = false;
    let initialMouseX = 0;
    const DRAG_THRESHOLD = 4;

    const onDbPointerMove = (e) => {
        if (!isDraggingLeft && !isDraggingRight && !isDraggingCenter) return;

        if (!hasMoved) {
            if (Math.abs(e.clientX - initialMouseX) >= DRAG_THRESHOLD) hasMoved = true;
            else return;
        }

        const rect = elements.dbBar.getBoundingClientRect();
        const N = getBaseFloats().length;
        if (N === 0) return;

        const zS = zoomRange ? zoomRange.start : 0;
        const zE = zoomRange ? zoomRange.end : N - 1;
        const zCount = zE - zS + 1;
        let pxRatio = (e.clientX - rect.left) / rect.width;

        if (isDraggingCenter) {
            let deltaIdx = Math.round((pxRatio - dragStartXRatio) * (zCount - 1));
            if (initialClipStart + deltaIdx < 0) deltaIdx = -initialClipStart;
            if (initialClipEnd + deltaIdx > N - 1) deltaIdx = N - 1 - initialClipEnd;
            setClipRange(initialClipStart + deltaIdx, initialClipEnd + deltaIdx, true);
            return;
        }

        let targetIdx = Math.max(0, Math.min(N - 1, Math.round(zS + pxRatio * (zCount - 1))));
        const clip = getClipRange();

        if (isDraggingLeft) {
            setClipRange(Math.min(targetIdx, clip.end), clip.end, true);
        } else if (isDraggingRight) {
            setClipRange(clip.start, Math.max(targetIdx, clip.start), true);
        }
    };

    const onDbPointerUp = (e) => {
        if (isDraggingLeft || isDraggingRight || isDraggingCenter) {
            isDraggingLeft = false;
            isDraggingRight = false;
            isDraggingCenter = false;

            if (e.target.releasePointerCapture) e.target.releasePointerCapture(e.pointerId);
            e.target.removeEventListener('pointermove', onDbPointerMove);
            e.target.removeEventListener('pointerup', onDbPointerUp);

            if (hasMoved) {
                setUserModifiedClip(true);
                const clip = getClipRange();
                if (clip.end - clip.start + 1 > 1048576 && !getHasWarnedLargeData()) {
                    pendingAction = { type: 'slider', start: clip.start, end: clip.end };
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

    if (elements.dbActiveRegion) {
        elements.dbActiveRegion.addEventListener('pointerdown', (e) => {
            if (e.target === elements.dbHandleLeft || e.target === elements.dbHandleRight) return;
            e.preventDefault(); e.stopPropagation();
            elements.dbActiveRegion.setPointerCapture(e.pointerId);
            isDraggingCenter = true;
            hasMoved = false;
            initialMouseX = e.clientX;
            const clip = getClipRange();
            initialClipStart = clip.start;
            initialClipEnd = clip.end;
            dragStartXRatio = (e.clientX - elements.dbBar.getBoundingClientRect().left) / elements.dbBar.getBoundingClientRect().width;
            elements.dbActiveRegion.addEventListener('pointermove', onDbPointerMove);
            elements.dbActiveRegion.addEventListener('pointerup', onDbPointerUp);
        });

        elements.dbActiveRegion.addEventListener('dblclick', (e) => {
            e.preventDefault(); e.stopPropagation();
            const clip = getClipRange();
            if (clip.start !== null && clip.end !== null) setZoomRange(clip.start, clip.end);
        });
    }

    if (elements.dbHandleLeft) {
        elements.dbHandleLeft.addEventListener('pointerdown', (e) => {
            e.preventDefault(); e.stopPropagation();
            elements.dbHandleLeft.setPointerCapture(e.pointerId);
            isDraggingLeft = true;
            hasMoved = false;
            initialMouseX = e.clientX;
            const clip = getClipRange();
            initialClipStart = clip.start;
            initialClipEnd = clip.end;
            elements.dbHandleLeft.addEventListener('pointermove', onDbPointerMove);
            elements.dbHandleLeft.addEventListener('pointerup', onDbPointerUp);
        });
    }

    if (elements.dbHandleRight) {
        elements.dbHandleRight.addEventListener('pointerdown', (e) => {
            e.preventDefault(); e.stopPropagation();
            elements.dbHandleRight.setPointerCapture(e.pointerId);
            isDraggingRight = true;
            hasMoved = false;
            initialMouseX = e.clientX;
            const clip = getClipRange();
            initialClipStart = clip.start;
            initialClipEnd = clip.end;
            elements.dbHandleRight.addEventListener('pointermove', onDbPointerMove);
            elements.dbHandleRight.addEventListener('pointerup', onDbPointerUp);
        });
    }

    if (elements.btnModalBack) {
        elements.btnModalBack.addEventListener('click', () => {
            elements.modalLargeData.style.display = 'none';
            pendingAction = null;
        });
    }

    if (elements.btnModalOkay) {
        elements.btnModalOkay.addEventListener('click', () => {
            elements.modalLargeData.style.display = 'none';
            setHasWarnedLargeData(true);

            if (pendingAction) {
                if (pendingAction.type === 'slider') {
                    setClipRange(pendingAction.start, pendingAction.end, false);
                } else if (pendingAction.type === 'uncheck_clip') {
                    elements.dbModeClip.checked = false;
                    if (elements.dbClipParams) elements.dbClipParams.style.display = 'none';
                    setUserModifiedClip(false);
                    updateDbBarVisibility();
                    requantize(true);
                }
                pendingAction = null;
            } else {
                requantize(true);
            }
        });
    }

    applyPreset('Q4_0');
    elements.presetEl.value = 'Q4_0';
    updateUI();
    updateDbBarVisibility();
    generateData();
    debouncedRequantize(true);
});