import { elements, getSettings } from './ui.js';
import registry from './quants/registry.js';
import { getErrStats } from './mathUtils.js';

let currentRenderData = null;
let lastHoveredIdx = 0;

function clearInspector() {
    elements.iWIdx.textContent = '[-]';
    elements.insWData.innerHTML = '<div class="empty-state">Hover over a bar to inspect</div>';
    elements.iBIdx.textContent = '[-]';
    elements.insBData.innerHTML = '<div class="empty-state">Hover over a block to inspect</div>';
    elements.iSBIdx.textContent = '[-]';
    elements.insSBData.innerHTML = '<div class="empty-state">Hover over a superblock</div>';
}

export function render() {
    const floats = elements.inputEl.value
        .split(/[, \n\t]+/)
        .map(s => s.trim())
        .filter(s => s !== '' && !isNaN(s))
        .map(Number);

    if (floats.length === 0) {
        currentRenderData = null;
        elements.chartArea.innerHTML = '<div class="baseline" id="baseline"></div>';
        elements.quantStats.textContent = 'Waiting...';
        elements.formulaBox.innerHTML = '';
        elements.chartMaxLbl.textContent = 'Max: 0';
        elements.chartMinLbl.textContent = 'Min: 0';
        clearInspector();
        return;
    }

    const settings = getSettings();
    const quant = registry[settings.qType];

    if (!quant || typeof quant.quantize !== 'function') {
        elements.quantStats.textContent = `Unknown quantizer: ${settings.qType}`;
        return;
    }

    const {
        qFloats, qMathStrings, bpw, blockMeta, superMeta, formulaHTML
    } = quant.quantize(floats, settings);

    const glbErr = getErrStats(floats, qFloats);
    elements.quantStats.textContent =
        `BPW Limit : ${settings.qType === 'none' ? '32.000' : bpw.toFixed(3)} bits\n` +
        `Ratio     : ${settings.qType === 'none' ? '1.00' : (32 / bpw).toFixed(2)}x smaller\n` +
        `Global MSE: ${glbErr.mse.toFixed(6)}`;

    elements.formulaBox.innerHTML = formulaHTML;

    let dMax = Math.max(...floats);
    let dMin = Math.min(...floats);
    if (dMax === dMin) {
        dMax += 0.1;
        dMin -= 0.1;
    }
    const spread = dMax - dMin;

    const sMax = settings.centeringMode === 'mid'
        ? dMax + spread * 0.1
        : Math.max(0.1, ...floats.map(Math.abs)) * 1.1;
    const sMin = settings.centeringMode === 'mid'
        ? dMin - spread * 0.1
        : -sMax;

    elements.chartMaxLbl.textContent = `Max: ${sMax.toFixed(2)}`;
    elements.chartMinLbl.textContent = `Min: ${sMin.toFixed(2)}`;

    elements.chartArea.innerHTML = '<div class="baseline" id="baseline"></div>';
    const baselineY = ((0 - sMin) / (sMax - sMin)) * 100;
    const baselineEl = document.getElementById('baseline');

    if (baselineY >= 0 && baselineY <= 100) {
        baselineEl.style.bottom = `${baselineY}%`;
        baselineEl.style.display = 'block';
    } else {
        baselineEl.style.display = 'none';
    }

    const clamp = (val) => Math.max(0, Math.min(100, val));

    const createBar = (val, valQ, globalIdx) => {
        const bar = document.createElement('div');
        bar.className = 'bar';
        bar.dataset.idx = globalIdx;

        const yCenter = clamp(((0 - sMin) / (sMax - sMin)) * 100);
        const yVQ = clamp(((valQ - sMin) / (sMax - sMin)) * 100);
        const yV = clamp(((val - sMin) / (sMax - sMin)) * 100);

        const qFill = document.createElement('div');
        qFill.className = 'bar-fill';
        qFill.style.bottom = `${Math.min(yCenter, yVQ)}%`;
        qFill.style.height = `${Math.abs(yVQ - yCenter)}%`;
        qFill.style.backgroundColor = valQ >= 0 ? 'var(--accent-color)' : 'var(--negative-color)';
        bar.appendChild(qFill);

        const errH = Math.abs(yV - yVQ);
        if (errH > 0.05) {
            const errFill = document.createElement('div');
            errFill.className = 'error-fill';
            errFill.style.bottom = `${Math.min(yV, yVQ)}%`;
            errFill.style.height = `${errH}%`;
            bar.appendChild(errFill);
        }

        return bar;
    };

    const frag = quant.buildElements(floats, qFloats, settings, createBar);
    elements.chartArea.appendChild(frag);

    currentRenderData = { floats, qFloats, qMathStrings, blockMeta, superMeta, settings, quant };
    updateInspector(lastHoveredIdx);
}

export function updateInspector(idx) {
    lastHoveredIdx = idx;
    if (!currentRenderData) return;

    const { floats, qFloats, qMathStrings, blockMeta, superMeta, settings, quant } = currentRenderData;
    if (floats.length === 0) return;
    if (idx < 0 || idx >= floats.length) idx = 0;

    const val = floats[idx];
    const valQ = qFloats[idx];
    const mathStr = qMathStrings[idx];

    elements.iWIdx.textContent = `[${idx}]`;

    const mathHtml = mathStr
        ? `<div class="data-row" style="margin-top:4px; color:var(--text-muted);">
               <span>Math:</span>
               <span style="color:var(--text-main);">${mathStr}</span>
           </div>`
        : '';

    elements.insWData.innerHTML =
        `<div class="data-row"><span>Original:</span> <span class="val-hl">${val.toFixed(5)}</span></div>
         <div class="data-row"><span>Quantized:</span> <span class="val-hl">${valQ.toFixed(5)}</span></div>
         ${mathHtml}
         <div class="data-row" style="margin-top:4px"><span>Abs Error:</span> <span>${Math.abs(val - valQ).toFixed(6)}</span></div>`;

    const { blockHtml, blockIdxStr, superHtml, superIdxStr } = quant.formatInspector(idx, blockMeta, superMeta, settings);

    if (blockHtml) {
        elements.iBIdx.textContent = blockIdxStr;
        elements.insBData.innerHTML = blockHtml;
    } else {
        elements.iBIdx.textContent = '[-]';
        elements.insBData.innerHTML = '<div class="empty-state">Hover over a block to inspect</div>';
    }

    if (superHtml) {
        elements.iSBIdx.textContent = superIdxStr;
        elements.insSBData.innerHTML = superHtml;
    } else {
        elements.iSBIdx.textContent = '[-]';
        elements.insSBData.innerHTML = '<div class="empty-state">Hover over a superblock</div>';
    }
}