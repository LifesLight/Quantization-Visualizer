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

    let sigPower = 0;
    let sumAbsOrig = 0;
    for (let i = 0; i < floats.length; i++) {
        sigPower += floats[i] * floats[i];
        sumAbsOrig += Math.abs(floats[i]);
    }
    const sigVar = floats.length > 0 ? sigPower / floats.length : 0;
    const sqnr = (glbErr.mse === 0 || sigVar === 0) ? Infinity : 10 * Math.log10(sigVar / glbErr.mse);
    const relError = sumAbsOrig === 0 ? 0 : ((glbErr.mae * floats.length) / sumAbsOrig) * 100;

    elements.quantStats.textContent = `BPW Limit : ${settings.qType === 'none' ? '32.000' : bpw.toFixed(3)} bits\nRatio     : ${settings.qType === 'none' ? '1.00' : (32 / bpw).toFixed(2)}x smaller\nGlobal MSE: ${glbErr.mse.toFixed(6)}\nSQNR      : ${sqnr === Infinity ? '∞' : sqnr.toFixed(2)} dB\nRel. Error: ${relError.toFixed(2)}%`;

    elements.formulaBox.innerHTML = formulaHTML;

    if (zoomRange) {
        elements.btnResetZoom.style.display = 'flex';
    } else {
        elements.btnResetZoom.style.display = 'none';
    }

    const zStart = zoomRange ? zoomRange.start : 0;
    const zEnd = zoomRange ? zoomRange.end : vFloats.length - 1;
    const zCount = zEnd - zStart + 1;

    let dMax = -Infinity;
    let dMin = Infinity;
    let absMax = 0;
    for (let i = zStart; i <= zEnd; i++) {
        let v = vFloats[i];
        if (v > dMax) dMax = v;
        if (v < dMin) dMin = v;
        let absV = Math.abs(v);
        if (absV > absMax) absMax = absV;
    }

    if (dMax === dMin) {
        dMax += 0.1;
        dMin -= 0.1;
    }
    const spread = dMax - dMin;

    const sMax = settings.centeringMode === 'mid' ? dMax + spread * 0.05 : Math.max(0.1, absMax) * 1.1;
    const sMin = settings.centeringMode === 'mid' ? dMin - spread * 0.05 : -sMax;

    let baselineValue = 0;
    if (settings.centeringMode === 'mid') {
        if (dMin >= 0) {
            baselineValue = sMin; 
        } else if (dMax <= 0) {
            baselineValue = sMax; 
        } else {
            baselineValue = 0; 
        }
    }

    elements.chartMaxLbl.textContent = `Max: ${sMax.toFixed(2)}`;
    elements.chartMinLbl.textContent = `Min: ${sMin.toFixed(2)}`;

    elements.chartArea.innerHTML = '<div class="baseline" id="baseline"></div><div class="zoom-box" id="zoom-box" style="display: none;"></div>';
    const baselineY = ((baselineValue - sMin) / (sMax - sMin)) * 100;
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

        const yCenter = clamp(((baselineValue - sMin) / (sMax - sMin)) * 100);
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

    const clientWidth = elements.chartArea.clientWidth || 800;
    const pxPerBar = clientWidth / zCount;
    
    elements.chartArea.style.gap = '';

    const hasBlocks = blockMeta && blockMeta.length > 0;
    const hasSupers = superMeta && superMeta.length > 0;

    let frag;

    if (pxPerBar < 1) { 
        elements.chartArea.style.gap = '0px';
        frag = document.createDocumentFragment();
        
        const maxBars = clientWidth; 
        const binSize = zCount / maxBars;

        const barsPerBlock = settings.blockSize ? (settings.blockSize / binSize) : 0;
        const showBlocks = hasBlocks && barsPerBlock >= 8;

        const barsPerSuper = settings.sbSize ? (settings.sbSize / binSize) : 0;
        const showSupers = hasSupers && barsPerSuper >= 8;

        let currentSbGrp = null;
        let currentBlkGrp = null;
        let lastSbIdx = -1;
        let lastBlkIdx = -1;

        for (let i = 0; i < maxBars; i++) {
            let binStartIdx = zStart + Math.floor(i * binSize);
            let binEndIdx = i === maxBars - 1 ? zEnd + 1 : zStart + Math.floor((i + 1) * binSize);
            
            let maxMag = -1;
            let bestIdx = binStartIdx;

            for (let j = binStartIdx; j < binEndIdx; j++) {
                let mag = Math.abs(vFloats[j]);
                if (mag > maxMag) {
                    maxMag = mag;
                    bestIdx = j;
                }
            }

            let sbIdx = showSupers ? Math.floor(binStartIdx / settings.sbSize) : -1;
            let blkIdx = showBlocks ? Math.floor(binStartIdx / settings.blockSize) : -1;

            if (showSupers && sbIdx !== lastSbIdx) {
                currentSbGrp = document.createElement('div');
                currentSbGrp.className = showBlocks ? 'sb-group' : 'block-group';
                currentSbGrp.style.gap = '0px';
                frag.appendChild(currentSbGrp);
                lastSbIdx = sbIdx;
                lastBlkIdx = -1; 
            }

            if (showBlocks && blkIdx !== lastBlkIdx) {
                currentBlkGrp = document.createElement('div');
                currentBlkGrp.className = 'block-group';
                currentBlkGrp.style.gap = '0px';
                if (currentSbGrp) {
                    currentSbGrp.appendChild(currentBlkGrp);
                } else {
                    frag.appendChild(currentBlkGrp);
                }
                lastBlkIdx = blkIdx;
            }

            const bar = createBar(vFloats[bestIdx], vQFloats[bestIdx], bestIdx);
            bar.style.flex = "1"; 

            if (currentBlkGrp) {
                currentBlkGrp.appendChild(bar);
            } else if (currentSbGrp) {
                currentSbGrp.appendChild(bar);
            } else {
                frag.appendChild(bar);
            }
        }

        const groups = frag.querySelectorAll('.block-group, .sb-group');
        groups.forEach(grp => {
            const visibleBars = grp.querySelectorAll('.bar').length;
            if (visibleBars === 0) {
                grp.style.display = 'none';
            } else {
                grp.style.flex = visibleBars;
            }
        });

    } else {
        frag = quant.buildElements(vFloats, vQFloats, settings, (val, valQ, globalIdx) => {
            if (zoomRange && (globalIdx < zoomRange.start || globalIdx > zoomRange.end)) {
                const dummy = document.createElement('div');
                dummy.className = 'dummy-bar';
                return dummy;
            }
            return createBar(val, valQ, globalIdx);
        });

        if (zoomRange) {
            const dummies = frag.querySelectorAll('.dummy-bar');
            dummies.forEach(d => d.remove());

            const groups = frag.querySelectorAll('.block-group, .sb-group');
            groups.forEach(grp => {
                const visibleBars = grp.querySelectorAll('.bar').length;
                if (visibleBars === 0) {
                    grp.style.display = 'none';
                } else {
                    grp.style.flex = visibleBars;
                }
            });
        }
    }

    const visibleTopGroups = Array.from(frag.childNodes).filter(el => el.nodeType === 1 && el.style.display !== 'none');
    if (visibleTopGroups.length > 0) {
        // Strip trailing right edge
        let rightEdge = visibleTopGroups[visibleTopGroups.length - 1];
        while (rightEdge) {
            if (rightEdge.classList && (rightEdge.classList.contains('sb-group') || rightEdge.classList.contains('block-group'))) {
                rightEdge.style.borderRight = 'none';
            }
            const visChildren = Array.from(rightEdge.childNodes).filter(el => el.nodeType === 1 && el.style.display !== 'none');
            rightEdge = visChildren.length > 0 ? visChildren[visChildren.length - 1] : null;
        }

        // Strip trailing left edge
        let leftEdge = visibleTopGroups[0];
        while (leftEdge) {
            if (leftEdge.classList && (leftEdge.classList.contains('sb-group') || leftEdge.classList.contains('block-group'))) {
                leftEdge.style.borderLeft = 'none';
            }
            const visChildren = Array.from(leftEdge.childNodes).filter(el => el.nodeType === 1 && el.style.display !== 'none');
            leftEdge = visChildren.length > 0 ? visChildren[0] : null;
        }
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