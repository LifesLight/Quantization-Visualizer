import { presetGroups, getPresetById } from './templates/presets.js';
import registry from './quants/registry.js';

export const elements = {
    get inputEl() { return document.getElementById('float-input'); },
    get modeEl() { return document.getElementById('centering-mode'); },
    get qTypeEl() { return document.getElementById('quant-type'); },
    get qBitsEl() { return document.getElementById('quant-bits'); },
    get presetEl() { return document.getElementById('preset-select'); },
    get chartArea() { return document.getElementById('chart-area'); },
    get formulaBox() { return document.getElementById('equation-box'); },
    get distEl() { return document.getElementById('gen-dist'); },
    get advToggleBtn() { return document.getElementById('gen-adv-toggle'); },
    get advPanel() { return document.getElementById('gen-adv-panel'); },
    get genCountEl() { return document.getElementById('gen-count'); },
    get genBtn() { return document.getElementById('gen-btn'); },
    get genBiasEl() { return document.getElementById('gen-bias'); },
    get genStdEl() { return document.getElementById('gen-std'); },
    get genScaleEl() { return document.getElementById('gen-scale'); },
    get genBimodalDistEl() { return document.getElementById('gen-bimodal-dist'); },
    get genBimodalSpreadEl() { return document.getElementById('gen-bimodal-spread'); },
    get genOutlierProbEl() { return document.getElementById('gen-outlier-prob'); },
    get genOutlierMultEl() { return document.getElementById('gen-outlier-mult'); },
    get genUniRangeEl() { return document.getElementById('gen-uni-range'); },
    get insWData() { return document.getElementById('ins-w-data'); },
    get insBData() { return document.getElementById('ins-b-data'); },
    get insSBData() { return document.getElementById('ins-sb-data'); },
    get iWIdx() { return document.getElementById('ins-w-idx'); },
    get iBIdx() { return document.getElementById('ins-b-idx'); },
    get iSBIdx() { return document.getElementById('ins-sb-idx'); },
    get blockSizeEl() { return document.getElementById('block-size'); },
    get sbSizeEl() { return document.getElementById('superblock-size'); },
    get subSizeEl() { return document.getElementById('subblock-size'); },
    get subBitsEl() { return document.getElementById('subblock-bits'); },
    get subOffsetEl() { return document.getElementById('subblock-offset'); },
    get turboBitsEl() { return document.getElementById('turbo-bits'); },
    get turboBlockSizeEl() { return document.getElementById('turbo-block-size'); },
    get turboWhtEl() { return document.getElementById('turbo-wht'); },
    get turboQjlEl() { return document.getElementById('turbo-qjl'); },
    get trellisSettings() { return document.getElementById('trellis-settings'); },
    get trellisBitsEl() { return document.getElementById('trellis-bits'); },
    get trellisBlockSizeEl() { return document.getElementById('trellis-block-size'); },
    get trellisStatesEl() { return document.getElementById('trellis-states'); },
    get trellisCbTypeEl() { return document.getElementById('trellis-cb-type'); },
    get trellisWhtEl() { return document.getElementById('trellis-wht'); },
    get nvfp4Settings() { return document.getElementById('nvfp4-settings'); },
    get nvfp4TensorSizeEl() { return document.getElementById('nvfp4-tensor-size'); },
    get mxfpSettings() { return document.getElementById('mxfp-settings'); },
    get mxfpFormatEl() { return document.getElementById('mxfp-format'); },
    get primitiveSettings() { return document.getElementById('primitive-settings'); },
    get primitiveFormatEl() { return document.getElementById('primitive-format'); },
    get toggleLeft() { return document.getElementById('toggle-left'); },
    get toggleRight() { return document.getElementById('toggle-right'); },
    get wrapperLeft() { return document.getElementById('panel-left-wrapper'); },
    get wrapperRight() { return document.getElementById('panel-right-wrapper'); },
    get blockSettings() { return document.getElementById('block-settings'); },
    get kquantSettings() { return document.getElementById('kquant-settings'); },
    get turboSettings() { return document.getElementById('turbo-settings'); },
    get cardBlock() { return document.getElementById('card-block'); },
    get cardSuper() { return document.getElementById('card-super'); },
    get quantStats() { return document.getElementById('quant-stats'); },
    get chartMaxLbl() { return document.getElementById('chart-max-lbl'); },
    get chartMinLbl() { return document.getElementById('chart-min-lbl'); },
    get btnResetZoom() { return document.getElementById('btn-reset-zoom'); },
    get btnToggleSRHT() { return document.getElementById('btn-toggle-srht'); },
    get autoUpdateElements() {
        return [
            this.modeEl, this.qTypeEl, this.qBitsEl,
            this.blockSizeEl, this.sbSizeEl, this.subSizeEl, this.subBitsEl,
            this.subOffsetEl, this.distEl, this.genBiasEl, this.genStdEl,
            this.genScaleEl, this.genBimodalDistEl, this.genBimodalSpreadEl,
            this.genOutlierProbEl, this.genOutlierMultEl, this.genUniRangeEl,
            this.turboBitsEl, this.turboBlockSizeEl, this.turboWhtEl, this.turboQjlEl,
            this.trellisBitsEl, this.trellisBlockSizeEl, this.trellisStatesEl, this.trellisCbTypeEl,
            this.trellisWhtEl, this.nvfp4TensorSizeEl, this.mxfpFormatEl, this.primitiveFormatEl
        ].filter(el => el !== null && el !== undefined);
    }
};

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

export const uiHelpers = {
    showBlockSettings: (show) => elements.blockSettings.style.display = show ? 'flex' : 'none',
    showKQuantSettings: (show) => elements.kquantSettings.style.display = show ? 'flex' : 'none',
    showTurboSettings: (show) => elements.turboSettings.style.display = show ? 'flex' : 'none',
    showTrellisSettings: (show) => { if (elements.trellisSettings) elements.trellisSettings.style.display = show ? 'flex' : 'none'; },
    showNvfp4Settings: (show) => { if (elements.nvfp4Settings) elements.nvfp4Settings.style.display = show ? 'flex' : 'none'; },
    showMxfpSettings: (show) => { if (elements.mxfpSettings) elements.mxfpSettings.style.display = show ? 'flex' : 'none'; },
    showPrimitiveSettings: (show) => { if (elements.primitiveSettings) elements.primitiveSettings.style.display = show ? 'flex' : 'none'; },
    showQuantBits: (show) => elements.qBitsEl.parentElement.style.display = show ? 'flex' : 'none',
    showSuperBlockCard: (show, title = 'Super-Block stats') => {
        elements.cardSuper.style.display = show ? 'flex' : 'none';
        const h4 = elements.cardSuper.querySelector('h4');
        if (h4 && h4.childNodes.length > 0) {
            h4.childNodes[0].nodeValue = title + ' ';
        }
    },
    showBlockCard: (show, title = 'Sub-Block stats') => {
        if (elements.cardBlock) {
            elements.cardBlock.style.display = show ? 'flex' : 'none';
            const h4 = elements.cardBlock.querySelector('h4');
            if (h4 && h4.childNodes.length > 0) {
                h4.childNodes[0].nodeValue = title + ' ';
            }
        }
    },
};

export function getSettings() {
    return {
        qType: elements.qTypeEl.value,
        weightBits: Math.max(1, Math.min(8, parseInt(elements.qBitsEl.value) || 4)),
        blockSize: parseInt(elements.blockSizeEl.value) || 32,
        sbSize: parseInt(elements.sbSizeEl.value) || 256,
        subSize: parseInt(elements.subSizeEl.value) || 32,
        subBits: parseInt(elements.subBitsEl.value) || 6,
        hasOffset: elements.subOffsetEl.checked,
        turboBits: parseInt(elements.turboBitsEl.value) || 4,
        turboBlockSize: parseInt(elements.turboBlockSizeEl.value) || 128,
        useWht: elements.turboWhtEl.checked,
        useQjl: elements.turboQjlEl.checked,
        trellisBits: parseInt(elements.trellisBitsEl.value) || 4,
        trellisBlockSize: parseInt(elements.trellisBlockSizeEl.value) || 64,
        trellisStates: parseInt(elements.trellisStatesEl.value) || 4,
        trellisCbType: elements.trellisCbTypeEl.value,
        trellisUseWht: elements.trellisWhtEl ? elements.trellisWhtEl.checked : false,
        nvfp4TensorSize: parseInt(elements.nvfp4TensorSizeEl?.value) || 256,
        mxfpFormat: elements.mxfpFormatEl ? elements.mxfpFormatEl.value : 'mxfp4_e2m1',
        primitiveFormat: elements.primitiveFormatEl ? elements.primitiveFormatEl.value : 'fp32',
        centeringMode: elements.modeEl.value
    };
}

export function updateUI() {
    const qType = elements.qTypeEl.value;
    const quant = registry[qType];

    // Auto-hide specific panels to prevent layout breaks when navigating to other schemes
    if (elements.trellisSettings) elements.trellisSettings.style.display = 'none';
    if (elements.nvfp4Settings) elements.nvfp4Settings.style.display = 'none';
    if (elements.mxfpSettings) elements.mxfpSettings.style.display = 'none';
    if (elements.primitiveSettings) elements.primitiveSettings.style.display = 'none';

    // Auto-show standard block layout stats as it maps to the majority of conventional quants
    if (elements.cardBlock) {
        elements.cardBlock.style.display = 'flex';
        const h4 = elements.cardBlock.querySelector('h4');
        if (h4 && h4.childNodes.length > 0) {
            h4.childNodes[0].nodeValue = 'Sub-Block stats ';
        }
    }

    if (quant && quant.setupUI) {
        quant.setupUI(uiHelpers);
    }
}

export function initUIListeners() {
    elements.toggleLeft.addEventListener('click', () => {
        elements.wrapperLeft.classList.toggle('collapsed');
        elements.toggleLeft.querySelector('svg').style.transform = elements.wrapperLeft.classList.contains('collapsed') ? 'rotate(180deg)' : 'rotate(0deg)';
    });

    elements.toggleRight.addEventListener('click', () => {
        elements.wrapperRight.classList.toggle('collapsed');
        elements.toggleRight.querySelector('svg').style.transform = elements.wrapperRight.classList.contains('collapsed') ? 'rotate(180deg)' : 'rotate(0deg)';
    });

    elements.genCountEl.addEventListener('keydown', (e) => {
        let val = parseInt(elements.genCountEl.value) || 1;
        if (e.key === 'ArrowUp') {
            e.preventDefault();
            elements.genCountEl.value = Math.min(8192, val * 2);
        } else if (e.key === 'ArrowDown') {
            e.preventDefault();
            elements.genCountEl.value = Math.max(1, Math.floor(val / 2));
        }
    });

    elements.advToggleBtn.addEventListener('click', () => {
        const isHidden = elements.advPanel.style.display === 'none';
        elements.advPanel.style.display = isHidden ? 'flex' : 'none';
        elements.advToggleBtn.classList.toggle('open', isHidden);
    });

    elements.distEl.addEventListener('change', (e) => {
        document.querySelectorAll('.dist-params').forEach(el => el.style.display = 'none');
        const targetParams = document.getElementById(`param-${e.target.value}`);
        if (targetParams) targetParams.style.display = 'flex';
    });
}