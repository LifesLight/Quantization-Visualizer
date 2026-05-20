import { elements, getSettings } from './ui.js';
import registry from './quants/registry.js';
import { getF32Array } from './wasmWrapper.js';

export let backend = null;
export function setBackend(b) { backend = b; }

let currentRenderData = null;
export let lastHoveredIdx = 0;
export let zoomRange = null;
export let showSRHT = false;

export let clipStart = null;
export let clipEnd = null;
export let hasWarnedLargeData = false;
export let userModifiedClip = false;

export let hoveredIdx = null;
export let dragStartIdx = null;
export let dragCurrentIdx = null;

let cachedPattern = null;
let cachedPatternIsDark = null;

/**
 * Syncs the position and sizes of the DOM highlight overlays without redrawing the canvas.
 */
export function updateOverlays() {
    const hoverEl = document.getElementById('hover-overlay');
    const blockEl = document.getElementById('block-overlay');
    const superEl = document.getElementById('super-overlay');
    const dragEl = document.getElementById('drag-overlay');

    if (!currentRenderData || !hoverEl) return;

    const { stats, baseLen } = currentRenderData;
    const zStart = zoomRange ? zoomRange.start : 0;
    const zEnd = zoomRange ? zoomRange.end : baseLen - 1;
    const zCount = zEnd - zStart + 1;
    const rect = elements.chartArea.getBoundingClientRect();
    const barW = rect.width / zCount;

    const useClip = elements.dbModeClip && elements.dbModeClip.checked;
    const isDark = document.documentElement.getAttribute('data-theme') === 'dark';

    // Helper to calculate pixel position and width
    const getXW = (dStart, dEnd) => {
        let x, w;
        if (barW >= 1) {
            x = (dStart - zStart) * barW;
            w = (dEnd - dStart + 1) * barW;
        } else {
            x = Math.floor(((dStart - zStart) / zCount) * rect.width);
            w = Math.max(1, Math.ceil(((dEnd - dStart + 1) / zCount) * rect.width));
        }
        return { x, w };
    };

    if (dragStartIdx !== null && dragCurrentIdx !== null) {
        const sIdx = Math.min(dragStartIdx, dragCurrentIdx);
        const eIdx = Math.max(dragStartIdx, dragCurrentIdx);
        const xStart = Math.max(zStart, sIdx);
        const xEnd = Math.min(zEnd, eIdx);
        if (xStart <= xEnd) {
            const { x, w } = getXW(xStart, xEnd);
            dragEl.style.display = 'block';
            dragEl.style.left = `${x}px`;
            dragEl.style.width = `${w}px`;
        } else {
            dragEl.style.display = 'none';
        }
    } else {
        dragEl.style.display = 'none';
    }

    // Hide Hover Overlays initially
    hoverEl.style.display = 'none';
    blockEl.style.display = 'none';
    superEl.style.display = 'none';

    const primaryIdx = hoveredIdx !== null ? hoveredIdx : dragStartIdx;

    if (primaryIdx !== null && primaryIdx >= zStart && primaryIdx <= zEnd) {
        // Individual Bar Hover
        const { x, w } = getXW(primaryIdx, primaryIdx);
        hoverEl.style.display = 'block';
        hoverEl.style.left = `${x}px`;
        hoverEl.style.width = `${w}px`;
        hoverEl.style.backgroundColor = isDark ? 'rgba(248, 250, 252, 0.15)' : 'rgba(15, 23, 42, 0.15)';

        // Block / Super Block Highlights
        const sliceIdx = useClip ? primaryIdx - clipStart : primaryIdx;
        if (sliceIdx >= 0 && sliceIdx < currentRenderData.actLen) {
            const actBlockSize = stats.block_size || 0;
            const actSbSize = stats.super_block_size || 0;
            const showBlocks = actBlockSize > 0 && (barW < 1 ? (actBlockSize * barW) >= 8 : true);
            const showSupers = actSbSize > 0 && (barW < 1 ? (actSbSize * barW) >= 8 : true);

            if (showSupers) {
                const startSlice = Math.floor(sliceIdx / actSbSize) * actSbSize;
                const endSlice = startSlice + actSbSize - 1;
                const dStart = Math.max(zStart, useClip ? startSlice + clipStart : startSlice);
                const dEnd = Math.min(zEnd, useClip ? endSlice + clipStart : endSlice);

                if (dStart <= dEnd) {
                    const xw = getXW(dStart, dEnd);
                    superEl.style.display = 'block';
                    superEl.style.left = `${xw.x}px`;
                    superEl.style.width = `${xw.w}px`;
                    superEl.style.backgroundColor = isDark ? 'rgba(255, 255, 255, 0.02)' : 'rgba(0, 0, 0, 0.02)';
                }
            }

            if (showBlocks) {
                const startSlice = Math.floor(sliceIdx / actBlockSize) * actBlockSize;
                const endSlice = startSlice + actBlockSize - 1;
                const dStart = Math.max(zStart, useClip ? startSlice + clipStart : startSlice);
                const dEnd = Math.min(zEnd, useClip ? endSlice + clipStart : endSlice);

                if (dStart <= dEnd) {
                    const xw = getXW(dStart, dEnd);
                    blockEl.style.display = 'block';
                    blockEl.style.left = `${xw.x}px`;
                    blockEl.style.width = `${xw.w}px`;
                    blockEl.style.backgroundColor = isDark ? 'rgba(79, 70, 229, 0.15)' : 'rgba(79, 70, 229, 0.05)';

                    // FIXED: Explicitly set the border color inside the shorthand string so it doesn't default to white text color
                    const borderColor = isDark ? 'rgba(79, 70, 229, 0.5)' : 'rgba(79, 70, 229, 0.3)';
                    blockEl.style.borderLeft = dStart >= zStart ? `1px solid ${borderColor}` : 'none';
                    blockEl.style.borderRight = dEnd + 1 <= zEnd ? `1px solid ${borderColor}` : 'none';
                }
            }
        }
    }
}

/**
 * Registers coordinates for the visual hover index and triggers DOM overlay redraw.
 * @param {number|null} idx - The focused data point index.
 */
export function setHoveredIdx(idx) {
    if (hoveredIdx === idx) return;
    hoveredIdx = idx;

    if (idx !== null) {
        lastHoveredIdx = idx;
        if (dragStartIdx === null) {
            updateInspector(idx);
        }
    }
    // Only update overlays - skip the heavy canvas rendering!
    updateOverlays();
}

/**
 * Stores interactive drag parameters for the range zoom gesture.
 * @param {number|null} startIdx - Boundary start.
 * @param {number|null} currentIdx - Boundary current.
 */
export function setDragState(startIdx, currentIdx) {
    if (dragStartIdx === startIdx && dragCurrentIdx === currentIdx) return;
    dragStartIdx = startIdx;
    dragCurrentIdx = currentIdx;
    updateOverlays();
}

export function setHasWarnedLargeData(val) { hasWarnedLargeData = val; }
export function getHasWarnedLargeData() { return hasWarnedLargeData; }
export function getClipRange() { return { start: clipStart, end: clipEnd }; }
export function setUserModifiedClip(val) { userModifiedClip = val; }

let rawFloatsStr = "";
let lastScale = null;
let lastOffset = null;

/**
 * Parses, transforms and scales raw buffer streams from UI.
 * @returns {Float32Array} Floating array reference mapped to memory.
 */
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
    if (previewOnly) drawDatasetBar();
    else requantize(true);
}

export function setZoomRange(start, end, doRender = true) {
    zoomRange = { start, end };
    if (doRender) render();
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
                let width = clipEnd - clipStart + 1;
                if (clipEnd >= N) {
                    clipEnd = Math.max(0, N - 1);
                    clipStart = Math.max(0, clipEnd - width + 1);
                }
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
    const stats = backend.quantize(settings);

    currentRenderData = {
        baseLen: backend.get_base_floats_len(),
        basePtr: backend.get_base_floats_ptr(),
        actLen: backend.get_active_floats_len(),
        actPtr: backend.get_active_floats_ptr(),
        qPtr: backend.get_q_floats_ptr(),
        tPtr: backend.get_t_floats_ptr(),
        tQPtr: backend.get_t_q_floats_ptr(),
        settings, quant: registry[settings.qType],
        bpw: stats.bpw,
        formulaHTML: stats.formula_html,
        stats
    };

    render();
}

/**
 * Processes layout transformations, active canvas painting routines and boundary overlays.
 */
export function render() {
    if (!currentRenderData) return;

    const {
        baseLen, basePtr, actLen, actPtr, qPtr, tPtr, tQPtr, settings, bpw, formulaHTML, stats
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

    elements.btnResetZoom.style.display = zoomRange ? 'flex' : 'none';

    const zStart = zoomRange ? zoomRange.start : 0;
    const zEnd = zoomRange ? zoomRange.end : baseLen - 1;
    const zCount = zEnd - zStart + 1;

    let dMax = -Infinity, dMin = Infinity, absMax = 0;
    const useSRHTForRange = showSRHT && tFloats;

    for (let i = zStart; i <= zEnd; i++) {
        let v = (useSRHTForRange && i >= clipStart && i <= clipEnd) ? tFloats[i - clipStart] : baseFloats[i];
        if (v > dMax) dMax = v;
        if (v < dMin) dMin = v;
        let absV = v < 0 ? -v : v;
        if (absV > absMax) absMax = absV;
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

    let canvas = document.getElementById('main-canvas');
    if (!canvas) {
        elements.chartArea.innerHTML = `
            <canvas id="main-canvas" style="position:absolute; width:100%; height:100%; left:0; top:0; pointer-events:none;"></canvas>
            <div id="super-overlay" style="position:absolute; top:0; height:100%; pointer-events:none; display:none; z-index:1;"></div>
            <div id="block-overlay" style="position:absolute; top:0; height:100%; pointer-events:none; display:none; z-index:2; border-top:none; border-bottom:none; box-sizing:border-box;"></div>
            <div id="hover-overlay" style="position:absolute; top:0; height:100%; pointer-events:none; display:none; z-index:3;"></div>
            <div id="drag-overlay" style="position:absolute; top:0; height:100%; background:rgba(79, 70, 229, 0.15); border-left:1px solid rgba(79, 70, 229, 0.5); border-right:1px solid rgba(79, 70, 229, 0.5); pointer-events:none; display:none; z-index:4; box-sizing:border-box;"></div>
            <div class="baseline" id="baseline" style="z-index:5;"></div>
        `;
        canvas = document.getElementById('main-canvas');
    }

    const ctx = canvas.getContext('2d');
    const rect = elements.chartArea.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    canvas.style.width = `${rect.width}px`;
    canvas.style.height = `${rect.height}px`;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, rect.width, rect.height);

    const baselineY = ((baselineValue - sMin) / (sMax - sMin)) * 100;
    const baselineEl = document.getElementById('baseline');
    if (baselineY >= 0 && baselineY <= 100) {
        baselineEl.style.bottom = `${baselineY}%`;
        baselineEl.style.display = 'block';
    } else {
        baselineEl.style.display = 'none';
    }

    const isDark = document.documentElement.getAttribute('data-theme') === 'dark';
    const accentColor = isDark ? '#10b981' : '#10b981';
    const negativeColor = isDark ? '#ef4444' : '#ef4444';
    const inactiveColor = isDark ? 'rgba(148, 163, 184, 0.25)' : 'rgba(100, 116, 139, 0.25)';

    if (!cachedPattern || cachedPatternIsDark !== isDark) {
        const pCanv = document.createElement('canvas');
        pCanv.width = 4; pCanv.height = 4;
        const pCtx = pCanv.getContext('2d');
        pCtx.fillStyle = isDark ? 'rgba(248, 250, 252, 0.4)' : 'rgba(15, 23, 42, 0.4)';
        pCtx.beginPath();
        pCtx.moveTo(0, 0); pCtx.lineTo(2, 0); pCtx.lineTo(0, 2); pCtx.closePath(); pCtx.fill();
        pCtx.beginPath();
        pCtx.moveTo(0, 4); pCtx.lineTo(4, 0); pCtx.lineTo(4, 2); pCtx.lineTo(2, 4); pCtx.closePath(); pCtx.fill();
        cachedPattern = ctx.createPattern(pCanv, 'repeat');
        cachedPatternIsDark = isDark;
    }
    const errPattern = cachedPattern;

    const clampY = (val) => Math.max(0, Math.min(rect.height, val));
    const getY = (v) => clampY(((sMax - v) / (sMax - sMin)) * rect.height);
    const yCenter = getY(baselineValue);

    const useClip = elements.dbModeClip && elements.dbModeClip.checked;
    const barW = rect.width / zCount;

    const actBlockSize = stats.block_size || 0;
    const actSbSize = stats.super_block_size || 0;

    const showBlocks = actBlockSize > 0 && (barW < 1 ? (actBlockSize * barW) >= 8 : true);
    const showSupers = actSbSize > 0 && (barW < 1 ? (actSbSize * barW) >= 8 : true);

    const cMain = isDark ? '#f8fafc' : '#0f172a';
    const cSub = isDark ? 'rgba(248, 250, 252, 0.25)' : 'rgba(15, 23, 42, 0.25)';

    if (barW < 1) {
        for (let i = 0; i < rect.width; i++) {
            const binS = zStart + Math.floor((i / rect.width) * zCount);
            const binE = Math.max(binS, zStart + Math.floor(((i + 1) / rect.width) * zCount) - 1);
            let bestIdx = binS, maxMag = -1;

            for (let j = binS; j <= binE; j++) {
                let v = (useSRHTForRange && j >= clipStart && j <= clipEnd) ? tFloats[j - clipStart] : baseFloats[j];
                let m = v < 0 ? -v : v;
                if (m > maxMag) { maxMag = m; bestIdx = j; }
            }

            const isActive = !useClip || (bestIdx >= clipStart && bestIdx <= clipEnd);
            const vO = useSRHTForRange && isActive ? tFloats[bestIdx - clipStart] : baseFloats[bestIdx];
            const vQ = isActive ? ((showSRHT && tQFloats) ? tQFloats[bestIdx - clipStart] : qFloats[bestIdx - clipStart]) : baseFloats[bestIdx];

            const yV = getY(vO), yVQ = getY(vQ);

            ctx.fillStyle = isActive ? (vQ >= 0 ? accentColor : negativeColor) : inactiveColor;
            const qTop = Math.min(yCenter, yVQ);
            const qH = Math.max(1, Math.abs(yVQ - yCenter));
            ctx.fillRect(i, qTop, 1, qH);

            if (isActive && Math.abs(yV - yVQ) > 1) {
                ctx.fillStyle = errPattern;
                ctx.fillRect(i, Math.min(yV, yVQ), 1, Math.abs(yV - yVQ));
            }
        }
    } else {
        for (let i = 0; i < zCount; i++) {
            const gIdx = zStart + i;
            const isActive = !useClip || (gIdx >= clipStart && gIdx <= clipEnd);
            const vO = useSRHTForRange && isActive ? tFloats[gIdx - clipStart] : baseFloats[gIdx];
            const vQ = isActive ? ((showSRHT && tQFloats) ? tQFloats[gIdx - clipStart] : qFloats[gIdx - clipStart]) : baseFloats[gIdx];

            const x = i * barW;
            const yV = getY(vO), yVQ = getY(vQ);

            ctx.fillStyle = isActive ? (vQ >= 0 ? accentColor : negativeColor) : inactiveColor;
            const qTop = Math.min(yCenter, yVQ);
            const qH = Math.max(1, Math.abs(yVQ - yCenter));
            ctx.fillRect(x, qTop, barW, qH);

            if (isActive && Math.abs(yV - yVQ) > 1) {
                ctx.fillStyle = errPattern;
                ctx.fillRect(x, Math.min(yV, yVQ), barW, Math.abs(yV - yVQ));
            }
        }
    }

    const drawBoundaries = (size, strokeStyle, lineWidth, skipSize = 0) => {
        ctx.strokeStyle = strokeStyle;
        ctx.lineWidth = lineWidth;
        ctx.beginPath();

        let firstSliceIdx = useClip ? zStart - clipStart : zStart;
        let startBoundary = Math.ceil(firstSliceIdx / size) * size;
        if (startBoundary === 0) startBoundary += size;

        for (let boundary = startBoundary; ; boundary += size) {
            let gIdx = useClip ? boundary + clipStart : boundary;
            if (gIdx > zEnd) break;

            if (skipSize > 0 && boundary % skipSize === 0) continue;

            let x = ((gIdx - zStart) / zCount) * rect.width;
            x = Math.round(x) + (lineWidth % 2 === 0 ? 0 : 0.5);

            ctx.moveTo(x, 0); ctx.lineTo(x, rect.height);
        }
        ctx.stroke();
    };

    if (showSupers) {
        if (showBlocks) {
            drawBoundaries(actBlockSize, cSub, 1, actSbSize);
            drawBoundaries(actSbSize, cMain, 2, 0);
        } else {
            drawBoundaries(actSbSize, cSub, 1, 0);
        }
    } else if (showBlocks) {
        drawBoundaries(actBlockSize, cSub, 1, 0);
    }

    drawDatasetBar();

    if (dragStartIdx === null) {
        updateInspector(lastHoveredIdx);
    }

    updateOverlays();
}

export function drawDatasetBar() {
    if (!elements.dbBar || elements.dbBar.style.display === 'none') return;
    const baseFloats = getBaseFloats();
    if (!baseFloats || baseFloats.length === 0) return;

    const N = baseFloats.length;
    const zS = zoomRange ? zoomRange.start : 0;
    const zE = zoomRange ? zoomRange.end : N - 1;
    const zCount = zE - zS + 1;

    const getZoomPct = (idx) => {
        if (zCount <= 1) return 0;
        return Math.max(0, Math.min(100, ((idx - zS) / (zCount - 1)) * 100));
    };

    const getZoomPctUnclamped = (idx) => {
        if (zCount <= 1) return 0;
        return ((idx - zS) / (zCount - 1)) * 100;
    };

    const useClip = elements.dbModeClip && elements.dbModeClip.checked;
    const useMinMax = elements.dbModeMinMax && elements.dbModeMinMax.checked;
    const useHotspots = elements.dbModeHotspots && elements.dbModeHotspots.checked;

    if (useClip) {
        let leftPct = getZoomPct(clipStart);
        let rightPct = getZoomPct(clipEnd);
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

    const dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    canvas.style.width = `${rect.width}px`;
    canvas.style.height = `${rect.height}px`;

    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, rect.width, rect.height);

    if (useMinMax) {
        let minIdx = 0, maxIdx = 0;
        let minV = Infinity, maxV = -Infinity;
        for (let i = 0; i < N; i++) {
            let v = baseFloats[i];
            if (v < minV) { minV = v; minIdx = i; }
            if (v > maxV) { maxV = v; maxIdx = i; }
        }

        const minPct = getZoomPctUnclamped(minIdx);
        if (minPct >= 0 && minPct <= 100) {
            elements.dbMinArrow.style.display = 'block';
            elements.dbMinArrow.style.left = `${minPct}%`;
            elements.dbMinArrow.style.width = '6px';
            elements.dbMinArrow.style.marginLeft = '-3px';
            elements.dbMinArrow.style.background = 'var(--negative-color)';
            if (elements.dbMinArrow.firstElementChild) {
                elements.dbMinArrow.firstElementChild.style.color = 'var(--negative-color)';
            }
        } else {
            elements.dbMinArrow.style.display = 'none';
        }

        const maxPct = getZoomPctUnclamped(maxIdx);
        if (maxPct >= 0 && maxPct <= 100) {
            elements.dbMaxArrow.style.display = 'block';
            elements.dbMaxArrow.style.left = `${maxPct}%`;
            elements.dbMaxArrow.style.width = '6px';
            elements.dbMaxArrow.style.marginLeft = '-3px';
            elements.dbMaxArrow.style.background = 'var(--accent-color)';
            if (elements.dbMaxArrow.firstElementChild) {
                elements.dbMaxArrow.firstElementChild.style.color = 'var(--accent-color)';
            }
        } else {
            elements.dbMaxArrow.style.display = 'none';
        }
    } else {
        elements.dbMinArrow.style.display = 'none';
        elements.dbMaxArrow.style.display = 'none';
    }

    if (useHotspots && currentRenderData) {
        const floats = getF32Array(currentRenderData.actPtr, currentRenderData.actLen);
        const qFloats = getF32Array(currentRenderData.qPtr, currentRenderData.actLen);

        let leftPct = useClip ? getZoomPct(clipStart) : 0;
        let rightPct = useClip ? getZoomPct(clipEnd) : 100;

        const startX = Math.max(0, (leftPct / 100) * rect.width);
        const endX = Math.min(rect.width, (rightPct / 100) * rect.width);
        const pxWidth = endX - startX;

        if (pxWidth > 0) {
            const bins = Math.ceil(pxWidth);
            const activeLen = floats.length;
            let maxSE = 0;
            const seArr = new Float32Array(bins);

            for (let b = 0; b < bins; b++) {
                const px = startX + b;
                let iGlobal = zS + (px / rect.width) * (zCount - 1);
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
                        ctx.fillRect(startX + b, 0, 1, rect.height);
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

    let mathHtml = insData.mathStr ? `<div class="data-row" style="margin-top:4px; color:var(--text-muted);"><span>Math:</span> <span style="color:var(--text-main);">${insData.mathStr}</span></div>` : '';
    const lblOrig = showSRHT ? "Transformed:" : "Original:";
    const lblQuant = showSRHT ? "Quantized (WHT):" : "Quantized:";
    const errVal = Math.abs(val - valQ);

    elements.insWData.innerHTML = `<div class="data-row"><span>${lblOrig}</span> <span class="val-hl" title="${val}">${val.toFixed(5)}</span></div><div class="data-row"><span>${lblQuant}</span> <span class="val-hl" title="${valQ}">${valQ.toFixed(5)}</span></div>${mathHtml}<div class="data-row" style="margin-top:4px"><span>Abs Error:</span> <span title="${errVal}">${errVal.toFixed(6)}</span></div>`;

    let blockHtml = '';
    if (insData.mse !== undefined) blockHtml += `<div class="data-row"><span>MSE:</span> <span class="val-hl">${insData.mse.toFixed(6)}</span></div>`;
    if (insData.mae !== undefined) blockHtml += `<div class="data-row"><span>MAE:</span> <span>${insData.mae.toFixed(6)}</span></div>`;
    if (insData.scale !== undefined) blockHtml += `<div class="data-row" style="margin-top:4px"><span>Scale (FP16):</span> <span>${insData.scale.toFixed(5)}</span></div>`;
    if (insData.min !== undefined) blockHtml += `<div class="data-row"><span>Min (FP16):</span> <span>${insData.min.toFixed(5)}</span></div>`;
    if (insData.scaleE !== undefined) blockHtml += `<div class="data-row" style="margin-top:4px"><span>Block Scale (E8M0):</span> <span>2<sup>${insData.scaleE}</sup></span></div>`;
    if (insData.qjlScale !== undefined && insData.qjlScale > 0) blockHtml += `<div class="data-row"><span>QJL Scale:</span> <span>${insData.qjlScale.toFixed(5)}</span></div>`;

    if (insData.blockIdx !== undefined) {
        elements.iBIdx.textContent = `[${insData.blockIdx}]`;
        elements.insBData.innerHTML = blockHtml;
    } else {
        elements.iBIdx.textContent = '[-]';
        elements.insBData.innerHTML = '<div class="empty-state">Hover over a block to inspect</div>';
    }

    let superHtml = '';
    if (insData.trellisJson) {
        const p = JSON.parse(insData.trellisJson);
        const states = currentRenderData.settings.trellisStates || 1;

        let candHtml = '';
        const numCands = p.candidates ? p.candidates.length : 0;

        if (numCands > 0) {
            for (let c of p.candidates) {
                if (c.isNone) {
                    candHtml += `
                        <div style="display:flex; justify-content:space-between; gap:8px; align-items:center; padding:5px 8px; border-radius:6px; border:1px dashed var(--border-color); opacity:0.4;">
                            <div style="display:flex; gap:6px; align-items:center; font-size:11px;">
                                <span>- None -</span>
                            </div>
                            <div style="font-size:11px; display:flex; gap:8px;">
                                <span style="width:45px; text-align:right;">-</span>
                                <span style="opacity:0.6; width:65px; text-align:right;">-</span>
                            </div>
                        </div>`;
                    continue;
                }

                const active = c.prevState === p.prevState && c.subset === p.subset && c.cbIdx === p.cbIdx;
                const border = active ? 'var(--primary-color)' : 'var(--border-color)';
                const bg = active ? 'var(--card-bg)' : 'transparent';
                const textCol = active ? 'var(--primary-color)' : 'inherit';
                const opacity = active ? '1' : '0.6';
                const weight = active ? 'bold' : 'normal';

                candHtml += `
                    <div style="display:flex; justify-content:space-between; gap:8px; align-items:center; padding:5px 8px; border-radius:6px; border:1px solid ${border}; background:${bg}; color:${textCol}; font-weight:${weight}; opacity:${opacity};">
                        <div style="display:flex; gap:6px; align-items:center; font-size:11px;">
                            <span>S${c.prevState} &rarr; D${c.subset}</span>
                        </div>
                        <div style="font-size:11px; display:flex; gap:8px;">
                            <span style="width:45px; text-align:right;">q=${(c.cbVal ?? 0).toFixed(3)}</span>
                            <span style="opacity:0.6; width:65px; text-align:right;">(c=${(c.cost ?? 0).toFixed(3)})</span>
                        </div>
                    </div>`;
            }
        } else {
            candHtml = '<div style="opacity:0.6; font-size:11px; padding: 4px 0;">No candidates</div>';
        }

        elements.iSBIdx.textContent = `[t=${sliceIdx % (currentRenderData.settings.trellisBlockSize || 1)}]`;
        elements.insSBData.innerHTML = `
            <div style="display:flex; flex-direction:column; gap:12px;">
                ${states > 1 ? `
                <div>
                    <div style="display:flex; justify-content:space-between; align-items:baseline; margin-bottom:6px;">
                        <div style="font-size:10px; font-weight:600; opacity:0.6; text-transform:uppercase; letter-spacing:0.5px;">Viterbi State Transition</div>
                        <div style="font-size:10px; opacity:0.8;">Codebook Idx: <strong style="color:var(--text-color);">${p.cbIdx}</strong></div>
                    </div>
                    <div style="display:flex; align-items:center; justify-content:space-between; background: var(--card-bg); padding: 12px 16px; border-radius: 8px; border: 1px solid var(--border-color);">
                        <div style="text-align:center; flex: 0 0 auto;">
                            <div style="font-size:9px; opacity:0.6; margin-bottom:6px; letter-spacing:0.5px;">PREV</div>
                            <div style="background:transparent; border:2px solid var(--border-color); border-radius:50%; width:28px; height:28px; display:flex; align-items:center; justify-content:center; font-weight:bold; font-size:11px; margin:0 auto;">S${p.prevState}</div>
                        </div>
                        <div style="flex:1; display:flex; flex-direction:column; align-items:center; justify-content:center; padding: 0 10px;">
                            <div style="font-size:10px; font-weight:bold; color:var(--primary-color); margin-bottom:4px;">Subset D${p.subset}</div>
                            <div style="width:100%; height:2px; background:var(--primary-color); position:relative;">
                                <div style="position:absolute; right:0; top:-4px; border-top:5px solid transparent; border-bottom:5px solid transparent; border-left:6px solid var(--primary-color);"></div>
                            </div>
                            <div style="font-size:9px; opacity:0.6; margin-top:6px;">Path Cost: ${(p.cost ?? 0) < 1e9 ? (p.cost ?? 0).toFixed(3) : '∞'}</div>
                        </div>
                        <div style="text-align:center; flex: 0 0 auto;">
                            <div style="font-size:9px; opacity:0.6; margin-bottom:6px; letter-spacing:0.5px;">CURR</div>
                            <div style="background:var(--primary-color); color:white; border:2px solid var(--primary-color); border-radius:50%; width:28px; height:28px; display:flex; align-items:center; justify-content:center; font-weight:bold; font-size:11px; margin:0 auto;">S${p.state}</div>
                        </div>
                    </div>
                </div>
                <div>
                    <div style="font-size:10px; font-weight:600; opacity:0.6; margin-bottom:6px; text-transform:uppercase; letter-spacing:0.5px;">Evaluated Branches (to S${p.state})</div>
                    <div style="display:flex; flex-direction:column; gap:4px; max-height:140px; overflow-y:auto; padding-right:4px;">
                        ${candHtml}
                    </div>
                </div>
                ` : `
                <div style="display:flex; justify-content:space-between; align-items:baseline; margin-bottom:6px;">
                    <div style="font-size:10px; font-weight:600; opacity:0.6; text-transform:uppercase; letter-spacing:0.5px;">Scalar Quantization</div>
                    <div style="font-size:10px; opacity:0.8;">Codebook Idx: <strong style="color:var(--text-color);">${p.cbIdx}</strong></div>
                </div>
                <div style="font-size:11px; line-height:1.4; color:var(--text-muted); padding:10px; border:1px dashed var(--border-color); border-radius:6px; text-align:center;">
                    Values are snapped to the closest level in the codebook independently, without path memory.
                </div>
                `}
            </div>
        `;
    } else if (insData.globalScale !== undefined) {
        superHtml += `<div class="data-row"><span>Global MSE:</span> <span class="val-hl">${insData.globalMse.toFixed(6)}</span></div>`;
        superHtml += `<div class="data-row"><span>Global MAE:</span> <span>${insData.globalMae.toFixed(6)}</span></div>`;
        superHtml += `<div class="data-row" style="margin-top:4px"><span>Global Scale (FP32):</span> <span>${insData.globalScale.toFixed(6)}</span></div>`;
        elements.iSBIdx.textContent = '[Global]';
        elements.insSBData.innerHTML = superHtml;
    } else if (insData.superIdx !== undefined) {
        superHtml += `<div class="data-row"><span>Super MSE:</span> <span class="val-hl">${insData.superMse.toFixed(6)}</span></div>`;
        superHtml += `<div class="data-row"><span>Super MAE:</span> <span>${insData.superMae.toFixed(6)}</span></div>`;
        superHtml += `<div class="data-row" style="margin-top:4px"><span>Super Scale (FP16):</span> <span>${insData.superScale.toFixed(5)}</span></div>`;
        if (insData.superMin !== undefined) superHtml += `<div class="data-row"><span>Super Min (FP16):</span> <span>${insData.superMin.toFixed(5)}</span></div>`;
        elements.iSBIdx.textContent = `[${insData.superIdx}]`;
        elements.insSBData.innerHTML = superHtml;
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