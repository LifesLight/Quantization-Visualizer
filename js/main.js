import './theme.js';
import { elements, initUIListeners, updateUI, applyPreset, populateDynamicSelectors } from './ui.js';
import { generateData } from './dataGen.js';
import { render, updateInspector, setZoomRange, resetZoom, toggleSRHT } from './render.js';

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

document.addEventListener('DOMContentLoaded', () => {
    populateDynamicSelectors();
    initUIListeners();

    const processFile = (file) => {
        if (!file) return;
        const reader = new FileReader();
        reader.onload = (evt) => {
            const content = evt.target.result;
            const floats = content.match(/-?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?/g);
            if (floats && floats.length > 0) {
                elements.inputEl.value = floats.join(', ');
                resetZoom();
            } else {
                alert("No valid numbers found in the file.");
            }
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
            resetZoom();
        } else {
            elements.presetEl.value = 'custom';
            updateUI();
            render();
        }
    }));

    elements.presetEl.addEventListener('change', () => {
        if (elements.presetEl.value !== 'custom') {
            applyPreset(elements.presetEl.value);
            updateUI();
            render();
        }
    });

    elements.genBtn.addEventListener('click', () => {
        generateData();
        resetZoom();
    });

    elements.inputEl.addEventListener('input', () => {
        render();
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

    applyPreset('Q4_0');
    elements.presetEl.value = 'Q4_0';
    updateUI();
    generateData();
    render();
});