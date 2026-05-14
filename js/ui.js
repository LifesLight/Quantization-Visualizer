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
    get trellisBitsEl() { return document.getElementById('trellis-bits'); },
    get trellisBlockSizeEl() { return document.getElementById('trellis-block-size'); },
    get trellisStatesEl() { return document.getElementById('trellis-states'); },
    get trellisCbTypeEl() { return document.getElementById('trellis-cb-type'); },
    get trellisWhtEl() { return document.getElementById('trellis-wht'); },
    get toggleLeft() { return document.getElementById('toggle-left'); },
    get toggleRight() { return document.getElementById('toggle-right'); },
    get wrapperLeft() { return document.getElementById('panel-left-wrapper'); },
    get wrapperRight() { return document.getElementById('panel-right-wrapper'); },
    get blockSettings() { return document.getElementById('block-settings'); },
    get kquantSettings() { return document.getElementById('kquant-settings'); },
    get turboSettings() { return document.getElementById('turbo-settings'); },
    get trellisSettings() { return document.getElementById('trellis-settings'); },
    get cardSuper() { return document.getElementById('card-super'); },
    get quantStats() { return document.getElementById('quant-stats'); },
    get chartMaxLbl() { return document.getElementById('chart-max-lbl'); },
    get chartMinLbl() { return document.getElementById('chart-min-lbl'); },
    get autoUpdateElements() {
        return [
            this.modeEl, this.qTypeEl, this.qBitsEl,
            this.blockSizeEl, this.sbSizeEl, this.subSizeEl, this.subBitsEl,
            this.subOffsetEl, this.distEl, this.genBiasEl, this.genStdEl,
            this.genScaleEl, this.genBimodalDistEl, this.genBimodalSpreadEl,
            this.genOutlierProbEl, this.genOutlierMultEl, this.genUniRangeEl,
            this.turboBitsEl, this.turboBlockSizeEl, this.turboWhtEl, this.turboQjlEl,
            this.trellisBitsEl, this.trellisBlockSizeEl, this.trellisStatesEl, this.trellisCbTypeEl,
            this.trellisWhtEl
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

function safeDisplay(el, show) {
    if (!el) return;
    el.style.display = show ? 'flex' : 'none';
}

export const uiHelpers = {
    showBlockSettings: (show) => safeDisplay(elements.blockSettings, show),
    showKQuantSettings: (show) => safeDisplay(elements.kquantSettings, show),
    showTurboSettings: (show) => safeDisplay(elements.turboSettings, show),
    showTrellisSettings: (show) => safeDisplay(elements.trellisSettings, show),
    showQuantBits: (show) => {
        if (!elements.qBitsEl || !elements.qBitsEl.parentElement) return;
        elements.qBitsEl.parentElement.style.display = show ? 'flex' : 'none';
    },
    showSuperBlockCard: (show) => safeDisplay(elements.cardSuper, show),
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
        centeringMode: elements.modeEl.value
    };
}

export function updateUI() {
    const qType = elements.qTypeEl.value;
    const quant = registry[qType];
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