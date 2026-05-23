/**
 * Render Components
 * Manages the UI overlays, minimap dataset bar, and sidebar inspector HTML building.
 */
import { state, getBaseFloats } from './state.js';
import { elements } from '../ui.js';
import { getF32Array } from '../wasmWrapper.js';

export function resizeCanvasCssOnly() {
    const canvas = document.getElementById('main-canvas');
    if (!canvas || !elements.chartArea) return;
    const rect = elements.chartArea.getBoundingClientRect();
    canvas.style.width = `${rect.width}px`;
    canvas.style.height = `${rect.height}px`;
    updateOverlays();
}

/**
 * Updates DOM overlays (blocks, supers, hover, drag bounding boxes) on top of the canvas.
 */
export function updateOverlays() {
    const hoverEl = document.getElementById('hover-overlay');
    const blockEl = document.getElementById('block-overlay');
    const superEl = document.getElementById('super-overlay');
    const dragEl = document.getElementById('drag-overlay');

    if (!state.currentRenderData || !hoverEl) return;

    const { stats, baseLen } = state.currentRenderData;
    const zStart = state.zoomRange ? state.zoomRange.start : 0;
    const zEnd = state.zoomRange ? state.zoomRange.end : baseLen - 1;
    const zCount = zEnd - zStart + 1;

    const rect = elements.chartArea.getBoundingClientRect();
    const barW = rect.width / zCount;

    const useClip = elements.dbModeClip && elements.dbModeClip.checked;
    const isDark = document.documentElement.getAttribute('data-theme') === 'dark';

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

    if (state.dragStartIdx !== null && state.dragCurrentIdx !== null) {
        const sIdx = Math.min(state.dragStartIdx, state.dragCurrentIdx);
        const eIdx = Math.max(state.dragStartIdx, state.dragCurrentIdx);
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

    hoverEl.style.display = 'none';
    blockEl.style.display = 'none';
    superEl.style.display = 'none';

    const primaryIdx = state.hoveredIdx !== null ? state.hoveredIdx : state.dragStartIdx;

    if (primaryIdx !== null && primaryIdx >= zStart && primaryIdx <= zEnd) {
        const { x, w } = getXW(primaryIdx, primaryIdx);
        hoverEl.style.display = 'block';
        hoverEl.style.left = `${x}px`;
        hoverEl.style.width = `${w}px`;
        hoverEl.style.backgroundColor = 'var(--hover-overlay-bg)';

        const sliceIdx = useClip ? primaryIdx - state.clipStart : primaryIdx;
        if (sliceIdx >= 0 && sliceIdx < state.currentRenderData.actLen) {
            const actBlockSize = stats.block_size || 0;
            const actSbSize = stats.super_block_size || 0;
            const showBlocks = actBlockSize > 0 && (barW < 1 ? (actBlockSize * barW) >= 8 : true);
            const showSupers = actSbSize > 0 && (barW < 1 ? (actSbSize * barW) >= 8 : true);

            if (showSupers) {
                const startSlice = Math.floor(sliceIdx / actSbSize) * actSbSize;
                const endSlice = startSlice + actSbSize - 1;
                const dStart = Math.max(zStart, useClip ? startSlice + state.clipStart : startSlice);
                const dEnd = Math.min(zEnd, useClip ? endSlice + state.clipStart : endSlice);

                if (dStart <= dEnd) {
                    const xw = getXW(dStart, dEnd);
                    superEl.style.display = 'block';
                    superEl.style.left = `${xw.x}px`;
                    superEl.style.width = `${xw.w}px`;
                    superEl.style.backgroundColor = 'var(--super-overlay-bg)';
                }
            }

            if (showBlocks) {
                const startSlice = Math.floor(sliceIdx / actBlockSize) * actBlockSize;
                const endSlice = startSlice + actBlockSize - 1;
                const dStart = Math.max(zStart, useClip ? startSlice + state.clipStart : startSlice);
                const dEnd = Math.min(zEnd, useClip ? endSlice + state.clipStart : endSlice);

                if (dStart <= dEnd) {
                    const xw = getXW(dStart, dEnd);
                    blockEl.style.display = 'block';
                    blockEl.style.left = `${xw.x}px`;
                    blockEl.style.width = `${xw.w}px`;
                    blockEl.style.backgroundColor = 'var(--block-overlay-bg)';

                    const borderColor = 'var(--highlight-border)';
                    blockEl.style.borderLeft = dStart >= zStart ? `1px solid ${borderColor}` : 'none';
                    blockEl.style.borderRight = dEnd + 1 <= zEnd ? `1px solid ${borderColor}` : 'none';
                }
            }
        }
    }
}

export function updateDbBarVisibility() {
    const checked = (elements.dbModeClip && elements.dbModeClip.checked) ||
        (elements.dbModeMinMax && elements.dbModeMinMax.checked) ||
        (elements.dbModeHotspots && elements.dbModeHotspots.checked);

    if (elements.dbContainer) elements.dbContainer.style.display = checked ? 'flex' : 'none';
    if (elements.dbBar) elements.dbBar.style.display = checked ? 'block' : 'none';
    if (checked) {
        requestAnimationFrame(() => drawDatasetBar());
    }
}

/**
 * Renders the small Dataset Overview Bar (Minimap)
 */
export function drawDatasetBar() {
    if (!elements.dbBar || elements.dbBar.style.display === 'none') return;

    const baseFloats = getBaseFloats();
    if (!baseFloats || baseFloats.length === 0) return;

    const N = baseFloats.length;
    const zS = state.zoomRange ? state.zoomRange.start : 0;
    const zE = state.zoomRange ? state.zoomRange.end : N - 1;
    const zCount = zE - zS + 1;

    const getZoomPct = (idx) => Math.max(0, Math.min(100, ((idx - zS) / (zCount - 1)) * 100)) || 0;
    const getZoomPctUnclamped = (idx) => ((idx - zS) / (zCount - 1)) * 100 || 0;

    const useClip = elements.dbModeClip && elements.dbModeClip.checked;
    const useMinMax = elements.dbModeMinMax && elements.dbModeMinMax.checked;
    const useHotspots = elements.dbModeHotspots && elements.dbModeHotspots.checked;

    if (useClip) {
        let leftPct = getZoomPct(state.clipStart);
        let rightPct = getZoomPct(state.clipEnd);

        elements.dbActiveRegion.style.left = `${leftPct}%`;
        elements.dbActiveRegion.style.width = `${rightPct - leftPct}%`;
        elements.dbActiveRegion.style.background = 'rgba(79, 70, 229, 0.15)';
        elements.dbActiveRegion.style.borderLeft = '1px solid var(--primary-color)';
        elements.dbActiveRegion.style.borderRight = '1px solid var(--primary-color)';

        if (elements.dbHandleLeft) elements.dbHandleLeft.style.display = 'block';
        if (elements.dbHandleRight) elements.dbHandleRight.style.display = 'block';

        if (elements.dbClipStart && document.activeElement !== elements.dbClipStart) elements.dbClipStart.value = state.clipStart;
        if (elements.dbClipWidth && document.activeElement !== elements.dbClipWidth) elements.dbClipWidth.value = (state.clipEnd - state.clipStart + 1);
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
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, rect.width, rect.height);

    if (useMinMax) {
        const mm = state.backend.get_global_minmax();
        const minPct = getZoomPctUnclamped(mm.min_idx);

        if (minPct >= 0 && minPct <= 100) {
            elements.dbMinArrow.style.display = 'block';
            elements.dbMinArrow.style.left = `${minPct}%`;
        } else {
            elements.dbMinArrow.style.display = 'none';
        }

        const maxPct = getZoomPctUnclamped(mm.max_idx);
        if (maxPct >= 0 && maxPct <= 100) {
            elements.dbMaxArrow.style.display = 'block';
            elements.dbMaxArrow.style.left = `${maxPct}%`;
        } else {
            elements.dbMaxArrow.style.display = 'none';
        }
    } else {
        elements.dbMinArrow.style.display = 'none';
        elements.dbMaxArrow.style.display = 'none';
    }

    if (useHotspots && state.currentRenderData) {
        const floats = getF32Array(state.currentRenderData.actPtr, state.currentRenderData.actLen);
        const qFloats = getF32Array(state.currentRenderData.qPtr, state.currentRenderData.actLen);

        let leftPct = useClip ? getZoomPct(state.clipStart) : 0;
        let rightPct = useClip ? getZoomPct(state.clipEnd) : 100;

        const startX = Math.max(0, (leftPct / 100) * rect.width);
        const endX = Math.min(rect.width, (rightPct / 100) * rect.width);
        const pxWidth = endX - startX;

        if (pxWidth > 0) {
            const bins = Math.ceil(pxWidth);
            let maxSE = 0;
            const seArr = new Float32Array(bins);

            for (let b = 0; b < bins; b++) {
                const px = startX + b;
                let iGlobal = zS + (px / rect.width) * (zCount - 1);
                let sliceIdx = useClip ? Math.floor(iGlobal - state.clipStart) : Math.floor(iGlobal);

                if (sliceIdx >= 0 && sliceIdx < floats.length) {
                    const err = floats[sliceIdx] - qFloats[sliceIdx];
                    const se = err * err;
                    seArr[b] = se;
                    if (se > maxSE) maxSE = se;
                }
            }

            if (maxSE > 0) {
                for (let b = 0; b < bins; b++) {
                    if (seArr[b] > 0) {
                        ctx.fillStyle = `rgba(239, 68, 68, ${Math.min(1, seArr[b] / maxSE)})`;
                        ctx.fillRect(startX + b, 0, 1, rect.height);
                    }
                }
            }
        }
    }
}

/**
 * Updates the Data Inspector sidebar (Hover details).
 */
export function updateInspector(idx) {
    state.lastHoveredIdx = idx;
    if (!state.currentRenderData) return;

    const N = state.currentRenderData.baseLen;
    if (N === 0) return;
    if (idx < 0 || idx >= N) idx = 0;

    elements.iWIdx.textContent = `[${idx}]`;

    const useClip = elements.dbModeClip && elements.dbModeClip.checked;
    const baseFloats = getF32Array(state.currentRenderData.basePtr, state.currentRenderData.baseLen);

    if (useClip && (idx < state.clipStart || idx > state.clipEnd)) {
        const val = baseFloats[idx];
        elements.insWData.innerHTML = `<div class="data-row"><span>Original:</span> <span class="val-hl" title="${val}">${val.toFixed(5)}</span></div><div class="empty-state" style="padding:10px 0;">Inactive Weight</div>`;
        elements.iBIdx.textContent = '[-]';
        elements.insBData.innerHTML = '<div class="empty-state">Inactive</div>';
        elements.iSBIdx.textContent = '[-]';
        elements.insSBData.innerHTML = '<div class="empty-state">Inactive</div>';
        return;
    }

    const sliceIdx = useClip ? idx - state.clipStart : idx;
    const insData = state.backend.get_inspector_data(sliceIdx, state.currentRenderData.settings);
    const { stats, actLen } = state.currentRenderData;

    const floats = getF32Array(state.currentRenderData.actPtr, actLen);
    const qFloats = getF32Array(state.currentRenderData.qPtr, actLen);
    const tFloats = stats.has_srht ? getF32Array(state.currentRenderData.tPtr, actLen) : null;
    const tQFloats = stats.has_srht ? getF32Array(state.currentRenderData.tQPtr, actLen) : null;

    const val = state.showSRHT && tFloats ? tFloats[sliceIdx] : floats[sliceIdx];
    const valQ = state.showSRHT && tQFloats ? tQFloats[sliceIdx] : qFloats[sliceIdx];

    let mathHtml = insData.mathStr ? `<div class="data-row" style="margin-top:4px; color:var(--text-muted);"><span>Math:</span> <span style="color:var(--text-main);">${insData.mathStr}</span></div>` : '';
    const lblOrig = state.showSRHT ? "Transformed:" : "Original:";
    const lblQuant = state.showSRHT ? "Quantized (WHT):" : "Quantized:";
    const errVal = Math.abs(val - valQ);

    let baseHtml = `<div class="data-row"><span>${lblOrig}</span> <span class="val-hl" title="${val}">${val.toFixed(5)}</span></div><div class="data-row"><span>${lblQuant}</span> <span class="val-hl" title="${valQ}">${valQ.toFixed(5)}</span></div>${mathHtml}<div class="data-row" style="margin-top:4px"><span>Abs Error:</span> <span title="${errVal}">${errVal.toFixed(6)}</span></div>`;

    if (insData.importanceRaw !== undefined) {
        baseHtml += `<div class="data-row" style="margin-top:4px" title="Relative to the max value in the block: ${insData.importancePctMax.toFixed(2)}%"><span>Importance:</span> <span>${insData.importancePctSum.toFixed(2)}%</span></div>`;
    }

    elements.insWData.innerHTML = baseHtml;

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

    // --- Advanced Metadata Rendering (Trellis / Superblocks / Globals / IQ) ---
    let superHtml = '';

    if (insData.trellisJson) {
        const p = JSON.parse(insData.trellisJson);
        const states = state.currentRenderData.settings.trellisStates || 1;

        let candHtml = '';

        let candsList = p.candidates || [];

        // PADDING LOGIC: If a Viterbi branch was pruned (length is 1), 
        // inject a dummy None branch so the UI visually stacks up properly.
        if (states > 1 && candsList.length === 1) {
            candsList.push({ isNone: true });
        }

        if (candsList.length > 0) {
            for (let c of candsList) {
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
            candHtml = '<div style="opacity:0.6; font-size:11px; padding: 4px 0;">No candidates (Initial State)</div>';
        }

        elements.iSBIdx.textContent = `[t=${sliceIdx % (state.currentRenderData.settings.trellisBlockSize || 1)}]`;
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
    } else if (insData.iqHtml) {
        elements.iSBIdx.textContent = `[IQ Map]`;
        elements.insSBData.innerHTML = insData.iqHtml;
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
        elements.insSBData.innerHTML = '<div class="empty-state">Hover over an item to inspect details</div>';
    }
}