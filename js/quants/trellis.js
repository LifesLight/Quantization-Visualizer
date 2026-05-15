import { getErrStats, getLloydMaxCentroids, fwht, getSignFlip, fp16 } from '../mathUtils.js';

const TRELLIS_TRANSITIONS = {
    4: [
        [{ from: 0, sub: 0 }, { from: 1, sub: 2 }],
        [{ from: 2, sub: 1 }, { from: 3, sub: 3 }],
        [{ from: 0, sub: 2 }, { from: 1, sub: 0 }],
        [{ from: 2, sub: 3 }, { from: 3, sub: 1 }]
    ],
    8: [
        [{ from: 0, sub: 0 }, { from: 1, sub: 2 }],
        [{ from: 2, sub: 1 }, { from: 3, sub: 3 }],
        [{ from: 4, sub: 2 }, { from: 5, sub: 0 }],
        [{ from: 6, sub: 3 }, { from: 7, sub: 1 }],
        [{ from: 0, sub: 2 }, { from: 1, sub: 0 }],
        [{ from: 2, sub: 3 }, { from: 3, sub: 1 }],
        [{ from: 4, sub: 0 }, { from: 5, sub: 2 }],
        [{ from: 6, sub: 1 }, { from: 7, sub: 3 }]
    ],
    16: [
        [{ from: 0, sub: 0 }, { from: 1, sub: 2 }],
        [{ from: 2, sub: 2 }, { from: 3, sub: 0 }],
        [{ from: 4, sub: 1 }, { from: 5, sub: 3 }],
        [{ from: 6, sub: 3 }, { from: 7, sub: 1 }],
        [{ from: 8, sub: 2 }, { from: 9, sub: 0 }],
        [{ from: 10, sub: 0 }, { from: 11, sub: 2 }],
        [{ from: 12, sub: 3 }, { from: 13, sub: 1 }],
        [{ from: 14, sub: 1 }, { from: 15, sub: 3 }],
        [{ from: 0, sub: 2 }, { from: 1, sub: 0 }],
        [{ from: 2, sub: 0 }, { from: 3, sub: 2 }],
        [{ from: 4, sub: 3 }, { from: 5, sub: 1 }],
        [{ from: 6, sub: 1 }, { from: 7, sub: 3 }],
        [{ from: 8, sub: 0 }, { from: 9, sub: 2 }],
        [{ from: 10, sub: 2 }, { from: 11, sub: 0 }],
        [{ from: 12, sub: 1 }, { from: 13, sub: 3 }],
        [{ from: 14, sub: 3 }, { from: 15, sub: 1 }]
    ]
};

function getTransitions(states) {
    if (states === 1) return null;
    return TRELLIS_TRANSITIONS[states] || null;
}

function findClosestInSubset(target, subVals, subIdxs) {
    const n = subVals.length;
    if (n === 0) return { val: 0, idx: -1 };
    if (n === 1) return { val: subVals[0], idx: subIdxs[0] };

    let lo = 0;
    let hi = n - 1;

    if (target <= subVals[lo]) return { val: subVals[lo], idx: subIdxs[lo] };
    if (target >= subVals[hi]) return { val: subVals[hi], idx: subIdxs[hi] };

    while (lo + 1 < hi) {
        const mid = (lo + hi) >>> 1;
        if (subVals[mid] < target) lo = mid;
        else if (subVals[mid] > target) hi = mid;
        else return { val: subVals[mid], idx: subIdxs[mid] };
    }

    const dLo = Math.abs(subVals[lo] - target);
    const dHi = Math.abs(subVals[hi] - target);
    return dLo <= dHi
        ? { val: subVals[lo], idx: subIdxs[lo] }
        : { val: subVals[hi], idx: subIdxs[hi] };
}

function buildSubsets(levels) {
    const subsets = [[], [], [], []];
    const subsetIndices = [[], [], [], []];

    for (let i = 0; i < levels.length; i++) {
        const sub = i % 4;
        subsets[sub].push(levels[i]);
        subsetIndices[sub].push(i);
    }

    return { subsets, subsetIndices };
}

function quantizeSingleState(sample, levels) {
    let bestIdx = 0;
    let bestVal = levels[0];
    let bestDist = Math.abs(sample - bestVal);

    for (let i = 1; i < levels.length; i++) {
        const d = Math.abs(sample - levels[i]);
        if (d < bestDist) {
            bestDist = d;
            bestIdx = i;
            bestVal = levels[i];
        }
    }

    return { val: bestVal, idx: bestIdx };
}

function quantizeTrellisBlock(samples, baseLevels, states) {
    const transitions = getTransitions(states);
    if (!transitions) {
        throw new Error(`Unsupported trellisStates value: ${states}`);
    }

    const { subsets, subsetIndices } = buildSubsets(baseLevels);
    const sampleCount = samples.length;

    const prevCosts = new Float64Array(states);
    prevCosts.fill(Infinity);
    prevCosts[0] = 0;

    const stepInfo = new Array(sampleCount);

    for (let t = 0; t < sampleCount; t++) {
        const x = samples[t];
        const nextCosts = new Float64Array(states);
        nextCosts.fill(Infinity);
        const perState = new Array(states);

        for (let currState = 0; currState < states; currState++) {
            let bestPrev = -1;
            let bestCbVal = 0;
            let bestCbIdx = -1;
            let bestSub = -1;
            let minCost = Infinity;
            const candidates = [];

            for (const edge of transitions[currState]) {
                const prevState = edge.from;
                const prevCost = prevCosts[prevState];
                if (!Number.isFinite(prevCost)) continue;

                const match = findClosestInSubset(x, subsets[edge.sub], subsetIndices[edge.sub]);
                const err = x - match.val;
                const total = prevCost + err * err;

                candidates.push({
                    prevState,
                    subset: edge.sub,
                    cbIdx: match.idx,
                    cbVal: match.val,
                    dist: Math.abs(err),
                    cost: total
                });

                if (total < minCost) {
                    minCost = total;
                    bestPrev = prevState;
                    bestCbVal = match.val;
                    bestCbIdx = match.idx;
                    bestSub = edge.sub;
                }
            }

            nextCosts[currState] = minCost;
            perState[currState] = {
                prevState: bestPrev,
                cbVal: bestCbVal,
                cbIdx: bestCbIdx,
                subset: bestSub,
                cost: minCost,
                candidates
            };
        }

        stepInfo[t] = {
            x,
            costs: Array.from(nextCosts),
            stateInfo: perState
        };

        prevCosts.set(nextCosts);
    }

    let finalState = 0;
    let minFinalCost = Infinity;
    for (let s = 0; s < states; s++) {
        if (prevCosts[s] < minFinalCost) {
            minFinalCost = prevCosts[s];
            finalState = s;
        }
    }

    const chunkQ = new Array(sampleCount);
    const pathData = new Array(sampleCount);

    let currState = finalState;
    for (let t = sampleCount - 1; t >= 0; t--) {
        const step = stepInfo[t].stateInfo[currState];
        if (!step || step.prevState === -1) {
            const safeMatch = quantizeSingleState(samples[t], baseLevels);
            chunkQ[t] = safeMatch.val;
            pathData[t] = {
                state: currState,
                subset: 0,
                cbIdx: safeMatch.idx,
                cwVal: safeMatch.val,
                prevState: -1,
                nextState: t === sampleCount - 1 ? 'End' : pathData[t + 1].state,
                input: samples[t],
                error: samples[t] - safeMatch.val,
                cost: Infinity,
                stateCosts: stepInfo[t].costs,
                candidates: []
            };
            currState = 0;
            continue;
        }

        const err = samples[t] - step.cbVal;
        chunkQ[t] = step.cbVal;
        pathData[t] = {
            state: currState,
            subset: step.subset,
            cbIdx: step.cbIdx,
            cwVal: step.cbVal,
            prevState: step.prevState,
            nextState: t === sampleCount - 1 ? 'End' : pathData[t + 1].state,
            input: samples[t],
            error: err,
            cost: step.cost,
            stateCosts: stepInfo[t].costs,
            candidates: step.candidates
        };
        currState = step.prevState;
    }

    return { chunkQ, pathData };
}

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
        const { trellisBits, trellisBlockSize, trellisStates, trellisCbType, trellisUseWht } = settings;

        const safeBlockSize = Math.max(1, trellisBlockSize | 0);
        const safeBits = Math.max(1, trellisBits | 0);
        const states = getTransitions(trellisStates) ? trellisStates : 1;

        const bpw = safeBits + (16 / safeBlockSize);
        const codebookSize = states === 1 ? (1 << safeBits) : (1 << (safeBits + 1));

        let baseLevels;
        if (trellisCbType === 'uniform') {
            baseLevels = new Array(codebookSize);
            for (let i = 0; i < codebookSize; i++) {
                baseLevels[i] = -3 + (6 * (i + 0.5)) / codebookSize;
            }
        } else {
            baseLevels = getLloydMaxCentroids(states === 1 ? safeBits : safeBits + 1);
        }

        const qFloats = new Float64Array(floats.length);
        const tFloats = new Float64Array(floats.length);
        const tQFloats = new Float64Array(floats.length);
        const qMathStrings = new Array(floats.length);
        const blockMeta = [];

        for (let i = 0; i < floats.length; i += safeBlockSize) {
            const chunk = floats.slice(i, i + safeBlockSize);
            const actualLen = chunk.length;

            let padLen = 1;
            while (padLen < actualLen) padLen <<= 1;

            const chunkPadded = new Array(padLen).fill(0);
            for (let j = 0; j < actualLen; j++) chunkPadded[j] = chunk[j];

            if (trellisUseWht) {
                for (let j = 0; j < padLen; j++) {
                    chunkPadded[j] *= getSignFlip(j);
                }
            }

            const chunkW = trellisUseWht ? fwht(chunkPadded) : chunkPadded.slice();

            let sumSq = 0;
            for (let j = 0; j < padLen; j++) sumSq += chunkW[j] * chunkW[j];
            const rms = fp16(Math.sqrt(sumSq / padLen) || 1e-5);

            const scaledLevels = baseLevels.map(c => c * rms);

            let chunkQ;
            let pathData;

            if (states === 1) {
                chunkQ = new Array(padLen);
                pathData = new Array(padLen);
                const allIdxs = Array.from({ length: scaledLevels.length }, (_, k) => k);

                for (let t = 0; t < padLen; t++) {
                    const match = findClosestInSubset(chunkW[t], scaledLevels, allIdxs);
                    chunkQ[t] = match.val;
                    pathData[t] = {
                        state: 0,
                        subset: 0,
                        cbIdx: match.idx,
                        cwVal: match.val,
                        prevState: 0,
                        nextState: t === padLen - 1 ? 'End' : 0,
                        input: chunkW[t],
                        error: chunkW[t] - match.val,
                        cost: Math.pow(chunkW[t] - match.val, 2),
                        stateCosts: [Math.pow(chunkW[t] - match.val, 2)],
                        candidates: [{
                            prevState: 0,
                            subset: 0,
                            cbIdx: match.idx,
                            cbVal: match.val,
                            dist: Math.abs(chunkW[t] - match.val),
                            cost: Math.pow(chunkW[t] - match.val, 2)
                        }]
                    };
                }
            } else {
                const result = quantizeTrellisBlock(chunkW, scaledLevels, states);
                chunkQ = result.chunkQ;
                pathData = result.pathData;
            }

            let chunkOut = trellisUseWht ? fwht(chunkQ) : chunkQ.slice();
            if (trellisUseWht) {
                for (let j = 0; j < padLen; j++) {
                    chunkOut[j] *= getSignFlip(j);
                }
            }

            for (let t = 0; t < actualLen; t++) {
                qFloats[i + t] = chunkOut[t];
                tFloats[i + t] = chunkW[t];
                tQFloats[i + t] = chunkQ[t];
                const p = pathData[t];

                let baseEq = states === 1 ? `CW[${p.cbIdx}]` : `S${p.prevState} &rarr; S${p.state} D${p.subset}[${p.cbIdx}]`;

                if (trellisUseWht) {
                    const signStr = getSignFlip(t) > 0 ? '+1' : '-1';
                    qMathStrings[i + t] = `D(${signStr}) &times; FWHT( ${baseEq} )[${t}]`;
                } else {
                    qMathStrings[i + t] = baseEq;
                }
            }

            blockMeta.push({
                idx: i / safeBlockSize,
                size: actualLen,
                scale: rms,
                pathData,
                ...getErrStats(chunk, chunkOut.slice(0, actualLen))
            });
        }

        const srhtStr = trellisUseWht
            ? '<span class="eq-pill" title="Diagonal Random Sign Array">D</span> &times; <span class="eq-pill" title="Orthogonal Fast Walsh-Hadamard Transform">FWHT</span> &times; '
            : '';

        const tcqStr = states > 1
            ? `<span class="eq-pill" title="Trellis Coded Quantization via Viterbi">TCQ_Path<span class="bits">${safeBits}b</span></span>`
            : `<span class="eq-pill">Codeword<span class="bits">${safeBits}b</span></span>`;

        const stateDesc = states === 1
            ? 'Independent scalar quantization.'
            : `${states}-state Viterbi path with Ungerboeck set partitioning.`;

        const transformDesc = trellisUseWht ? "SRHT applied. " : "";

        const formulaHTML = `
            <span>Weight = ${srhtStr}[ ( ${tcqStr} &times; <span class="eq-pill">RMS_Scale<span class="bits">16b</span></span> ) ]</span>
            <br><span style="color:var(--text-muted);font-size:0.8rem;">Block size ${safeBlockSize}. ${transformDesc}Every ${safeBlockSize} weights share one FP16 RMS scale.</span>`;

        return {
            qFloats,
            qMathStrings,
            bpw,
            blockMeta,
            superMeta: [],
            formulaHTML,
            tFloats: trellisUseWht ? Array.from(tFloats) : null,
            tQFloats: trellisUseWht ? Array.from(tQFloats) : null
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

        blockIdxStr = `[${bm.idx}]`;
        blockHtml = `<div class="data-row"><span>MSE:</span> <span class="val-hl">${bm.mse.toFixed(6)}</span></div>
                     <div class="data-row"><span>MAE:</span> <span>${bm.mae.toFixed(6)}</span></div>
                     <div class="data-row" style="margin-top:4px"><span>Scale (RMS FP16):</span> <span>${bm.scale.toFixed(5)}</span></div>`;

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