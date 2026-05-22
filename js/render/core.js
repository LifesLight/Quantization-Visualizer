/**
 * Render Core
 * Contains the main WASM quantization caller and the primary Canvas2D drawing loop.
 */
import { state, getBaseFloats, getImportanceFloats } from './state.js';
import { elements, getSettings, validateImportance } from '../ui.js';
import registry from '../quants/registry.js';
import { updateDbBarVisibility, drawDatasetBar, updateInspector, updateOverlays } from './components.js';
import { getF32Array, getI32Array } from '../wasmWrapper.js';

let requantizeTimeout = null;
let cachedPattern = null;
let cachedPatternIsDark = null;

export const debouncedRequantize = (forceSync = false) => {
    clearTimeout(requantizeTimeout);
    if (forceSync) {
        requantize();
    } else {
        requantizeTimeout = setTimeout(() => requantize(), 100);
    }
};

export function requantize(overrideClipCheck = false) {
    const baseFloats = getBaseFloats();
    const impFloats = getImportanceFloats();
    const N = baseFloats.length;

    if (N === 0) {
        state.currentRenderData = null;
        validateImportance(0, 0, "");
        render();
        return;
    }

    validateImportance(N, impFloats.length, elements.inputImportanceEl.value);

    if (elements.dbModeClip && !elements.dbModeClip.checked) {
        state.clipStart = 0;
        state.clipEnd = Math.max(0, N - 1);
    } else {
        if (!state.userModifiedClip) {
            state.clipStart = 0;
            state.clipEnd = Math.max(0, N - 1);
        } else if (state.clipStart === null || state.clipEnd === null) {
            state.clipStart = 0;
            state.clipEnd = Math.max(0, N - 1);
        } else {
            let width = state.clipEnd - state.clipStart + 1;
            if (state.clipEnd >= N) {
                state.clipEnd = Math.max(0, N - 1);
                state.clipStart = Math.max(0, state.clipEnd - width + 1);
            }
        }
    }

    const activeCount = state.clipEnd - state.clipStart + 1;
    if (!overrideClipCheck && activeCount > 1048576 && !state.hasWarnedLargeData) {
        state.clipStart = 0;
        state.clipEnd = 1048575;
        if (state.clipEnd >= N) state.clipEnd = N - 1;

        if (elements.dbModeClip) elements.dbModeClip.checked = true;
        if (elements.dbClipParams) elements.dbClipParams.style.display = 'flex';

        updateDbBarVisibility();

        if (elements.dbToggleBtn && !elements.dbToggleBtn.classList.contains('open')) {
            elements.dbToggleBtn.classList.add('open');
            if (elements.dbToolsPanel) elements.dbToolsPanel.style.display = 'flex';
        }
    }

    state.clipStart = Math.max(0, Math.min(state.clipStart, N - 1));
    state.clipEnd = Math.max(state.clipStart, Math.min(state.clipEnd, N - 1));

    state.backend.set_clip(state.clipStart, state.clipEnd);
    const settings = getSettings();
    const stats = state.backend.quantize(settings);

    state.currentRenderData = {
        baseLen: state.backend.get_base_floats_len(),
        basePtr: state.backend.get_base_floats_ptr(),
        actLen: state.backend.get_active_floats_len(),
        actPtr: state.backend.get_active_floats_ptr(),
        qPtr: state.backend.get_q_floats_ptr(),
        tPtr: state.backend.get_t_floats_ptr(),
        tQPtr: state.backend.get_t_q_floats_ptr(),
        impIntensityPtr: state.backend.get_active_importance_intensity_ptr(),
        settings,
        quant: registry[settings.qType],
        bpw: stats.bpw,
        formulaHTML: stats.formula_html,
        stats
    };

    render();
}

export function render(opts = {}) {
    let canvas = document.getElementById('main-canvas');

    if (!state.currentRenderData) {
        if (canvas) {
            const ctx = canvas.getContext('2d');
            ctx.clearRect(0, 0, canvas.width, canvas.height);
        }
        elements.quantStats.textContent = "Waiting...";
        elements.importanceStats.textContent = "Waiting...";
        elements.importanceStatsGroup.style.display = 'none';
        elements.globalStatsGroup.style.marginTop = 'auto';
        elements.chartMaxLbl.textContent = "Max: 0";
        elements.chartMinLbl.textContent = "Min: 0";
        elements.formulaBox.innerHTML = "";

        const baselineEl = document.getElementById('baseline');
        if (baselineEl) baselineEl.style.display = 'none';

        updateOverlays();
        return;
    }

    const { updateInspector: doInspector = true, drawDbBar: doDbBar = true } = opts;

    const { baseLen, basePtr, actLen, actPtr, qPtr, tPtr, tQPtr, impIntensityPtr, settings, bpw, formulaHTML, stats } = state.currentRenderData;

    const baseFloats = getF32Array(basePtr, baseLen);
    const qFloats = getF32Array(qPtr, actLen);
    const tFloats = stats.has_srht ? getF32Array(tPtr, actLen) : null;
    const tQFloats = stats.has_srht ? getF32Array(tQPtr, actLen) : null;
    const useImportance = stats.has_importance;
    const actImpIntensity = useImportance ? getF32Array(impIntensityPtr, actLen) : null;

    if (!stats.has_srht) {
        state.showSRHT = false;
        elements.btnToggleSRHT.style.display = 'none';
        elements.btnToggleSRHT.classList.remove('active');
    } else {
        elements.btnToggleSRHT.style.display = 'flex';
        elements.btnToggleSRHT.classList.toggle('active', state.showSRHT);
    }

    const sqnr = (stats.global_mse === 0 || stats.global_variance === 0) ? Infinity : 10 * Math.log10(stats.global_variance / stats.global_mse);
    const relError = stats.global_sum_abs === 0 ? 0 : ((stats.global_mae * actLen) / stats.global_sum_abs) * 100;

    let statsText = `BPW Limit : ${settings.qType === 'none' ? '32.000' : bpw.toFixed(3)} bits\nRatio     : ${settings.qType === 'none' ? '1.00' : (32 / bpw).toFixed(2)}x smaller\nGlobal MSE: ${stats.global_mse.toFixed(6)}\nCosine Sim: ${stats.global_cosine_similarity.toFixed(6)}\nSQNR      : ${sqnr === Infinity ? '∞' : sqnr.toFixed(2)} dB\nMax Error : ${stats.max_error.toFixed(6)}\nRel. Error: ${relError.toFixed(2)}%`;
    elements.quantStats.textContent = statsText;

    if (stats.has_importance) {
        elements.importanceStatsGroup.style.display = 'flex';
        elements.globalStatsGroup.style.marginTop = '10px';

        const wSqnr = stats.weighted_snr === null || stats.weighted_snr === Infinity ? '∞' : stats.weighted_snr.toFixed(2);

        let impStatsText = `Weighted MSE: ${stats.weighted_mse.toFixed(6)}\nW-SQNR      : ${wSqnr} dB\nW-Cosine Sim: ${stats.weighted_cosine_similarity.toFixed(6)}`;
        elements.importanceStats.textContent = impStatsText;
    } else {
        elements.importanceStatsGroup.style.display = 'none';
        elements.globalStatsGroup.style.marginTop = 'auto';
    }

    elements.formulaBox.innerHTML = formulaHTML;
    elements.btnResetZoom.style.display = state.zoomRange ? 'flex' : 'none';

    const zStart = state.zoomRange ? state.zoomRange.start : 0;
    const zEnd = state.zoomRange ? state.zoomRange.end : baseLen - 1;
    const zCount = zEnd - zStart + 1;

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
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, rect.width, rect.height);

    const useClip = elements.dbModeClip && elements.dbModeClip.checked;
    const barW = rect.width / zCount;
    const useSRHTForRange = !!(state.showSRHT && tFloats);

    let sMin = 0, sMax = 0;
    let bestIdxArr = null;

    if (barW < 1) {
        const rs = state.backend.prepare_render(zStart, zEnd, rect.width, useSRHTForRange, settings);
        sMin = rs.render_min; sMax = rs.render_max;
        bestIdxArr = getI32Array(state.backend.get_best_indices_ptr(), state.backend.get_best_indices_len());
    } else {
        const rs = state.backend.get_range_stats(zStart, zEnd, useSRHTForRange, settings);
        sMin = rs.render_min; sMax = rs.render_max;
    }

    if (sMax === sMin) { sMax += 0.1; sMin -= 0.1; }

    let baselineValue = 0;
    if (settings.centeringMode === 'data' || settings.centeringMode === 'manual') {
        if (sMin >= 0) baselineValue = sMin;
        else if (sMax <= 0) baselineValue = sMax;
    }

    elements.chartMaxLbl.textContent = `Max: ${sMax.toFixed(2)}`;
    elements.chartMinLbl.textContent = `Min: ${sMin.toFixed(2)}`;

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

    const impH = 4;
    const bottomY = rect.height - impH;

    if (barW < 1) {
        for (let i = 0; i < rect.width; i++) {
            const bestIdx = bestIdxArr ? bestIdxArr[i] : (zStart + Math.floor((i / rect.width) * zCount));
            if (bestIdx < zStart || bestIdx > zEnd) continue;

            const isActive = !useClip || (bestIdx >= state.clipStart && bestIdx <= state.clipEnd);
            const vO = useSRHTForRange && isActive ? tFloats[bestIdx - state.clipStart] : baseFloats[bestIdx];
            const vQ = isActive ? ((state.showSRHT && tQFloats) ? tQFloats[bestIdx - state.clipStart] : qFloats[bestIdx - state.clipStart]) : baseFloats[bestIdx];

            const yV = getY(vO), yVQ = getY(vQ);

            ctx.fillStyle = isActive ? (vQ >= 0 ? accentColor : negativeColor) : inactiveColor;
            const qTop = Math.min(yCenter, yVQ);
            const qH = Math.max(1, Math.abs(yVQ - yCenter));
            ctx.fillRect(i, qTop, 1, qH);

            if (isActive && Math.abs(yV - yVQ) > 1) {
                ctx.fillStyle = errPattern;
                ctx.fillRect(i, Math.min(yV, yVQ), 1, Math.abs(yV - yVQ));
            }

            if (useImportance && isActive) {
                const sliceIdx = bestIdx - state.clipStart;
                const intensity = actImpIntensity[sliceIdx];
                const impColor = isDark ? '250, 204, 21' : '234, 179, 8';
                ctx.fillStyle = `rgba(${impColor}, ${0.1 + 0.9 * intensity})`;
                ctx.fillRect(i, bottomY, 1, impH);
            }
        }
    } else {
        for (let i = 0; i < zCount; i++) {
            const gIdx = zStart + i;
            const isActive = !useClip || (gIdx >= state.clipStart && gIdx <= state.clipEnd);
            const vO = useSRHTForRange && isActive ? tFloats[gIdx - state.clipStart] : baseFloats[gIdx];
            const vQ = isActive ? ((state.showSRHT && tQFloats) ? tQFloats[gIdx - state.clipStart] : qFloats[gIdx - state.clipStart]) : baseFloats[gIdx];

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

            if (useImportance && isActive) {
                const sliceIdx = gIdx - state.clipStart;
                const intensity = actImpIntensity[sliceIdx];
                const impColor = isDark ? '250, 204, 21' : '234, 179, 8';
                ctx.fillStyle = `rgba(${impColor}, ${0.1 + 0.9 * intensity})`;
                ctx.fillRect(x, bottomY, barW, impH);
            }
        }
    }

    const actBlockSize = stats.block_size || 0;
    const actSbSize = stats.super_block_size || 0;

    const showBlocks = actBlockSize > 0 && (barW < 1 ? (actBlockSize * barW) >= 8 : true);
    const showSupers = actSbSize > 0 && (barW < 1 ? (actSbSize * barW) >= 8 : true);

    const cMain = isDark ? '#f8fafc' : '#0f172a';
    const cSub = isDark ? 'rgba(248, 250, 252, 0.25)' : 'rgba(15, 23, 42, 0.25)';

    const drawBoundaries = (size, strokeStyle, lineWidth, skipSize = 0) => {
        ctx.strokeStyle = strokeStyle;
        ctx.lineWidth = lineWidth;
        ctx.beginPath();

        let firstSliceIdx = useClip ? zStart - state.clipStart : zStart;
        let startBoundary = Math.ceil(firstSliceIdx / size) * size;
        if (startBoundary === 0) startBoundary += size;

        for (let boundary = startBoundary; ; boundary += size) {
            let gIdx = useClip ? boundary + state.clipStart : boundary;
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

    if (doDbBar) drawDatasetBar();
    if (doInspector && state.dragStartIdx === null) updateInspector(state.lastHoveredIdx);
    updateOverlays();
}

export function updateVisualsOnly() {
    if (state.currentRenderData) {
        Object.assign(state.currentRenderData.settings, getSettings());
        render();
    }
}