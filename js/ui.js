/**
 * UI State and Element Bindings
 * Centralizes DOM element lookups and UI state extraction.
 */
import { presetGroups, getPresetById } from './templates/presets.js';
import registry from './quants/registry.js';
import { drawDatasetBar } from './render/components.js'; // <-- Updated Import

export const elements = {
    // --- Data Input & Generation ---
    get inputEl() { return document.getElementById('float-input'); },
    get fileDropZone() { return document.getElementById('file-drop-zone'); },
    get fileInput() { return document.getElementById('file-input'); },
    get dataScaleEl() { return document.getElementById('data-scale'); },
    get dataOffsetEl() { return document.getElementById('data-offset'); },
    get genBtn() { return document.getElementById('gen-btn'); },
    get genCountEl() { return document.getElementById('gen-count'); },
    get distEl() { return document.getElementById('gen-dist'); },
    get genStdEl() { return document.getElementById('gen-std'); },
    get genScaleEl() { return document.getElementById('gen-scale'); },
    get genBimodalDistEl() { return document.getElementById('gen-bimodal-dist'); },
    get genBimodalSpreadEl() { return document.getElementById('gen-bimodal-spread'); },
    get genOutlierProbEl() { return document.getElementById('gen-outlier-prob'); },
    get genOutlierMultEl() { return document.getElementById('gen-outlier-mult'); },
    get genUniRangeEl() { return document.getElementById('gen-uni-range'); },
    get advToggleBtn() { return document.getElementById('gen-adv-toggle'); },
    get advPanel() { return document.getElementById('gen-adv-panel'); },
    get rawAdvToggleBtn() { return document.getElementById('raw-adv-toggle'); },
    get rawAdvPanel() { return document.getElementById('raw-adv-panel'); },
    get dataTransformToggle() { return document.getElementById('data-transform-toggle'); },
    get dataTransformPanel() { return document.getElementById('data-transform-panel'); },

    // --- Core Quantization Settings ---
    get modeEl() { return document.getElementById('centering-mode'); },
    get qTypeEl() { return document.getElementById('quant-type'); },
    get qBitsEl() { return document.getElementById('quant-bits'); },
    get presetEl() { return document.getElementById('preset-select'); },

    // --- Chart Y-Axis Settings ---
    get axisToggleBtn() { return document.getElementById('chart-axis-toggle'); },
    get axisPanel() { return document.getElementById('chart-axis-panel'); },
    get axisOutliersWrap() { return document.getElementById('axis-outliers-wrap'); },
    get axisIgnoreOutliers() { return document.getElementById('axis-ignore-outliers'); },
    get axisOutlierPctWrap() { return document.getElementById('axis-outlier-pct-wrap'); },
    get axisOutlierPct() { return document.getElementById('axis-outlier-pct'); },
    get axisManualWrap() { return document.getElementById('axis-manual-wrap'); },
    get axisManualMin() { return document.getElementById('axis-manual-min'); },
    get axisManualMax() { return document.getElementById('axis-manual-max'); },

    // --- Chart & Visualization ---
    get chartArea() { return document.getElementById('chart-area'); },
    get formulaBox() { return document.getElementById('equation-box'); },
    get quantStats() { return document.getElementById('quant-stats'); },
    get chartMaxLbl() { return document.getElementById('chart-max-lbl'); },
    get chartMinLbl() { return document.getElementById('chart-min-lbl'); },
    get btnResetZoom() { return document.getElementById('btn-reset-zoom'); },
    get btnToggleSRHT() { return document.getElementById('btn-toggle-srht'); },

    // --- Inspector ---
    get insWData() { return document.getElementById('ins-w-data'); },
    get insBData() { return document.getElementById('ins-b-data'); },
    get insSBData() { return document.getElementById('ins-sb-data'); },
    get iWIdx() { return document.getElementById('ins-w-idx'); },
    get iBIdx() { return document.getElementById('ins-b-idx'); },
    get iSBIdx() { return document.getElementById('ins-sb-idx'); },

    // --- Block Settings ---
    get blockSettings() { return document.getElementById('block-settings'); },
    get blockSizeEl() { return document.getElementById('block-size'); },
    get sbSizeEl() { return document.getElementById('superblock-size'); },
    get subSizeEl() { return document.getElementById('subblock-size'); },
    get subBitsEl() { return document.getElementById('subblock-bits'); },
    get subOffsetEl() { return document.getElementById('subblock-offset'); },

    // --- Turbo Quantization Settings ---
    get turboSettings() { return document.getElementById('turbo-settings'); },
    get turboBitsEl() { return document.getElementById('turbo-bits'); },
    get turboBlockSizeEl() { return document.getElementById('turbo-block-size'); },
    get turboWhtEl() { return document.getElementById('turbo-wht'); },
    get turboQjlEl() { return document.getElementById('turbo-qjl'); },
    get turboSignSeedEl() { return document.getElementById('turbo-sign-seed'); },
    get turboAdvToggle() { return document.getElementById('turbo-adv-toggle'); },
    get turboAdvPanel() { return document.getElementById('turbo-adv-panel'); },

    // --- Trellis Settings ---
    get trellisSettings() { return document.getElementById('trellis-settings'); },
    get trellisBitsEl() { return document.getElementById('trellis-bits'); },
    get trellisBlockSizeEl() { return document.getElementById('trellis-block-size'); },
    get trellisStatesEl() { return document.getElementById('trellis-states'); },
    get trellisCbTypeEl() { return document.getElementById('trellis-cb-type'); },
    get trellisWhtEl() { return document.getElementById('trellis-wht'); },
    get trellisWhtScopeEl() { return document.getElementById('trellis-wht-scope'); },
    get trellisOptItersEl() { return document.getElementById('trellis-opt-iters'); },
    get trellisSignSeedEl() { return document.getElementById('trellis-sign-seed'); },
    get trellisAdvToggle() { return document.getElementById('trellis-adv-toggle'); },
    get trellisAdvPanel() { return document.getElementById('trellis-adv-panel'); },

    // --- Other Formats ---
    get mxfpSettings() { return document.getElementById('mxfp-settings'); },
    get mxfpFormatEl() { return document.getElementById('mxfp-format'); },
    get primitiveSettings() { return document.getElementById('primitive-settings'); },
    get primitiveFormatEl() { return document.getElementById('primitive-format'); },
    get kquantSettings() { return document.getElementById('kquant-settings'); },

    // --- Layout Elements ---
    get toggleLeft() { return document.getElementById('toggle-left'); },
    get toggleRight() { return document.getElementById('toggle-right'); },
    get wrapperLeft() { return document.getElementById('panel-left-wrapper'); },
    get wrapperRight() { return document.getElementById('panel-right-wrapper'); },
    get cardBlock() { return document.getElementById('card-block'); },
    get cardSuper() { return document.getElementById('card-super'); },

    // --- Dataset Mini-map & Tooling ---
    get dbToggleBtn() { return document.getElementById('btn-dataset-bar'); },
    get dbToolsPanel() { return document.getElementById('db-tools-panel'); },
    get dbModeClip() { return document.getElementById('db-mode-clip'); },
    get dbClipParams() { return document.getElementById('db-clip-params'); },
    get dbClipStart() { return document.getElementById('db-clip-start'); },
    get dbClipWidth() { return document.getElementById('db-clip-width'); },
    get dbModeMinMax() { return document.getElementById('db-mode-minmax'); },
    get dbModeHotspots() { return document.getElementById('db-mode-hotspots'); },
    get dbContainer() { return document.getElementById('dataset-bar-wrapper'); },
    get dbBar() { return document.getElementById('dataset-bar'); },
    get dbHandleLeft() { return document.getElementById('db-handle-left'); },
    get dbHandleRight() { return document.getElementById('db-handle-right'); },
    get dbActiveRegion() { return document.getElementById('db-active-region'); },
    get dbCanvas() { return document.getElementById('db-canvas'); },
    get dbMinArrow() { return document.getElementById('db-min-arrow'); },
    get dbMaxArrow() { return document.getElementById('db-max-arrow'); },

    // --- Modals ---
    get modalLargeData() { return document.getElementById('large-data-modal'); },
    get btnModalBack() { return document.getElementById('btn-modal-back'); },
    get btnModalOkay() { return document.getElementById('btn-modal-okay'); },

    /**
     * Array of inputs that automatically trigger a render/requantization when changed.
     */
    get autoUpdateElements() {
        return [
            this.modeEl, this.qTypeEl, this.qBitsEl,
            this.blockSizeEl, this.sbSizeEl, this.subSizeEl, this.subBitsEl,
            this.subOffsetEl, this.distEl, this.genStdEl,
            this.genScaleEl, this.genBimodalDistEl, this.genBimodalSpreadEl,
            this.genOutlierProbEl, this.genOutlierMultEl, this.genUniRangeEl,
            this.turboBitsEl, this.turboBlockSizeEl, this.turboWhtEl, this.turboQjlEl,
            this.turboSignSeedEl,
            this.trellisBitsEl, this.trellisBlockSizeEl, this.trellisStatesEl, this.trellisCbTypeEl,
            this.trellisWhtEl, this.trellisWhtScopeEl, this.trellisOptItersEl, this.trellisSignSeedEl,
            this.mxfpFormatEl, this.primitiveFormatEl, this.dataScaleEl, this.dataOffsetEl,
            this.axisIgnoreOutliers, this.axisOutlierPct, this.axisManualMin, this.axisManualMax
        ].filter(el => el !== null);
    }
};

/**
 * Populates dropdown selects from the quantization registry and preset templates.
 */
export function populateDynamicSelectors() {
    elements.qTypeEl.innerHTML = '';
    Object.entries(registry).forEach(([id, quant]) => {
        const opt = document.createElement('option');
        opt.value = id;
        opt.textContent = quant.label;
        elements.qTypeEl.appendChild(opt);
    });

    const presetSelect = elements.presetEl;
    presetSelect.innerHTML = '<option value="custom">-- Custom --</option>';
    presetGroups.forEach(group => {
        const optGroup = document.createElement('optgroup');
        optGroup.label = group.label;
        Object.entries(group.presets).forEach(([id, config]) => {
            const opt = document.createElement('option');
            opt.value = id;
            opt.textContent = config.label || id;
            optGroup.appendChild(opt);
        });
        presetSelect.appendChild(optGroup);
    });
}

/**
 * Applies a given preset configuration to the UI inputs.
 */
export function applyPreset(presetId) {
    const p = getPresetById(presetId);
    if (!p) return;
    for (const [id, val] of Object.entries(p)) {
        if (id === 'label') continue;
        const el = document.getElementById(id);
        if (!el) continue;
        if (el.type === 'checkbox') el.checked = val;
        else el.value = val;
    }
}

/**
 * Utility hooks allowing individual quantizers to toggle relevant UI sections.
 */
export const uiHelpers = {
    showBlockSettings: (show) => elements.blockSettings.style.display = show ? 'flex' : 'none',
    showKQuantSettings: (show) => elements.kquantSettings.style.display = show ? 'flex' : 'none',
    showTurboSettings: (show) => elements.turboSettings.style.display = show ? 'flex' : 'none',
    showTrellisSettings: (show) => { if (elements.trellisSettings) elements.trellisSettings.style.display = show ? 'flex' : 'none'; },
    showMxfpSettings: (show) => { if (elements.mxfpSettings) elements.mxfpSettings.style.display = show ? 'flex' : 'none'; },
    showPrimitiveSettings: (show) => { if (elements.primitiveSettings) elements.primitiveSettings.style.display = show ? 'flex' : 'none'; },
    showQuantBits: (show) => elements.qBitsEl.parentElement.style.display = show ? 'flex' : 'none',
    showSuperBlockCard: (show, title = 'Super-Block stats') => {
        elements.cardSuper.style.display = show ? 'flex' : 'none';
        const h4 = elements.cardSuper.querySelector('h4');
        if (h4 && h4.childNodes.length > 0) h4.childNodes[0].nodeValue = title + ' ';
    },
    showBlockCard: (show, title = 'Sub-Block stats') => {
        if (elements.cardBlock) {
            elements.cardBlock.style.display = show ? 'flex' : 'none';
            const h4 = elements.cardBlock.querySelector('h4');
            if (h4 && h4.childNodes.length > 0) h4.childNodes[0].nodeValue = title + ' ';
        }
    },
};

/**
 * Reads all current states from the UI inputs.
 */
export function getSettings() {
    return {
        qType: elements.qTypeEl.value,
        weightBits: parseInt(elements.qBitsEl.value) || 4,
        blockSize: parseInt(elements.blockSizeEl.value) || 32,
        sbSize: parseInt(elements.sbSizeEl.value) || 256,
        subSize: parseInt(elements.subSizeEl.value) || 32,
        subBits: parseInt(elements.subBitsEl.value) || 6,
        hasOffset: elements.subOffsetEl.checked,

        turboBits: parseInt(elements.turboBitsEl.value) || 4,
        turboBlockSize: parseInt(elements.turboBlockSizeEl.value) || 128,
        useWht: elements.turboWhtEl.checked,
        useQjl: elements.turboQjlEl.checked,
        turboSignSeed: parseInt(elements.turboSignSeedEl?.value) || 42,

        trellisBits: parseInt(elements.trellisBitsEl.value) || 4,
        trellisBlockSize: parseInt(elements.trellisBlockSizeEl.value) || 64,
        trellisStates: parseInt(elements.trellisStatesEl.value) || 4,
        trellisCbType: elements.trellisCbTypeEl.value,
        trellisUseWht: elements.trellisWhtEl?.checked || false,
        trellisWhtScope: elements.trellisWhtScopeEl?.value || 'global',
        trellisOptIters: parseInt(elements.trellisOptItersEl?.value) || 0,
        trellisSignSeed: parseInt(elements.trellisSignSeedEl?.value) || 42,

        mxfpFormat: elements.mxfpFormatEl?.value || 'mxfp4_e2m1',
        primitiveFormat: elements.primitiveFormatEl?.value || 'fp32',

        centeringMode: elements.modeEl.value,
        axisIgnoreOutliers: elements.axisIgnoreOutliers ? elements.axisIgnoreOutliers.checked : false,
        axisOutlierPct: elements.axisOutlierPct ? (parseFloat(elements.axisOutlierPct.value) || 0.0) : 0.0,
        axisManualMin: elements.axisManualMin ? (parseFloat(elements.axisManualMin.value) || 0.0) : 0.0,
        axisManualMax: elements.axisManualMax ? (parseFloat(elements.axisManualMax.value) || 0.0) : 0.0
    };
}

/**
 * Triggers UI updates based on the currently selected quantizer type.
 */
export function updateUI() {
    const qType = elements.qTypeEl.value;
    const quant = registry[qType];

    if (elements.trellisSettings) elements.trellisSettings.style.display = 'none';
    if (elements.mxfpSettings) elements.mxfpSettings.style.display = 'none';
    if (elements.primitiveSettings) elements.primitiveSettings.style.display = 'none';
    if (elements.cardBlock) elements.cardBlock.style.display = 'flex';

    if (quant && quant.setupUI) {
        quant.setupUI(uiHelpers);
    }
}

/**
 * Initializes generic UI interactions (panels, dist options).
 */
export function initUIListeners() {
    elements.toggleLeft.addEventListener('click', () => {
        elements.wrapperLeft.classList.toggle('collapsed');
        const isCollapsed = elements.wrapperLeft.classList.contains('collapsed');
        elements.toggleLeft.querySelector('svg').style.transform = isCollapsed ? 'rotate(180deg)' : 'rotate(0deg)';
    });

    elements.toggleRight.addEventListener('click', () => {
        elements.wrapperRight.classList.toggle('collapsed');
        const isCollapsed = elements.wrapperRight.classList.contains('collapsed');
        elements.toggleRight.querySelector('svg').style.transform = isCollapsed ? 'rotate(180deg)' : 'rotate(0deg)';
    });

    elements.genCountEl.addEventListener('keydown', (e) => {
        let val = parseInt(elements.genCountEl.value) || 1;
        if (e.key === 'ArrowUp') {
            e.preventDefault(); elements.genCountEl.value = val * 2;
        } else if (e.key === 'ArrowDown') {
            e.preventDefault(); elements.genCountEl.value = Math.max(1, Math.floor(val / 2));
        }
    });

    const setupToggle = (btn, panel) => {
        if (!btn || !panel) return;
        btn.addEventListener('click', () => {
            const isHidden = panel.style.display === 'none';
            panel.style.display = isHidden ? 'flex' : 'none';
            btn.classList.toggle('open', isHidden);
        });
    };

    setupToggle(elements.advToggleBtn, elements.advPanel);
    setupToggle(elements.rawAdvToggleBtn, elements.rawAdvPanel);
    setupToggle(elements.turboAdvToggle, elements.turboAdvPanel);
    setupToggle(elements.trellisAdvToggle, elements.trellisAdvPanel);
    setupToggle(elements.axisToggleBtn, elements.axisPanel);
    setupToggle(elements.dataTransformToggle, elements.dataTransformPanel);

    const updateAxisUI = () => {
        if (!elements.modeEl) return;
        const mode = elements.modeEl.value;
        if (mode === 'manual') {
            elements.axisOutliersWrap.style.display = 'none';
            elements.axisManualWrap.style.display = 'flex';
        } else {
            elements.axisOutliersWrap.style.display = 'flex';
            elements.axisManualWrap.style.display = 'none';
            elements.axisOutlierPctWrap.style.display = elements.axisIgnoreOutliers.checked ? 'flex' : 'none';
        }
    };
    if (elements.modeEl) elements.modeEl.addEventListener('change', updateAxisUI);
    if (elements.axisIgnoreOutliers) elements.axisIgnoreOutliers.addEventListener('change', updateAxisUI);
    updateAxisUI();

    elements.distEl.addEventListener('change', (e) => {
        document.querySelectorAll('.dist-params').forEach(el => el.style.display = 'none');
        const targetParams = document.getElementById(`param-${e.target.value}`);
        if (targetParams) targetParams.style.display = 'flex';
    });
}