/**
 * Main Initialization Orchestrator
 * Bootstraps WebAssembly, binds UI events, and triggers the initial render.
 */
import './theme.js';
import { elements, initUIListeners, updateUI, applyPreset, populateDynamicSelectors } from './ui.js';
import { generateData } from './dataGen.js';
import { initWasm, getWasm } from './wasmWrapper.js';

// Setup Event Bindings
import { setupChartInteractions, setupDatasetBarInteractions } from './events/interactions.js';
import { setupResizeObserver, setupFileHandling, setupAutoUpdateListeners, setupModals } from './events/setup.js';

// Core State & Logic
import { setBackend } from './render/state.js';
import { debouncedRequantize } from './render/core.js';
import { updateDbBarVisibility } from './render/components.js';

document.addEventListener('DOMContentLoaded', async () => {
    // 1. Initialize WASM Backend
    await initWasm();
    const wasm = getWasm();
    setBackend(new wasm.AppBackend());

    // 2. Setup Base UI Dropdowns
    populateDynamicSelectors();
    initUIListeners();

    // 3. Attach Render & Component Listeners
    setupResizeObserver();
    setupFileHandling();
    setupAutoUpdateListeners();
    setupChartInteractions();
    setupDatasetBarInteractions();
    setupModals();

    // 4. Set Initial Defaults & Run First Quantization
    applyPreset('Q4_0');
    elements.presetEl.value = 'Q4_0';
    updateUI();
    updateDbBarVisibility();
    generateData();

    // Force a synchronous initial render pipeline to avoid popping
    debouncedRequantize(true);
});