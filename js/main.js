import './theme.js';
import { elements, initUIListeners, updateUI, applyPreset, populateDynamicSelectors } from './ui.js';
import { generateData } from './dataGen.js';
import { render, requantize, updateInspector, setZoomRange, resetZoom, toggleSRHT, getBaseFloats, getClipRange, setClipRange, getHasWarnedLargeData, setHasWarnedLargeData, drawDatasetBar, zoomRange, setUserModifiedClip, updateDbBarVisibility, updateVisualsOnly, setBackend } from './render.js';
import { initWasm, getWasm } from './wasmWrapper.js';

function getNearestBarIdx(clientX) {
    const bars = Array.from(elements.chartArea.querySelectorAll('.bar')).filter(b => b.style.display !== 'none');
    let closest = null;
    let minSub = Infinity;
    bars.forEach(b => {
        const r = b.getBoundingClientRect();
        const center = r.left + r.width / 2;
        const diff = Math.abs(center - clientX);
        if (diff < minSub) {
            minSub = diff;
            closest = b;
        }
    });
    return closest ? parseInt(closest.dataset.idx, 10) : null;
}

document.addEventListener('DOMContentLoaded', async () => {
    await initWasm();
    const wasm = getWasm();
    setBackend(new wasm.AppBackend());

    populateDynamicSelectors();
    initUIListeners();

    const processFile = (file) => {
        if (!file) return;
        const reader = new FileReader();
        reader.onload = (evt) => {
            elements.inputEl.value = evt.target.result;
            resetZoom(false);
            requantize();
        };
        reader.readAsText(file);
    };

    if (elements.fileDropZone && elements.fileInput) {
        elements.fileDropZone.addEventListener('click', () => elements.fileInput.click());

        elements.fileInput.addEventListener('change', (e) => {
            processFile(e.target.files[0]);
            e.target.value = ''; // Reset input
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
            requantize();
        } else if (e.target.id === 'centering-mode') {
            updateVisualsOnly();
        } else {
            if (e.target.id !== 'data-scale' && e.target.id !== 'data-offset') {
                elements.presetEl.value = 'custom';
                updateUI();
            }
            requantize();
        }
    }));

    elements.presetEl.addEventListener('change', () => {
        if (elements.presetEl.value !== 'custom') {
            applyPreset(elements.presetEl.value);
            updateUI();
            requantize();
        }
    });

    elements.genBtn.addEventListener('click', () => {
        generateData();
        resetZoom(false);
        requantize();
    });

    elements.inputEl.addEventListener('input', () => {
        requantize();
    });

    elements.chartArea.addEventListener('mouseover', (e) => {
        const bar = e.target.closest('.bar');
        if (!bar) return;
        const idx = parseInt(bar.dataset.idx, 10);
        if (!Number.isNaN(idx)) updateInspector(idx);
    });

    elements.btnResetZoom.addEventListener('click', () => resetZoom());
    elements.btnToggleSRHT.addEventListener('click', () => toggleSRHT());

    let dragStartIdx = null;

    elements.chartArea.addEventListener('mousedown', (e) => {
        if (e.button !== 0) return;
        e.preventDefault();

        let bar = e.target.closest('.bar');
        dragStartIdx = bar ? parseInt(bar.dataset.idx, 10) : getNearestBarIdx(e.clientX);
        if (dragStartIdx === null) return;

        const rect = elements.chartArea.getBoundingClientRect();
        const startX = e.clientX - rect.left;

        const dragBox = document.getElementById('zoom-box');
        if (!dragBox) return;
        dragBox.style.display = 'block';
        dragBox.style.left = `${startX}px`;
        dragBox.style.width = '0px';

        const onMouseMove = (moveEvt) => {
            const currX = Math.max(0, Math.min(rect.width, moveEvt.clientX - rect.left));
            dragBox.style.left = `${Math.min(startX, currX)}px`;
            dragBox.style.width = `${Math.abs(startX - currX)}px`;
        };

        const onMouseUp = (upEvt) => {
            document.removeEventListener('mousemove', onMouseMove);
            document.removeEventListener('mouseup', onMouseUp);
            dragBox.style.display = 'none';

            dragBox.style.visibility = 'hidden';
            const target = document.elementFromPoint(upEvt.clientX, upEvt.clientY);
            dragBox.style.visibility = 'visible';

            let barUp = target ? target.closest('.bar') : null;
            let endIdx = barUp ? parseInt(barUp.dataset.idx, 10) : getNearestBarIdx(upEvt.clientX);

            if (endIdx !== null && dragStartIdx !== null && endIdx !== dragStartIdx) {
                setZoomRange(Math.min(dragStartIdx, endIdx), Math.max(dragStartIdx, endIdx));
            }
            dragStartIdx = null;
        };

        document.addEventListener('mousemove', onMouseMove);
        document.addEventListener('mouseup', onMouseUp);
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
                e.target.checked = true; // revert visually
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

        let end = start + width - 1;
        if (end > N - 1) {
            end = N - 1;
        }

        const diff = end - start + 1;
        setUserModifiedClip(true);
        if (diff > 1048576 && !getHasWarnedLargeData()) {
            pendingAction = { type: 'slider', start: start, end: end };
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

    const onDbMouseMove = (e) => {
        if (!isDraggingLeft && !isDraggingRight && !isDraggingCenter) return;

        if (!hasMoved) {
            if (Math.abs(e.clientX - initialMouseX) >= DRAG_THRESHOLD) {
                hasMoved = true;
            } else {
                return;
            }
        }

        const rect = elements.dbBar.getBoundingClientRect();
        const baseFloats = getBaseFloats();
        const N = baseFloats.length;
        if (N === 0) return;

        let zS = zoomRange ? zoomRange.start : 0;
        let zE = zoomRange ? zoomRange.end : N - 1;
        const zCount = zE - zS + 1;

        let pxRatio = (e.clientX - rect.left) / rect.width;

        if (isDraggingCenter) {
            let deltaRatio = pxRatio - dragStartXRatio;
            let deltaIdx = Math.round(deltaRatio * (zCount - 1));

            if (initialClipStart + deltaIdx < 0) {
                deltaIdx = -initialClipStart;
            }
            if (initialClipEnd + deltaIdx > N - 1) {
                deltaIdx = N - 1 - initialClipEnd;
            }

            let newStart = initialClipStart + deltaIdx;
            let newEnd = initialClipEnd + deltaIdx;
            setClipRange(newStart, newEnd, true);
            return;
        }

        let targetIdx = Math.round(zS + pxRatio * (zCount - 1));
        targetIdx = Math.max(0, Math.min(N - 1, targetIdx));

        const clip = getClipRange();
        if (isDraggingLeft) {
            let newStart = Math.min(targetIdx, clip.end);
            setClipRange(newStart, clip.end, true);
        } else if (isDraggingRight) {
            let newEnd = Math.max(targetIdx, clip.start);
            setClipRange(clip.start, newEnd, true);
        }
    };

    const onDbMouseUp = (e) => {
        if (isDraggingLeft || isDraggingRight || isDraggingCenter) {
            isDraggingLeft = false;
            isDraggingRight = false;
            isDraggingCenter = false;
            document.removeEventListener('mousemove', onDbMouseMove);
            document.removeEventListener('mouseup', onDbMouseUp);

            if (hasMoved) {
                setUserModifiedClip(true);
                const clip = getClipRange();
                const diff = clip.end - clip.start + 1;
                if (diff > 1048576 && !getHasWarnedLargeData()) {
                    pendingAction = { type: 'slider', start: clip.start, end: clip.end };
                    setClipRange(initialClipStart, initialClipEnd, true); // revert visually until accepted
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
        elements.dbActiveRegion.addEventListener('mousedown', (e) => {
            if (e.target === elements.dbHandleLeft || e.target === elements.dbHandleRight) return;
            e.preventDefault(); e.stopPropagation();
            isDraggingCenter = true;
            hasMoved = false;
            initialMouseX = e.clientX;
            const clip = getClipRange();
            initialClipStart = clip.start;
            initialClipEnd = clip.end;
            const rect = elements.dbBar.getBoundingClientRect();
            dragStartXRatio = (e.clientX - rect.left) / rect.width;
            document.addEventListener('mousemove', onDbMouseMove);
            document.addEventListener('mouseup', onDbMouseUp);
        });

        elements.dbActiveRegion.addEventListener('dblclick', (e) => {
            e.preventDefault(); e.stopPropagation();
            const clip = getClipRange();
            if (clip.start !== null && clip.end !== null) {
                setZoomRange(clip.start, clip.end);
            }
        });
    }

    if (elements.dbHandleLeft) {
        elements.dbHandleLeft.addEventListener('mousedown', (e) => {
            e.preventDefault(); e.stopPropagation();
            isDraggingLeft = true;
            hasMoved = false;
            initialMouseX = e.clientX;
            const clip = getClipRange();
            initialClipStart = clip.start;
            initialClipEnd = clip.end;
            document.addEventListener('mousemove', onDbMouseMove);
            document.addEventListener('mouseup', onDbMouseUp);
        });
    }

    if (elements.dbHandleRight) {
        elements.dbHandleRight.addEventListener('mousedown', (e) => {
            e.preventDefault(); e.stopPropagation();
            isDraggingRight = true;
            hasMoved = false;
            initialMouseX = e.clientX;
            const clip = getClipRange();
            initialClipStart = clip.start;
            initialClipEnd = clip.end;
            document.addEventListener('mousemove', onDbMouseMove);
            document.addEventListener('mouseup', onDbMouseUp);
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
    requantize();
});