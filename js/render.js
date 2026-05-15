import { elements, getSettings } from './ui.js';
import registry from './quants/registry.js';
import { getErrStats } from './mathUtils.js';

let currentRenderData = null;
let lastHoveredIdx = 0;
export let zoomRange = null;
export let showSRHT = false;

export function setZoomRange(start, end) {
    zoomRange = { start, end };
    render();
}

export function resetZoom() {
    zoomRange = null;
    render();
}

export function toggleSRHT() {
    showSRHT = !showSRHT;
    render();
}

export function render() {
    const floats = elements.inputEl.value.split(/[, \n\t]+/).map(s => s.trim()).filter(s => s !== '' && !isNaN(s)).map(Number);
    if (floats.length === 0) return;

    const settings = getSettings();
    const quant = registry[settings.qType];

    const {
        qFloats, qMathStrings, bpw, blockMeta, superMeta, formulaHTML, tFloats, tQFloats
    } = quant.quantize(floats, settings);

    const hasSRHT = tFloats != null && tQFloats != null;
    if (!hasSRHT) {
        showSRHT = false;
        elements.btnToggleSRHT.style.display = 'none';
        elements.btnToggleSRHT.classList.remove('active');
    } else {
        elements.btnToggleSRHT.style.display = 'flex';
        elements.btnToggleSRHT.classList.toggle('active', showSRHT);
    }

    let vFloats = showSRHT ? tFloats : floats;
    let vQFloats = showSRHT ? tQFloats : qFloats;

    const glbErr = getErrStats(floats, qFloats);
    elements.quantStats.textContent = `BPW Limit : ${settings.qType === 'none' ? '32.000' : bpw.toFixed(3)} bits\nRatio     : ${settings.qType === 'none' ? '1.00' : (32 / bpw).toFixed(2)}x smaller\nGlobal MSE: ${glbErr.mse.toFixed(6)}`;

    elements.formulaBox.innerHTML = formulaHTML;

    let zFloats = vFloats;
    if (zoomRange) {
        zFloats = vFloats.slice(zoomRange.start, zoomRange.end + 1);
        elements.btnResetZoom.style.display = 'flex';
    } else {
        elements.btnResetZoom.style.display = 'none';
    }

    let dMax = Math.max(...zFloats);
    let dMin = Math.min(...zFloats);
    if (dMax === dMin) {
        dMax += 0.1;
        dMin -= 0.1;
    }
    const spread = dMax - dMin;

    const sMax = settings.centeringMode === 'mid' ? dMax + spread * 0.1 : Math.max(0.1, ...zFloats.map(Math.abs)) * 1.1;
    const sMin = settings.centeringMode === 'mid' ? dMin - spread * 0.1 : -sMax;

    elements.chartMaxLbl.textContent = `Max: ${sMax.toFixed(2)}`;
    elements.chartMinLbl.textContent = `Min: ${sMin.toFixed(2)}`;

    elements.chartArea.innerHTML = '<div class="baseline" id="baseline"></div><div class="zoom-box" id="zoom-box" style="display: none;"></div>';
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

    const frag = quant.buildElements(vFloats, vQFloats, settings, createBar);

    if (zoomRange) {
        const bars = frag.querySelectorAll('.bar');
        bars.forEach(bar => {
            const idx = parseInt(bar.dataset.idx, 10);
            if (idx < zoomRange.start || idx > zoomRange.end) {
                bar.style.display = 'none';
            }
        });

        const groups = frag.querySelectorAll('.block-group, .sb-group');
        groups.forEach(grp => {
            const visibleBars = Array.from(grp.querySelectorAll('.bar')).filter(b => b.style.display !== 'none');
            if (visibleBars.length === 0) {
                grp.style.display = 'none';
            } else {
                grp.style.flex = visibleBars.length;
            }
        });
    }

    elements.chartArea.appendChild(frag);

    currentRenderData = { floats, qFloats, qMathStrings, blockMeta, superMeta, settings, quant, tFloats, tQFloats };
    updateInspector(lastHoveredIdx);
}

export function updateInspector(idx) {
    lastHoveredIdx = idx;
    if (!currentRenderData) return;
    const { floats, qFloats, qMathStrings, blockMeta, superMeta, settings, quant, tFloats, tQFloats } = currentRenderData;
    if (floats.length === 0) return;
    if (idx < 0 || idx >= floats.length) idx = 0;

    const val = showSRHT && tFloats ? tFloats[idx] : floats[idx];
    const valQ = showSRHT && tQFloats ? tQFloats[idx] : qFloats[idx];
    const mathStr = qMathStrings[idx];
    elements.iWIdx.textContent = `[${idx}]`;

    let mathHtml = mathStr ? `<div class="data-row" style="margin-top:4px; color:var(--text-muted);"><span>Math:</span> <span style="color:var(--text-main);">${mathStr}</span></div>` : '';

    const lblOrig = showSRHT ? "Transformed:" : "Original:";
    const lblQuant = showSRHT ? "Quantized (WHT):" : "Quantized:";

    elements.insWData.innerHTML = `<div class="data-row"><span>${lblOrig}</span> <span class="val-hl">${val.toFixed(5)}</span></div><div class="data-row"><span>${lblQuant}</span> <span class="val-hl">${valQ.toFixed(5)}</span></div>${mathHtml}<div class="data-row" style="margin-top:4px"><span>Abs Error:</span> <span>${Math.abs(val - valQ).toFixed(6)}</span></div>`;

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