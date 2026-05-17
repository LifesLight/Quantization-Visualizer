import { getErrStats, fwht, getSignFlip, fp16, getLloydMaxCentroids } from '../mathUtils.js';
import { getWasm } from '../wasmWrapper.js';

export default {
    id: 'trellis',
    label: 'Trellis Quantization (TCQ)',

    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showTrellisSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(true, 'Trellis Overview');
    },

    quantize(floats, settings) {
        const { trellisBits, trellisBlockSize, trellisStates, trellisCbType, trellisUseWht, trellisWhtScope, trellisOptIters, trellisSignSeed } = settings;

        const safeBlockSize = Math.max(1, trellisBlockSize | 0);
        const safeBits = Math.max(1, trellisBits | 0);
        const states = [1, 4, 8, 16, 64, 256].includes(trellisStates) ? trellisStates : 1;
        const codebookSize = states === 1 ? (1 << safeBits) : (1 << (safeBits + 1));

        let baseLevels;
        if (trellisCbType === 'uniform') {
            baseLevels = new Array(codebookSize);
            for (let i = 0; i < codebookSize; i++) baseLevels[i] = -3 + (6 * (i + 0.5)) / codebookSize;
        } else {
            baseLevels = getLloydMaxCentroids(states === 1 ? safeBits : safeBits + 1, trellisCbType);
        }

        const isGlobalWht = trellisUseWht && trellisWhtScope === 'global';
        const needsLocalWht = trellisUseWht && trellisWhtScope === 'local';

        const tFloatsOut = trellisUseWht ? new Float64Array(floats.length) : null;
        const tQFloatsOut = trellisUseWht ? new Float64Array(floats.length) : null;

        let processFloats = floats.slice();
        let globalPadLen = floats.length;
        let globalRms = 1e-5;

        if (isGlobalWht) {
            globalPadLen = 1;
            while (globalPadLen < floats.length) globalPadLen <<= 1;
            const padded = new Float64Array(globalPadLen);
            padded.set(floats);
            for (let j = 0; j < globalPadLen; j++) padded[j] *= getSignFlip(j, trellisSignSeed);
            processFloats = fwht(Array.from(padded));

            let globalSumSq = 0;
            for (let i = 0; i < globalPadLen; i++) {
                globalSumSq += processFloats[i] * processFloats[i];
            }
            globalRms = fp16(Math.sqrt(globalSumSq / globalPadLen) || 1e-5);

            if (trellisOptIters > 0) {
                const wasm = getWasm();
                const optimal = wasm.wasm_trellis_opt_scale(
                    new Float64Array(processFloats),
                    new Float64Array(baseLevels),
                    states,
                    globalRms * 0.1,
                    globalRms * 2.5,
                    trellisOptIters,
                    safeBlockSize
                );
                globalRms = fp16(optimal);
            }
        }

        const len = isGlobalWht ? globalPadLen : floats.length;
        const qProcessOut = new Float64Array(len);
        const blockMeta = [];

        for (let i = 0; i < len; i += safeBlockSize) {
            const chunk = processFloats.slice(i, i + safeBlockSize);
            const actualLen = chunk.length;

            let chunkW = Array.from(chunk);
            let padLen = actualLen;

            if (needsLocalWht) {
                padLen = 1;
                while (padLen < actualLen) padLen <<= 1;
                const chunkPadded = new Array(padLen).fill(0);
                for (let j = 0; j < actualLen; j++) chunkPadded[j] = chunk[j];
                for (let j = 0; j < padLen; j++) chunkPadded[j] *= getSignFlip(i + j, trellisSignSeed);
                chunkW = fwht(chunkPadded);
            }

            let optScale;

            if (isGlobalWht) {
                optScale = globalRms;
            } else {
                let sumSq = 0;
                for (let j = 0; j < actualLen; j++) sumSq += chunk[j] * chunk[j];
                const rms = fp16(Math.sqrt(sumSq / actualLen) || 1e-5);
                optScale = rms;

                if (trellisOptIters > 0) {
                    const wasm = getWasm();
                    const optimal = wasm.wasm_trellis_opt_scale(
                        new Float64Array(chunkW),
                        new Float64Array(baseLevels),
                        states,
                        rms * 0.1,
                        rms * 2.5,
                        trellisOptIters,
                        0 // 0 means treat as one chunk in rust
                    );
                    optScale = fp16(optimal);
                }
            }

            const wasm = getWasm();
            const result = wasm.wasm_trellis_quantize_block(
                new Float64Array(chunkW),
                new Float64Array(baseLevels),
                states,
                optScale
            );

            let chunkQ = result.chunkQ;
            let pathData = result.pathData;

            if (needsLocalWht) {
                for (let t = 0; t < actualLen; t++) {
                    if (i + t < floats.length) {
                        tFloatsOut[i + t] = chunkW[t];
                        tQFloatsOut[i + t] = chunkQ[t];
                    }
                }
            }

            let chunkOut = chunkQ;
            if (needsLocalWht) {
                chunkOut = fwht(Array.from(chunkQ));
                for (let j = 0; j < padLen; j++) chunkOut[j] *= getSignFlip(i + j, trellisSignSeed);
            }

            for (let t = 0; t < actualLen; t++) qProcessOut[i + t] = chunkOut[t];

            if (i < floats.length) {
                blockMeta.push({
                    idx: i / safeBlockSize,
                    size: actualLen,
                    scale: optScale,
                    pathData: pathData.slice(0, actualLen),
                    ...getErrStats(chunk, chunkOut.slice(0, actualLen))
                });
            }
        }

        const qFloats = new Float64Array(floats.length);
        if (isGlobalWht) {
            const invGlobal = fwht(Array.from(qProcessOut));
            for (let j = 0; j < globalPadLen; j++) invGlobal[j] *= getSignFlip(j, trellisSignSeed);
            for (let j = 0; j < floats.length; j++) {
                qFloats[j] = invGlobal[j];
                tFloatsOut[j] = processFloats[j];
                tQFloatsOut[j] = qProcessOut[j];
            }
        } else {
            for (let j = 0; j < floats.length; j++) qFloats[j] = qProcessOut[j];
        }

        const qMathStrings = new Array(floats.length);
        for (let i = 0; i < floats.length; i++) {
            const blockIndex = Math.floor(i / safeBlockSize);
            const localIdx = i % safeBlockSize;
            const p = blockMeta[blockIndex]?.pathData?.[localIdx];
            if (p) {
                const baseEq = states === 1 ? `CW[${p.cbIdx}]` : `S${p.prevState} &rarr; S${p.state} D${p.subset}[${p.cbIdx}]`;
                qMathStrings[i] = trellisUseWht
                    ? `D(${getSignFlip(i, trellisSignSeed) > 0 ? '+1' : '-1'}) &times; FWHT( ${baseEq} )[${i}]`
                    : baseEq;
            } else {
                qMathStrings[i] = "Hidden";
            }
        }

        const strictLinearBpw = isGlobalWht
            ? safeBits + (16 / floats.length)
            : safeBits + (16 / safeBlockSize);

        const srhtStr = trellisUseWht
            ? `<span class="eq-pill" title="Diagonal Random Sign Array">D</span> &times; <span class="eq-pill" title="Orthogonal Fast Walsh-Hadamard Transform">FWHT</span> &times; `
            : '';

        const tcqStr = states > 1
            ? `<span class="eq-pill" title="Trellis Coded Quantization via Viterbi">TCQ_Path<span class="bits">${safeBits}b</span></span>`
            : `<span class="eq-pill">Codeword<span class="bits">${safeBits}b</span></span>`;

        const scalePill = isGlobalWht ? 'Global_Scale' : 'RMS_Scale';

        const transformDesc = trellisUseWht
            ? (isGlobalWht
                ? `Global QuIP# SRHT applied. All ${floats.length} weights share a single FP16 Global Scale.`
                : `Local Block SRHT applied. Every ${safeBlockSize} weights share one FP16 Optimal Scale.`)
            : `Every ${safeBlockSize} weights share one FP16 Optimal Scale.`;

        const formulaHTML = `
            <span>Weight = ${srhtStr}[ ( ${tcqStr} &times; <span class="eq-pill">${scalePill}<span class="bits">16b</span></span> ) ]</span>
            <br><span style="color:var(--text-muted);font-size:0.8rem;">Block size ${safeBlockSize}. ${transformDesc}</span>`;

        return {
            qFloats,
            qMathStrings,
            bpw: strictLinearBpw,
            blockMeta,
            superMeta: [],
            formulaHTML,
            tFloats: tFloatsOut ? Array.from(tFloatsOut) : null,
            tQFloats: tQFloatsOut ? Array.from(tQFloatsOut) : null
        };
    },

    buildElements(floats, qFloats, settings, createBar) {
        const frag = document.createDocumentFragment();
        let bIdx = 0;

        for (let i = 0; i < floats.length; i += settings.trellisBlockSize) {
            const chunk = floats.slice(i, i + settings.trellisBlockSize);
            const grp = document.createElement('div');
            grp.className = 'block-group';
            grp.style.flex = chunk.length;
            grp.dataset.bIdx = bIdx++;
            chunk.forEach((v, j) => grp.appendChild(createBar(v, qFloats[i + j], i + j)));
            frag.appendChild(grp);
        }

        return frag;
    },

    formatInspector(idx, blockMeta, superMeta, settings) {
        const bIdx = Math.floor(idx / settings.trellisBlockSize);
        const bm = blockMeta[bIdx];

        let blockHtml = '';
        let blockIdxStr = '';
        let superHtml = '';
        let superIdxStr = '';

        if (!bm) return { blockHtml, blockIdxStr, superHtml, superIdxStr };

        const isGlobal = settings.trellisUseWht && settings.trellisWhtScope === 'global';

        blockIdxStr = `[${bm.idx}]`;
        blockHtml = `<div class="data-row"><span>MSE:</span> <span class="val-hl">${bm.mse.toFixed(6)}</span></div>
                     <div class="data-row"><span>MAE:</span> <span>${bm.mae.toFixed(6)}</span></div>
                     <div class="data-row" style="margin-top:4px"><span>${isGlobal ? 'Global Scale (FP16)' : 'Scale (FP16)'}:</span> <span>${bm.scale.toFixed(5)}</span></div>`;

        const localIdx = idx % settings.trellisBlockSize;
        const p = bm.pathData?.[localIdx];

        if (!p) return { blockHtml, blockIdxStr, superHtml, superIdxStr };

        const candidateHtml = (p.candidates || []).map(c => {
            const active = c.prevState === p.prevState && c.subset === p.subset && c.cbIdx === p.cbIdx;
            return `<div style="display:flex; justify-content:space-between; gap:8px; align-items:center; padding:5px 8px; border-radius:6px; border:1px solid ${active ? 'var(--primary-color)' : 'var(--border-color)'}; ${active ? 'background:var(--card-bg);' : 'opacity:0.6;'}">
                <div style="display:flex; gap:6px; align-items:center; font-size:11px; ${active ? 'color:var(--primary-color); font-weight:bold;' : ''}">
                    <span>S${c.prevState} &rarr; D${c.subset}</span>
                </div>
                <div style="font-size:11px; display:flex; gap:8px;">
                    <span style="width:45px; text-align:right;">q=${c.cbVal.toFixed(3)}</span>
                    <span style="opacity:0.6; width:65px; text-align:right;">(c=${c.cost.toFixed(3)})</span>
                </div>
            </div>`;
        }).join('');

        superIdxStr = `[t=${localIdx}]`;
        superHtml = `
            <div style="display:flex; flex-direction:column; gap:12px;">
                ${settings.trellisStates > 1 ? `
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
                            <div style="font-size:9px; opacity:0.6; margin-top:6px;">Path Cost: ${Number.isFinite(p.cost) ? p.cost.toFixed(3) : '∞'}</div>
                        </div>
                        <div style="text-align:center; flex: 0 0 auto;">
                            <div style="font-size:9px; opacity:0.6; margin-bottom:6px; letter-spacing:0.5px;">CURR</div>
                            <div style="background:var(--primary-color); color:white; border:2px solid var(--primary-color); border-radius:50%; width:28px; height:28px; display:flex; align-items:center; justify-content:center; font-weight:bold; font-size:11px; margin:0 auto;">S${p.state}</div>
                        </div>
                    </div>
                </div>

                <div>
                    <div style="font-size:10px; font-weight:600; opacity:0.6; margin-bottom:6px; text-transform:uppercase; letter-spacing:0.5px;">Evaluated Branches (to S${p.state})</div>
                    <div style="display:flex; flex-direction:column; gap:4px; max-height:140px; overflow-y:scroll; padding-right:4px;">
                        ${candidateHtml || '<div style="opacity:0.6; font-size:11px;">No candidates</div>'}
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

        return { blockHtml, blockIdxStr, superHtml, superIdxStr };
    }
};