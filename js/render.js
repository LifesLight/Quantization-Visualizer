import { elements, getSettings } from './ui.js';
import registry from './quants/registry.js';
import { getF32Array } from './wasmWrapper.js';

export let backend = null;
export function setBackend(b) { backend = b; }

let currentRenderData = null;
let lastHoveredIdx = 0;
export let zoomRange = null;
export let showSRHT = false;

export let clipStart = null;
export let clipEnd = null;
export let hasWarnedLargeData = false;
export let userModifiedClip = false;

export function setHasWarnedLargeData(val) { hasWarnedLargeData = val; }
export function getHasWarnedLargeData() { return hasWarnedLargeData; }
export function getClipRange() { return { start: clipStart, end: clipEnd }; }
export function setUserModifiedClip(val) { userModifiedClip = val; }

let rawFloatsStr = "";
let lastScale = null;
let lastOffset = null;

export function getBaseFloats() {
    if (!backend) return new Float32Array();
    const text = elements.inputEl.value;
    const scale = parseFloat(elements.dataScaleEl.value);
    const finalScale = isNaN(scale) ? 1.0 : scale;
    const offset = parseFloat(elements.dataOffsetEl.value);
    const finalOffset = isNaN(offset) ? 0.0 : offset;

    if (text !== rawFloatsStr) {
        rawFloatsStr = text;
        backend.parse_floats(text);
        lastScale = null;
    }

    if (finalScale !== lastScale || finalOffset !== lastOffset) {
        backend.set_scale_offset(finalScale, finalOffset);
        lastScale = finalScale;
        lastOffset = finalOffset;
    }

    return getF32Array(backend.get_base_floats_ptr(), backend.get_base_floats_len());
}

export function updateDbBarVisibility() {
    const checked = (elements.dbModeClip && elements.dbModeClip.checked) ||
        (elements.dbModeMinMax && elements.dbModeMinMax.checked) ||
        (elements.dbModeHotspots && elements.dbModeHotspots.checked);
    if (elements.dbContainer) elements.dbContainer.style.display = checked ? 'flex' : 'none';
    if (elements.dbBar) elements.dbBar.style.display = checked ? 'block' : 'none';
    if (checked) drawDatasetBar();
}

export function setClipRange(start, end, previewOnly = false) {
    clipStart = start;
    clipEnd = end;
    if (previewOnly) {
        drawDatasetBar();
    } else {
        requantize(true);
    }
}

const tplBar = document.createElement('div');
tplBar.className = 'bar';

const tplBarFill = document.createElement('div');
tplBarFill.className = 'bar-fill';

const tplErrFill = document.createElement('div');
tplErrFill.className = 'error-fill';

export function setZoomRange(start, end) {
    zoomRange = { start, end };
    render();
}

export function resetZoom(doRender = true) {
    zoomRange = null;
    if (doRender) render();
}

export function toggleSRHT() {
    showSRHT = !showSRHT;
    render();
}

export function requantize(overrideClipCheck = false) {
    const baseFloats = getBaseFloats();
    const N = baseFloats.length;
    if (N === 0) {
        currentRenderData = null;
        return;
    }

    if (elements.dbModeClip && !elements.dbModeClip.checked) {
        clipStart = 0;
        clipEnd = Math.max(0, N - 1);
    } else {
        if (!userModifiedClip) {
            clipStart = 0;
            clipEnd = Math.max(0, N - 1);
        } else {
            if (clipStart === null || clipEnd === null) {
                clipStart = 0;
                clipEnd = Math.max(0, N - 1);
            } else {
                if (clipStart >= N) clipStart = Math.max(0, N - 1);
                if (clipEnd >= N) clipEnd = Math.max(0, N - 1);
            }
        }
    }

    const activeCount = clipEnd - clipStart + 1;
    if (!overrideClipCheck && activeCount > 1048576 && !hasWarnedLargeData) {
        clipStart = 0;
        clipEnd = 1048575;
        if (clipEnd >= N) clipEnd = N - 1;

        if (elements.dbModeClip) elements.dbModeClip.checked = true;
        if (elements.dbClipParams) elements.dbClipParams.style.display = 'flex';

        updateDbBarVisibility();

        if (elements.dbToggleBtn && !elements.dbToggleBtn.classList.contains('open')) {
            elements.dbToggleBtn.classList.add('open');
            if (elements.dbToolsPanel) elements.dbToolsPanel.style.display = 'flex';
        }
    }

    clipStart = Math.max(0, Math.min(clipStart, N - 1));
    clipEnd = Math.max(clipStart, Math.min(clipEnd, N - 1));

    backend.set_clip(clipStart, clipEnd);
    const settings = getSettings();
    const quant = registry[settings.qType];

    const stats = backend.quantize(settings);

    currentRenderData = {
        baseLen: backend.get_base_floats_len(),
        basePtr: backend.get_base_floats_ptr(),
        actLen: backend.get_active_floats_len(),
        actPtr: backend.get_active_floats_ptr(),
        qPtr: backend.get_q_floats_ptr(),
        tPtr: backend.get_t_floats_ptr(),
        tQPtr: backend.get_t_q_floats_ptr(),
        settings, quant,
        bpw: stats.bpw,
        formulaHTML: stats.formula_html,
        stats
    };

    render();
}

export function render() {
    if (!currentRenderData) return;

    const {
        baseLen, basePtr, actLen, actPtr, qPtr, tPtr, tQPtr, settings,
        quant, bpw, formulaHTML, stats
    } = currentRenderData;

    const baseFloats = getF32Array(basePtr, baseLen);
    const floats = getF32Array(actPtr, actLen);
    const qFloats = getF32Array(qPtr, actLen);
    const tFloats = stats.has_srht ? getF32Array(tPtr, actLen) : null;
    const tQFloats = stats.has_srht ? getF32Array(tQPtr, actLen) : null;

    if (!stats.has_srht) {
        showSRHT = false;
        elements.btnToggleSRHT.style.display = 'none';
        elements.btnToggleSRHT.classList.remove('active');
    } else {
        elements.btnToggleSRHT.style.display = 'flex';
        elements.btnToggleSRHT.classList.toggle('active', showSRHT);
    }

    const sqnr = (stats.global_mse === 0 || stats.global_variance === 0) ? Infinity : 10 * Math.log10(stats.global_variance / stats.global_mse);
    const relError = stats.global_sum_abs === 0 ? 0 : ((stats.global_mae * actLen) / stats.global_sum_abs) * 100;

    elements.quantStats.textContent = `BPW Limit : ${settings.qType === 'none' ? '32.000' : bpw.toFixed(3)} bits\nRatio     : ${settings.qType === 'none' ? '1.00' : (32 / bpw).toFixed(2)}x smaller\nGlobal MSE: ${stats.global_mse.toFixed(6)}\nSQNR      : ${sqnr === Infinity ? '∞' : sqnr.toFixed(2)} dB\nRel. Error: ${relError.toFixed(2)}%`;
    elements.formulaBox.innerHTML = formulaHTML;

    if (zoomRange) {
        elements.btnResetZoom.style.display = 'flex';
    } else {
        elements.btnResetZoom.style.display = 'none';
    }

    const zStart = zoomRange ? zoomRange.start : 0;
    const zEnd = zoomRange ? zoomRange.end : baseLen - 1;
    const zCount = zEnd - zStart + 1;

    let dMax = -Infinity;
    let dMin = Infinity;
    let absMax = 0;

    const useSRHTForRange = showSRHT && tFloats;
    if (useSRHTForRange) {
        for (let i = zStart; i <= zEnd; i++) {
            let v = (i >= clipStart && i <= clipEnd) ? tFloats[i - clipStart] : baseFloats[i];
            if (v > dMax) dMax = v;
            if (v < dMin) dMin = v;
            let absV = v < 0 ? -v : v;
            if (absV > absMax) absMax = absV;
        }
    } else {
        for (let i = zStart; i <= zEnd; i++) {
            let v = baseFloats[i];
            if (v > dMax) dMax = v;
            if (v < dMin) dMin = v;
            let absV = v < 0 ? -v : v;
            if (absV > absMax) absMax = absV;
        }
    }

    if (dMax === dMin) { dMax += 0.1; dMin -= 0.1; }
    const spread = dMax - dMin;

    const sMax = settings.centeringMode === 'mid' ? dMax + spread * 0.05 : (absMax === 0 ? 0.1 : absMax) * 1.1;
    const sMin = settings.centeringMode === 'mid' ? dMin - spread * 0.05 : -sMax;

    let baselineValue = 0;
    if (settings.centeringMode === 'mid') {
        if (dMin >= 0) baselineValue = sMin;
        else if (dMax <= 0) baselineValue = sMax;
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

    const createBar = (val, valQ, globalIdx, isActive = true) => {
        const bar = tplBar.cloneNode(false);
        bar.dataset.idx = globalIdx;

        const yCenter = clamp(((baselineValue - sMin) / (sMax - sMin)) * 100);
        const yVQ = clamp(((valQ - sMin) / (sMax - sMin)) * 100);
        const yV = clamp(((val - sMin) / (sMax - sMin)) * 100);

        const qFill = tplBarFill.cloneNode(false);
        if (!isActive) {
            qFill.style.bottom = `${Math.min(yCenter, yV)}%`;
            qFill.style.height = `${Math.abs(yV - yCenter)}%`;
            bar.appendChild(qFill);
            bar.classList.add('inactive');
            return bar;
        }

        qFill.style.bottom = `${Math.min(yCenter, yVQ)}%`;
        qFill.style.height = `${Math.abs(yVQ - yCenter)}%`;
        qFill.style.backgroundColor = valQ >= 0 ? 'var(--accent-color)' : 'var(--negative-color)';
        bar.appendChild(qFill);

        const errH = Math.abs(yV - yVQ);
        if (errH > 0.05) {
            const errFill = tplErrFill.cloneNode(false);
            errFill.style.bottom = `${Math.min(yV, yVQ)}%`;
            errFill.style.height = `${errH}%`;
            bar.appendChild(errFill);
        }

        return bar;
    };

    const clientWidth = elements.chartArea.clientWidth || 800;
    const pxPerBar = clientWidth / zCount;

    elements.chartArea.style.gap = '';
    const actBlockSize = stats.block_size || 0;
    const actSbSize = stats.super_block_size || 0;

    let frag = document.createDocumentFragment();
    const useClip = elements.dbModeClip ? elements.dbModeClip.checked : true;

    if (pxPerBar < 1) {
        elements.chartArea.style.gap = '0px';

        const maxBars = clientWidth;
        const binSize = zCount / maxBars;
        const showBlocks = actBlockSize > 0 && (actBlockSize / binSize) >= 8;
        const showSupers = actSbSize > 0 && (actSbSize / binSize) >= 8;

        let currentSbGrp = null;
        let currentBlkGrp = null;
        let lastSbIdx = -1;
        let lastBlkIdx = -1;

        for (let i = 0; i < maxBars; i++) {
            let binStartIdx = zStart + Math.floor(i * binSize);
            let binEndIdx = i === maxBars - 1 ? zEnd + 1 : zStart + Math.floor((i + 1) * binSize);

            let maxMag = -1;
            let bestIdx = binStartIdx;

            if (useSRHTForRange) {
                for (let j = binStartIdx; j < binEndIdx; j++) {
                    let v = (j >= clipStart && j <= clipEnd) ? tFloats[j - clipStart] : baseFloats[j];
                    let mag = v < 0 ? -v : v;
                    if (mag > maxMag) { maxMag = mag; bestIdx = j; }
                }
            } else {
                for (let j = binStartIdx; j < binEndIdx; j++) {
                    let v = baseFloats[j];
                    let mag = v < 0 ? -v : v;
                    if (mag > maxMag) { maxMag = mag; bestIdx = j; }
                }
            }

            let sbIdx = showSupers ? Math.floor(Math.max(0, binStartIdx - clipStart) / actSbSize) : -1;
            let blkIdx = showBlocks ? Math.floor(Math.max(0, binStartIdx - clipStart) / actBlockSize) : -1;

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
                if (currentSbGrp) currentSbGrp.appendChild(currentBlkGrp);
                else frag.appendChild(currentBlkGrp);
                lastBlkIdx = blkIdx;
            }

            const isActive = !useClip || (bestIdx >= clipStart && bestIdx <= clipEnd);
            let val = useSRHTForRange && isActive ? tFloats[bestIdx - clipStart] : baseFloats[bestIdx];
            let valQ = isActive ? ((showSRHT && tQFloats) ? tQFloats[bestIdx - clipStart] : qFloats[bestIdx - clipStart]) : baseFloats[bestIdx];

            const bar = createBar(val, valQ, bestIdx, isActive);
            bar.style.flex = "1";

            if (currentBlkGrp) currentBlkGrp.appendChild(bar);
            else if (currentSbGrp) currentSbGrp.appendChild(bar);
            else frag.appendChild(bar);
        }

        const groups = frag.querySelectorAll('.block-group, .sb-group');
        for (let i = 0; i < groups.length; i++) {
            const grp = groups[i];
            const visibleBars = grp.querySelectorAll('.bar').length;
            if (visibleBars === 0) grp.style.display = 'none';
            else grp.style.flex = visibleBars;
        }

    } else {
        const actZStart = useClip ? Math.max(zStart, clipStart) : zStart;
        const actZEnd = useClip ? Math.min(zEnd, clipEnd) : zEnd;

        if (useClip && zStart < clipStart) {
            const preEnd = Math.min(zEnd, clipStart - 1);
            for (let i = zStart; i <= preEnd; i++) frag.appendChild(createBar(baseFloats[i], baseFloats[i], i, false));
        }

        if (actZStart <= actZEnd) {
            let vFloatsActive = showSRHT ? tFloats : floats;
            let vQFloatsActive = showSRHT ? tQFloats : qFloats;

            let currentSbGrp = null;
            let currentBlkGrp = null;
            let lastSbIdx = -1;
            let lastBlkIdx = -1;

            for (let i = actZStart; i <= actZEnd; i++) {
                const sliceIdx = useClip ? i - clipStart : i;
                if (sliceIdx < 0 || sliceIdx >= actLen) continue;

                let sbIdx = actSbSize > 0 ? Math.floor(sliceIdx / actSbSize) : -1;
                let blkIdx = actBlockSize > 0 ? Math.floor(sliceIdx / actBlockSize) : -1;

                if (actSbSize > 0 && sbIdx !== lastSbIdx) {
                    currentSbGrp = document.createElement('div');
                    currentSbGrp.className = actBlockSize > 0 ? 'sb-group' : 'block-group';
                    currentSbGrp.style.gap = '0px';
                    frag.appendChild(currentSbGrp);
                    lastSbIdx = sbIdx;
                    lastBlkIdx = -1;
                }

                if (actBlockSize > 0 && blkIdx !== lastBlkIdx) {
                    currentBlkGrp = document.createElement('div');
                    currentBlkGrp.className = 'block-group';
                    currentBlkGrp.style.gap = '0px';
                    if (currentSbGrp) currentSbGrp.appendChild(currentBlkGrp);
                    else frag.appendChild(currentBlkGrp);
                    lastBlkIdx = blkIdx;
                }

                const val = vFloatsActive[sliceIdx];
                const valQ = vQFloatsActive[sliceIdx];
                const bar = createBar(val, valQ, i, true);
                bar.style.flex = "1";

                if (currentBlkGrp) currentBlkGrp.appendChild(bar);
                else if (currentSbGrp) currentSbGrp.appendChild(bar);
                else frag.appendChild(bar);
            }
        }

        if (useClip && zEnd > clipEnd) {
            const postStart = Math.max(zStart, clipEnd + 1);
            for (let i = postStart; i <= zEnd; i++) frag.appendChild(createBar(baseFloats[i], baseFloats[i], i, false));
        }

        const groups = frag.querySelectorAll('.block-group, .sb-group');
        for (let i = 0; i < groups.length; i++) {
            const grp = groups[i];
            const visibleBars = grp.querySelectorAll('.bar').length;
            if (visibleBars === 0) grp.style.display = 'none';
            else grp.style.flex = visibleBars;
        }
    }

    const visibleTopGroups = Array.from(frag.childNodes).filter(el => el.nodeType === 1 && el.style.display !== 'none');
    if (visibleTopGroups.length > 0) {
        let rightEdge = visibleTopGroups[visibleTopGroups.length - 1];
        while (rightEdge) {
            if (rightEdge.classList && (rightEdge.classList.contains('sb-group') || rightEdge.classList.contains('block-group'))) rightEdge.style.borderRight = 'none';
            const visChildren = Array.from(rightEdge.childNodes).filter(el => el.nodeType === 1 && el.style.display !== 'none');
            rightEdge = visChildren.length > 0 ? visChildren[visChildren.length - 1] : null;
        }

        let leftEdge = visibleTopGroups[0];
        while (leftEdge) {
            if (leftEdge.classList && (leftEdge.classList.contains('sb-group') || leftEdge.classList.contains('block-group'))) leftEdge.style.borderLeft = 'none';
            const visChildren = Array.from(leftEdge.childNodes).filter(el => el.nodeType === 1 && el.style.display !== 'none');
            leftEdge = visChildren.length > 0 ? visChildren[0] : null;
        }
    }

    elements.chartArea.appendChild(frag);
    drawDatasetBar();
    updateInspector(lastHoveredIdx);
}

export function drawDatasetBar() {
    if (!elements.dbBar || elements.dbBar.style.display === 'none') return;
    const baseFloats = getBaseFloats();
    if (!baseFloats || baseFloats.length === 0) return;

    const N = baseFloats.length;
    const zS = zoomRange ? zoomRange.start : 0;
    const zE = zoomRange ? zoomRange.end : N - 1;
    const zCount = zE - zS + 1;

    const getPct = (idx) => {
        if (zCount <= 1) return 0;
        return Math.max(0, Math.min(100, ((idx - zS) / (zCount - 1)) * 100));
    };

    const useClip = elements.dbModeClip && elements.dbModeClip.checked;
    const useMinMax = elements.dbModeMinMax && elements.dbModeMinMax.checked;
    const useHotspots = elements.dbModeHotspots && elements.dbModeHotspots.checked;

    if (useClip) {
        let leftPct = getPct(clipStart);
        let rightPct = getPct(clipEnd);
        elements.dbActiveRegion.style.left = `${leftPct}%`;
        elements.dbActiveRegion.style.width = `${rightPct - leftPct}%`;
        elements.dbActiveRegion.style.background = 'rgba(79, 70, 229, 0.15)';
        elements.dbActiveRegion.style.borderLeft = '1px solid var(--primary-color)';
        elements.dbActiveRegion.style.borderRight = '1px solid var(--primary-color)';
        if (elements.dbHandleLeft) elements.dbHandleLeft.style.display = 'block';
        if (elements.dbHandleRight) elements.dbHandleRight.style.display = 'block';

        if (elements.dbClipStart && document.activeElement !== elements.dbClipStart) elements.dbClipStart.value = clipStart;
        if (elements.dbClipWidth && document.activeElement !== elements.dbClipWidth) elements.dbClipWidth.value = (clipEnd - clipStart + 1);
    } else {
        elements.dbActiveRegion.style.left = `0%`;
        elements.dbActiveRegion.style.width = `100%`;
        elements.dbActiveRegion.style.background = 'transparent';
        elements.dbActiveRegion.style.borderLeft = 'none';
        elements.dbActiveRegion.style.borderRight = 'none';
        if (elements.dbHandleLeft) elements.dbHandleLeft.style.display = 'none';
        if (elements.dbHandleRight) elements.dbHandleRight.style.display = 'none';
    }

    const canvas = elements.dbCanvas;
    const ctx = canvas.getContext('2d');
    const rect = elements.dbBar.getBoundingClientRect();
    canvas.width = rect.width;
    canvas.height = rect.height;
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    elements.dbMinArrow.style.display = 'none';
    elements.dbMaxArrow.style.display = 'none';

    if (useMinMax) {
        let minIdx = zS, maxIdx = zS;
        let minV = Infinity, maxV = -Infinity;
        for (let i = zS; i <= zE; i++) {
            let v = baseFloats[i];
            if (v < minV) { minV = v; minIdx = i; }
            if (v > maxV) { maxV = v; maxIdx = i; }
        }
        elements.dbMinArrow.style.display = 'block';
        elements.dbMinArrow.style.left = `${getPct(minIdx)}%`;
        elements.dbMaxArrow.style.display = 'block';
        elements.dbMaxArrow.style.left = `${getPct(maxIdx)}%`;
    }

    if (useHotspots && currentRenderData) {
        const floats = getF32Array(currentRenderData.actPtr, currentRenderData.actLen);
        const qFloats = getF32Array(currentRenderData.qPtr, currentRenderData.actLen);

        let leftPct = useClip ? getPct(clipStart) : 0;
        let rightPct = useClip ? getPct(clipEnd) : 100;

        const startX = Math.max(0, (leftPct / 100) * canvas.width);
        const endX = Math.min(canvas.width, (rightPct / 100) * canvas.width);
        const pxWidth = endX - startX;

        if (pxWidth > 0) {
            const bins = Math.ceil(pxWidth);
            const activeLen = floats.length;
            let maxSE = 0;
            const seArr = new Float32Array(bins);

            for (let b = 0; b < bins; b++) {
                const px = startX + b;
                let iGlobal = zS + (px / canvas.width) * (zCount - 1);
                let sliceIdx = useClip ? Math.floor(iGlobal - clipStart) : Math.floor(iGlobal);

                if (sliceIdx >= 0 && sliceIdx < activeLen) {
                    const err = floats[sliceIdx] - qFloats[sliceIdx];
                    const se = err * err;
                    seArr[b] = se;
                    if (se > maxSE) maxSE = se;
                }
            }

            if (maxSE > 0) {
                for (let b = 0; b < bins; b++) {
                    if (seArr[b] > 0) {
                        const intensity = Math.min(1, seArr[b] / maxSE);
                        ctx.fillStyle = `rgba(239, 68, 68, ${intensity})`;
                        ctx.fillRect(startX + b, 0, 1, canvas.height);
                    }
                }
            }
        }
    }
}

export function updateInspector(idx) {
    lastHoveredIdx = idx;
    if (!currentRenderData) return;
    const N = currentRenderData.baseLen;
    if (N === 0) return;
    if (idx < 0 || idx >= N) idx = 0;

    elements.iWIdx.textContent = `[${idx}]`;

    const useClip = elements.dbModeClip && elements.dbModeClip.checked;
    const baseFloats = getF32Array(currentRenderData.basePtr, currentRenderData.baseLen);

    if (useClip && (idx < clipStart || idx > clipEnd)) {
        const val = baseFloats[idx];
        elements.insWData.innerHTML = `<div class="data-row"><span>Original:</span> <span class="val-hl" title="${val}">${val.toFixed(5)}</span></div><div class="empty-state" style="padding:10px 0;">Inactive Weight</div>`;
        elements.iBIdx.textContent = '[-]';
        elements.insBData.innerHTML = '<div class="empty-state">Inactive</div>';
        elements.iSBIdx.textContent = '[-]';
        elements.insSBData.innerHTML = '<div class="empty-state">Inactive</div>';
        return;
    }

    const sliceIdx = useClip ? idx - clipStart : idx;

    const insData = backend.get_inspector_data(sliceIdx, currentRenderData.settings);

    const floats = getF32Array(currentRenderData.actPtr, currentRenderData.actLen);
    const qFloats = getF32Array(currentRenderData.qPtr, currentRenderData.actLen);
    const tFloats = currentRenderData.stats.has_srht ? getF32Array(currentRenderData.tPtr, currentRenderData.actLen) : null;
    const tQFloats = currentRenderData.stats.has_srht ? getF32Array(currentRenderData.tQPtr, currentRenderData.actLen) : null;

    const val = showSRHT && tFloats ? tFloats[sliceIdx] : floats[sliceIdx];
    const valQ = showSRHT && tQFloats ? tQFloats[sliceIdx] : qFloats[sliceIdx];
    const mathStr = insData.mathStr;

    let mathHtml = mathStr ? `<div class="data-row" style="margin-top:4px; color:var(--text-muted);"><span>Math:</span> <span style="color:var(--text-main);">${mathStr}</span></div>` : '';

    const lblOrig = showSRHT ? "Transformed:" : "Original:";
    const lblQuant = showSRHT ? "Quantized (WHT):" : "Quantized:";
    const errVal = Math.abs(val - valQ);

    elements.insWData.innerHTML = `<div class="data-row"><span>${lblOrig}</span> <span class="val-hl" title="${val}">${val.toFixed(5)}</span></div><div class="data-row"><span>${lblQuant}</span> <span class="val-hl" title="${valQ}">${valQ.toFixed(5)}</span></div>${mathHtml}<div class="data-row" style="margin-top:4px"><span>Abs Error:</span> <span title="${errVal}">${errVal.toFixed(6)}</span></div>`;

    if (insData.blockHtml) {
        elements.iBIdx.textContent = insData.blockIdxStr;
        elements.insBData.innerHTML = insData.blockHtml;
    } else {
        elements.iBIdx.textContent = '[-]';
        elements.insBData.innerHTML = '<div class="empty-state">Hover over a block to inspect</div>';
    }

    if (insData.superHtml) {
        elements.iSBIdx.textContent = insData.superIdxStr;
        elements.insSBData.innerHTML = insData.superHtml;
    } else {
        elements.iSBIdx.textContent = '[-]';
        elements.insSBData.innerHTML = '<div class="empty-state">Hover over a superblock</div>';
    }
}

export function updateVisualsOnly() {
    if (currentRenderData) {
        currentRenderData.settings.centeringMode = elements.modeEl.value;
        render();
    }
}