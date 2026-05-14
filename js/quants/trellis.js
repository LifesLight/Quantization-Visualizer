import { getErrStats, getLloydMaxCentroids, fwht, getSignFlip } from '../mathUtils.js';

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
        [{ from: 2, sub: 1 }, { from: 3, sub: 3 }],
        [{ from: 4, sub: 2 }, { from: 5, sub: 0 }],
        [{ from: 6, sub: 3 }, { from: 7, sub: 1 }],
        [{ from: 8, sub: 2 }, { from: 9, sub: 0 }],
        [{ from: 10, sub: 3 }, { from: 11, sub: 1 }],
        [{ from: 12, sub: 0 }, { from: 13, sub: 2 }],
        [{ from: 14, sub: 1 }, { from: 15, sub: 3 }],
        [{ from: 0, sub: 2 }, { from: 1, sub: 0 }],
        [{ from: 2, sub: 3 }, { from: 3, sub: 1 }],
        [{ from: 4, sub: 0 }, { from: 5, sub: 2 }],
        [{ from: 6, sub: 1 }, { from: 7, sub: 3 }],
        [{ from: 8, sub: 0 }, { from: 9, sub: 2 }],
        [{ from: 10, sub: 1 }, { from: 11, sub: 3 }],
        [{ from: 12, sub: 2 }, { from: 13, sub: 0 }],
        [{ from: 14, sub: 1 }, { from: 15, sub: 3 }]
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
    if (!Number.isFinite(prevCosts[finalState])) {
        let best = Infinity;
        for (let s = 0; s < states; s++) {
            if (prevCosts[s] < best) {
                best = prevCosts[s];
                finalState = s;
            }
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

function statePill(n, active) {
    return `<span style="padding:2px 6px; border-radius:4px; border:1px solid var(--border-color); font-size:10px; ${active ? 'background:var(--primary-color); color:white; border-color:var(--primary-color); font-weight:bold;' : 'opacity:0.55;'}">S${n}</span>`;
}

function subsetPill(n, active) {
    return `<span style="padding:2px 6px; border-radius:4px; border:1px solid var(--border-color); font-size:10px; ${active ? 'background:var(--primary-color); color:white; border-color:var(--primary-color); font-weight:bold;' : 'opacity:0.55;'}">D${n}</span>`;
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
        ui.showSuperBlockCard(true);
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
            const rms = Math.sqrt(sumSq / padLen) || 1e-9;

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

                const p = pathData[t];
                const cwStr = p.cwVal.toFixed(3);

                if (states === 1) {
                    qMathStrings[i + t] = `Codeword [Idx ${p.cbIdx}] = ${cwStr}`;
                } else {
                    qMathStrings[i + t] = `S${p.prevState} → S${p.state} via D${p.subset} [Idx ${p.cbIdx}] = ${cwStr}`;
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
            ? '<span class="eq-pill" title="Diagonal random sign matrix">D</span> &times; <span class="eq-pill" title="Orthogonal Fast Walsh-Hadamard Transform">FWHT</span> &times; '
            : '';

        const stateDesc = states === 1
            ? 'Single-state nearest-neighbor quantization.'
            : 'Viterbi decoding with a fixed block end state. The state is path memory, not output value.';

        const formulaHTML = `
            <span>Weight = ${srhtStr}[ <span class="eq-pill">TCQ_Codeword<span class="bits">${safeBits}b</span></span> &times; <span class="eq-pill">RMS_Scale<span class="bits">16b</span></span> ]</span>
            <br><span style="color:var(--text-muted);font-size:0.8rem;">Every ${safeBlockSize} weights share one RMS scale. ${stateDesc}</span>`;

        return { qFloats, qMathStrings, bpw, blockMeta, superMeta: [], formulaHTML };
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
                     <div class="data-row" style="margin-top:4px"><span>Scale (RMS):</span> <span>${bm.scale.toFixed(5)}</span></div>`;

        const localIdx = idx % settings.trellisBlockSize;
        const p = bm.pathData?.[localIdx];

        if (!p) return { blockHtml, blockIdxStr, superHtml, superIdxStr };

        const transitions = getTransitions(settings.trellisStates);
        const legalPredecessors = transitions && p.state >= 0
            ? transitions[p.state].map(edge => {
                const active = edge.from === p.prevState && edge.sub === p.subset;
                return `<span style="padding:2px 6px; border-radius:999px; border:1px solid var(--border-color); font-size:10px; ${active ? 'background:var(--primary-color); color:white; border-color:var(--primary-color); font-weight:bold;' : 'opacity:0.65;'}">S${edge.from} → D${edge.sub}</span>`;
            }).join(' ')
            : '';

        const candidateHtml = (p.candidates || []).map(c => {
            const active = c.prevState === p.prevState && c.subset === p.subset && c.cbIdx === p.cbIdx;
            return `<div style="display:flex; justify-content:space-between; gap:8px; align-items:center; padding:3px 6px; border-radius:6px; border:1px solid var(--border-color); ${active ? 'background:var(--card-bg);' : 'opacity:0.75;'}">
                <div style="display:flex; gap:4px; align-items:center; flex-wrap:wrap;">${statePill(c.prevState, active)} ${subsetPill(c.subset, active)}</div>
                <div style="font-size:11px; text-align:right; white-space:nowrap;">q=${c.cbVal.toFixed(3)}<br><span style="opacity:0.7">cost=${c.cost.toFixed(4)}</span></div>
            </div>`;
        }).join('');

        const stateCostHtml = (p.stateCosts || []).map((cost, s) => {
            const active = s === p.state;
            return `<div style="flex:1; min-width:0; text-align:center; padding:5px 4px; border-radius:6px; border:1px solid var(--border-color); ${active ? 'background:var(--primary-color); color:white; border-color:var(--primary-color);' : 'opacity:0.8;'}">
                <div style="font-size:10px; opacity:${active ? 0.9 : 0.6};">S${s}</div>
                <div style="font-size:11px; font-weight:600; overflow:hidden; text-overflow:ellipsis;">${Number.isFinite(cost) ? cost.toFixed(3) : '∞'}</div>
            </div>`;
        }).join('');

        const inputVal = Number.isFinite(p.input) ? p.input.toFixed(4) : 'n/a';
        const outVal = Number.isFinite(p.cwVal) ? p.cwVal.toFixed(4) : 'n/a';
        const errVal = Number.isFinite(p.error) ? p.error.toFixed(4) : 'n/a';

        superIdxStr = `[t=${localIdx}]`;
        superHtml = `
            <div style="display:flex; flex-direction:column; gap:10px;">
                <div style="display:flex; align-items:center; justify-content:space-between; gap:8px; padding:8px 10px; border:1px solid var(--border-color); border-radius:8px; background:var(--card-bg);">
                    <div style="min-width:0;">
                        <div style="font-size:11px; opacity:0.65;">CURRENT STEP</div>
                        <div style="font-weight:700;">State S${p.state}</div>
                    </div>
                    <div style="text-align:right; font-size:11px; line-height:1.4;">
                        <div><span style="opacity:0.65;">x<t>:</span> ${inputVal}</div>
                        <div><span style="opacity:0.65;">q<t>:</span> ${outVal}</div>
                        <div><span style="opacity:0.65;">e<t>:</span> ${errVal}</div>
                    </div>
                </div>

                <div style="font-size:11px; line-height:1.35; color:var(--text-muted);">
                    A trellis state is path memory. It does not quantize by itself. It only decides which subset choices are legal next.
                </div>

                ${settings.trellisStates > 1 ? `
                <div>
                    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:6px; font-size:11px; opacity:0.7;">
                        <span>LEGAL PREDECESSORS INTO THIS STATE</span>
                    </div>
                    <div style="display:flex; flex-wrap:wrap; gap:6px; align-items:center;">
                        ${legalPredecessors}
                    </div>
                </div>
                ` : ''}

                <div class="data-row" style="margin-top:2px;">
                    <span>Chosen Path:</span>
                    <div style="text-align:right; font-size:12px;">S${p.prevState} → S${p.state} via D${p.subset}</div>
                </div>

                <div>
                    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:6px; font-size:11px; opacity:0.7;">
                        <span>STATE COSTS AFTER THIS SAMPLE</span>
                        <span>winning cost: ${Number.isFinite(p.cost) ? p.cost.toFixed(4) : '∞'}</span>
                    </div>
                    <div style="display:grid; grid-template-columns:repeat(${Math.max(2, settings.trellisStates)}, minmax(0,1fr)); gap:6px;">
                        ${stateCostHtml}
                    </div>
                </div>

                <div>
                    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:6px; font-size:11px; opacity:0.7;">
                        <span>CHOICES AT THIS STEP</span>
                        <span>${settings.trellisStates > 1 ? `picked D${p.subset}` : 'nearest level'}</span>
                    </div>
                    <div style="display:flex; flex-direction:column; gap:6px; max-height:140px; overflow:auto; padding-right:2px;">
                        ${candidateHtml || '<div style="opacity:0.6; font-size:11px;">No candidates</div>'}
                    </div>
                </div>

                <div class="data-row" style="margin-top:2px;">
                    <span>Data Stored:</span>
                    <div style="text-align:right; font-size:12px;">
                        ${settings.trellisStates > 1
                ? `<span class="val-hl">1b</span> path + <span class="val-hl">${Math.max(0, settings.trellisBits - 1)}b</span> index`
                : `<span class="val-hl">${settings.trellisBits}b</span> index`}
                    </div>
                </div>
            </div>
        `;

        return { blockHtml, blockIdxStr, superHtml, superIdxStr };
    }
};