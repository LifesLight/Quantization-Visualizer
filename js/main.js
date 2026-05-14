import './theme.js';
import { elements, initUIListeners, updateUI, applyPreset, populateDynamicSelectors } from './ui.js';
import { generateData } from './dataGen.js';
import { render, updateInspector } from './render.js';

document.addEventListener('DOMContentLoaded', () => {
    populateDynamicSelectors();
    initUIListeners();

    elements.autoUpdateElements.forEach(el => el.addEventListener('change', (e) => {
        if (e.target.id.startsWith('gen-')) {
            generateData();
            render();
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
        render();
    });

    elements.inputEl.addEventListener('input', () => render());

    elements.chartArea.addEventListener('mouseover', (e) => {
        const bar = e.target.closest('.bar');
        if (!bar) return;
        const idx = parseInt(bar.dataset.idx, 10);
        if (!Number.isNaN(idx)) updateInspector(idx);
    });

    applyPreset('Q4_0');
    elements.presetEl.value = 'Q4_0';
    updateUI();
    generateData();
    render();
});