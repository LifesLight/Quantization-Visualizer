/**
 * Standard Application Events
 * Wires up form inputs, file drops, window resizing, and warning modals.
 */
import { elements, applyPreset, updateUI, validateImportance } from '../ui.js';
import { generateData, generateImportanceData } from '../dataGen.js';
import { debouncedRequantize, requantize, render, updateVisualsOnly } from '../render/core.js';
import { resetZoom, setClipRange } from '../render/actions.js';
import { resizeCanvasCssOnly, updateOverlays, updateDbBarVisibility } from '../render/components.js';
import { setHasWarnedLargeData, setUserModifiedClip, getBaseFloats, getImportanceFloats } from '../render/state.js';
import { pendingAction, clearPendingAction } from './interactions.js';

window.addEventListener('dragover', (e) => e.preventDefault(), false);
window.addEventListener('drop', (e) => e.preventDefault(), false);

export function setupResizeObserver() {
    let resizeRaf = null;
    let resizeFinalDebounce = null;

    const resizeObserver = new ResizeObserver(() => {
        if (resizeRaf) cancelAnimationFrame(resizeRaf);
        resizeRaf = requestAnimationFrame(() => {
            resizeCanvasCssOnly();
            updateOverlays();
        });

        clearTimeout(resizeFinalDebounce);
        resizeFinalDebounce = setTimeout(() => render({ updateInspector: false }), 180);
    });
    resizeObserver.observe(elements.chartArea);
}

export function setupFileHandling() {
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
            e.stopPropagation();
            elements.fileDropZone.style.borderColor = 'var(--accent-color, #007bff)';
            elements.fileDropZone.style.background = 'rgba(128, 128, 128, 0.1)';
        });
        elements.fileDropZone.addEventListener('dragleave', (e) => {
            e.preventDefault();
            e.stopPropagation();
            elements.fileDropZone.style.borderColor = 'var(--border-color, #555)';
            elements.fileDropZone.style.background = 'transparent';
        });
        elements.fileDropZone.addEventListener('drop', (e) => {
            e.preventDefault();
            e.stopPropagation();
            elements.fileDropZone.style.borderColor = 'var(--border-color, #555)';
            elements.fileDropZone.style.background = 'transparent';
            if (e.dataTransfer.files && e.dataTransfer.files.length > 0) processFile(e.dataTransfer.files[0]);
        });
    }

    const processImpFile = (file) => {
        if (!file) return;
        const reader = new FileReader();
        reader.onload = (evt) => {
            elements.inputImportanceEl.value = evt.target.result;
            if (elements.useImportance.checked || elements.useImportance.disabled === false) {
                debouncedRequantize(true);
            } else {
                const N = getBaseFloats().length;
                const impFloats = getImportanceFloats();
                validateImportance(N, impFloats.length, elements.inputImportanceEl.value);
            }
        };
        reader.readAsText(file);
    };

    if (elements.impFileDropZone && elements.impFileInput) {
        elements.impFileDropZone.addEventListener('click', () => elements.impFileInput.click());
        elements.impFileInput.addEventListener('change', (e) => {
            processImpFile(e.target.files[0]);
            e.target.value = '';
        });
        elements.impFileDropZone.addEventListener('dragover', (e) => {
            e.preventDefault();
            e.stopPropagation();
            elements.impFileDropZone.style.borderColor = 'var(--accent-color, #007bff)';
            elements.impFileDropZone.style.background = 'rgba(128, 128, 128, 0.1)';
        });
        elements.impFileDropZone.addEventListener('dragleave', (e) => {
            e.preventDefault();
            e.stopPropagation();
            elements.impFileDropZone.style.borderColor = 'var(--border-color, #555)';
            elements.impFileDropZone.style.background = 'transparent';
        });
        elements.impFileDropZone.addEventListener('drop', (e) => {
            e.preventDefault();
            e.stopPropagation();
            elements.impFileDropZone.style.borderColor = 'var(--border-color, #555)';
            elements.impFileDropZone.style.background = 'transparent';
            if (e.dataTransfer.files && e.dataTransfer.files.length > 0) processImpFile(e.dataTransfer.files[0]);
        });
    }
}

export function setupAutoUpdateListeners() {
    elements.autoUpdateElements.forEach(el => el.addEventListener('change', (e) => {
        if (e.target.id.startsWith('gen-')) {
            generateData();
            resetZoom(false);
            debouncedRequantize();
        } else if (e.target.id === 'centering-mode' || e.target.id.startsWith('axis-')) {
            updateVisualsOnly();
        } else {
            if (e.target.id !== 'data-scale' && e.target.id !== 'data-offset' && e.target.id !== 'use-importance') {
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

    if (elements.impGenBtn) {
        elements.impGenBtn.addEventListener('click', () => {
            generateImportanceData();
            debouncedRequantize();
        });
    }

    elements.inputEl.addEventListener('input', () => debouncedRequantize());
    if (elements.inputImportanceEl) {
        elements.inputImportanceEl.addEventListener('input', () => {
            if (elements.useImportance.checked) {
                debouncedRequantize();
            } else {
                const N = getBaseFloats().length;
                const impFloats = getImportanceFloats();
                validateImportance(N, impFloats.length, elements.inputImportanceEl.value);
            }
        });
    }
}

export function setupModals() {
    if (elements.btnModalBack) {
        elements.btnModalBack.addEventListener('click', () => {
            elements.modalLargeData.style.display = 'none';
            clearPendingAction();
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
                clearPendingAction();
            } else {
                requantize(true);
            }
        });
    }
}