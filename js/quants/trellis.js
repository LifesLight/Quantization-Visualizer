import { getErrStats, getLloydMaxCentroids, fwht, getSignFlip, fp16 } from '../mathUtils.js';

const transitionsCache = {};

// Algorithmically generates optimal Ungerboeck Trellis State transitions using 
// standard systematic parity-check polynomials. Guarantees maximum free Euclidean 
// distance for 4, 8, 16, 64, and 256 states.
function getTransitions(states) {
    if (states === 1) return null;
    if (transitionsCache[states]) return transitionsCache[states];

    const K = Math.log2(states);
    let poly_LSB, poly_MSB;

    switch (states) {
        case 4: poly_LSB = 0x02; poly_MSB = 0x01; break;
        case 8: poly_LSB = 0x02; poly_MSB = 0x05; break;
        case 16: poly_LSB = 0x04; poly_MSB = 0x0B; break;
        case 64: poly_LSB = 0x10; poly_MSB = 0x2D; break;
        case 256: poly_LSB = 0x40; poly_MSB = 0x95; break;
        default: return null;
    }

    const transitions = Array.from({ length: states }, () => []);

    function popcount(x) {
        let c = 0;
        for (; x > 0; x >>= 1) c += x & 1;
        return c & 1;
    }

    for (let state = 0; state < states; state++) {
        for (let input = 0; input <= 1; input++) {
            const next_state = (state >> 1) | (input << (K - 1));
            const sub_LSB = popcount(state & poly_LSB);
            const sub_MSB = input ^ popcount(state & poly_MSB);
            const sub = (sub_MSB << 1) | sub_LSB;
            transitions[next_state].push({ from: state, sub: sub });
        }
    }

    transitionsCache[states] = transitions;
    return transitions;
}

function buildSubsetIndices(levels) {
    const indices = [[], [], [], []];
    for (let i = 0; i < levels.length; i++) {
        indices[i % 4].push(i);
    }
    return indices;
}

function findClosestIdx(target, subsetIdxs, levels) {
    let bestIdx = subsetIdxs[0];
    let bestDist = Math.abs(target - levels[bestIdx]);
    for (let i = 1; i < subsetIdxs.length; i++) {
        const idx = subsetIdxs[i];
        const d = Math.abs(target - levels[idx]);
        if (d < bestDist) {
            bestDist = d;
            bestIdx = idx;
        }
    }
    return { val: levels[bestIdx], idx: bestIdx };
}

function quantizeTrellisBlock(samples, baseLevels, states, scale, subsetIndices, allIdxs) {
    const transitions = getTransitions(states);
    const sampleCount = samples.length;

    const scaledLevels = new Float64Array(baseLevels.length);
    for (let i = 0; i < baseLevels.length; i++) scaledLevels[i] = baseLevels[i] * scale;

    const prevCosts = new Float64Array(states).fill(Infinity);
    prevCosts[0] = 0;

    const stepInfo = new Array(sampleCount);

    for (let t = 0; t < sampleCount; t++) {
        const x = samples[t];
        const nextCosts = new Float64Array(states).fill(Infinity);
        const perState = new Array(states);

        for (let currState = 0; currState < states; currState++) {
            let bestPrev = -1, bestCbVal = 0, bestCbIdx = -1, bestSub = -1;
            let minCost = Infinity;
            const candidates = [];

            for (const edge of transitions[currState]) {
                const prevCost = prevCosts[edge.from];
                if (!Number.isFinite(prevCost)) continue;

                const match = findClosestIdx(x, subsetIndices[edge.sub], scaledLevels);
                const err = x - match.val;
                const total = prevCost + err * err;

                candidates.push({
                    prevState: edge.from, subset: edge.sub, cbIdx: match.idx,
                    cbVal: match.val, dist: Math.abs(err), cost: total
                });

                if (total < minCost) {
                    minCost = total;
                    bestPrev = edge.from;
                    bestCbVal = match.val;
                    bestCbIdx = match.idx;
                    bestSub = edge.sub;
                }
            }

            nextCosts[currState] = minCost;
            perState[currState] = { prevState: bestPrev, cbVal: bestCbVal, cbIdx: bestCbIdx, subset: bestSub, cost: minCost, candidates };
        }

        stepInfo[t] = { x, costs: Array.from(nextCosts), stateInfo: perState };
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

    const chunkQ = new Float64Array(sampleCount);
    const pathData = new Array(sampleCount);

    let currState = finalState;
    for (let t = sampleCount - 1; t >= 0; t--) {
        const step = stepInfo[t].stateInfo[currState];
        if (!step || step.prevState === -1) {
            const safeMatch = findClosestIdx(samples[t], allIdxs, scaledLevels);
            chunkQ[t] = safeMatch.val;
            pathData[t] = {
                state: currState, subset: 0, cbIdx: safeMatch.idx, cwVal: safeMatch.val,
                prevState: -1, nextState: t === sampleCount - 1 ? 'End' : pathData[t + 1].state,
                input: samples[t], error: samples[t] - safeMatch.val, cost: Infinity,
                stateCosts: stepInfo[t].costs, candidates: []
            };
            currState = 0;
            continue;
        }

        chunkQ[t] = step.cbVal;
        pathData[t] = {
            state: currState, subset: step.subset, cbIdx: step.cbIdx, cwVal: step.cbVal,
            prevState: step.prevState, nextState: t === sampleCount - 1 ? 'End' : pathData[t + 1].state,
            input: samples[t], error: samples[t] - step.cbVal, cost: step.cost,
            stateCosts: stepInfo[t].costs, candidates: step.candidates
        };
        currState = step.prevState;
    }

    return { chunkQ, pathData, cost: minFinalCost };
}

function evaluateScaleViterbi(samples, baseLevels, states, scale, subsetIndices, scaledBuf) {
    for (let i = 0; i < baseLevels.length; i++) scaledBuf[i] = baseLevels[i] * scale;

    if (states === 1) {
        let cost = 0;
        for (let i = 0; i < samples.length; i++) {
            let bestDist = Infinity;
            for (let l = 0; l < scaledBuf.length; l++) {
                const d = Math.abs(samples[i] - scaledBuf[l]);
                if (d < bestDist) bestDist = d;
            }
            cost += bestDist * bestDist;
        }
        return cost;
    }

    const transitions = getTransitions(states);
    const prevCosts = new Float64Array(states).fill(Infinity);
    prevCosts[0] = 0;

    for (let t = 0; t < samples.length; t++) {
        const x = samples[t];
        const nextCosts = new Float64Array(states).fill(Infinity);
        for (let currState = 0; currState < states; currState++) {
            let minCost = Infinity;
            for (const edge of transitions[currState]) {
                const prevCost = prevCosts[edge.from];
                if (!Number.isFinite(prevCost)) continue;
                const match = findClosestIdx(x, subsetIndices[edge.sub], scaledBuf);
                const err = x - match.val;
                const total = prevCost + err * err;
                if (total < minCost) minCost = total;
            }
            nextCosts[currState] = minCost;
        }
        prevCosts.set(nextCosts);
    }
    return Math.min(...prevCosts);
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
        const { trellisBits, trellisBlockSize, trellisStates, trellisCbType, trellisUseWht, trellisWhtScope, trellisOptIters, trellisSignSeed } = settings;

        const safeBlockSize = Math.max(1, trellisBlockSize | 0);
        const safeBits = Math.max(1, trellisBits | 0);
        const states = getTransitions(trellisStates) ? trellisStates : 1;
        const codebookSize = states === 1 ? (1 << safeBits) : (1 << (safeBits + 1));

        let baseLevels;
        if (trellisCbType === 'uniform') {
            baseLevels = new Array(codebookSize);
            for (let i = 0; i < codebookSize; i++) baseLevels[i] = -3 + (6 * (i + 0.5)) / codebookSize;
        } else {
            baseLevels = getLloydMaxCentroids(states === 1 ? safeBits : safeBits + 1, trellisCbType);
        }

        const subsetIndices = buildSubsetIndices(baseLevels);
        const scaledBuf = new Float64Array(baseLevels.length);
        const allIdxs = Array.from({ length: baseLevels.length }, (_, k) => k);

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
                const evalGlobalCost = (scale) => {
                    let totalCost = 0;
                    for (let i = 0; i < floats.length; i += safeBlockSize) {
                        const chunk = processFloats.slice(i, i + safeBlockSize);
                        totalCost += evaluateScaleViterbi(chunk, baseLevels, states, scale, subsetIndices, scaledBuf);
                    }
                    return totalCost;
                };

                const resphi = 2 - 1.6180339887;
                let a = globalRms * 0.1;
                let b = globalRms * 2.5;
                let c = a + resphi * (b - a);
                let d = b - resphi * (b - a);
                let fc = evalGlobalCost(c);
                let fd = evalGlobalCost(d);

                for (let iter = 0; iter < trellisOptIters; iter++) {
                    if (fc < fd) {
                        b = d; d = c; fd = fc;
                        c = a + resphi * (b - a);
                        fc = evalGlobalCost(c);
                    } else {
                        a = c; c = d; fc = fd;
                        d = b - resphi * (b - a);
                        fd = evalGlobalCost(d);
                    }
                }
                globalRms = fp16(fc < fd ? c : d);
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
                    const resphi = 2 - 1.6180339887;
                    let a = rms * 0.1;
                    let b = rms * 2.5;
                    let c = a + resphi * (b - a);
                    let d = b - resphi * (b - a);
                    let fc = evaluateScaleViterbi(chunkW, baseLevels, states, c, subsetIndices, scaledBuf);
                    let fd = evaluateScaleViterbi(chunkW, baseLevels, states, d, subsetIndices, scaledBuf);

                    for (let iter = 0; iter < trellisOptIters; iter++) {
                        if (fc < fd) {
                            b = d; d = c; fd = fc;
                            c = a + resphi * (b - a);
                            fc = evaluateScaleViterbi(chunkW, baseLevels, states, c, subsetIndices, scaledBuf);
                        } else {
                            a = c; c = d; fc = fd;
                            d = b - resphi * (b - a);
                            fd = evaluateScaleViterbi(chunkW, baseLevels, states, d, subsetIndices, scaledBuf);
                        }
                    }
                    optScale = fp16(fc < fd ? c : d);
                }
            }

            let chunkQ, pathData;
            if (states === 1) {
                chunkQ = new Float64Array(padLen);
                pathData = new Array(padLen);
                for (let k = 0; k < baseLevels.length; k++) scaledBuf[k] = baseLevels[k] * optScale;
                for (let t = 0; t < padLen; t++) {
                    const match = findClosestIdx(chunkW[t], allIdxs, scaledBuf);
                    chunkQ[t] = match.val;
                    const err = chunkW[t] - match.val;
                    pathData[t] = {
                        state: 0, subset: 0, cbIdx: match.idx, cwVal: match.val,
                        prevState: 0, nextState: t === padLen - 1 ? 'End' : 0,
                        input: chunkW[t], error: err, cost: err * err,
                        stateCosts: [err * err],
                        candidates: [{ prevState: 0, subset: 0, cbIdx: match.idx, cbVal: match.val, dist: Math.abs(err), cost: err * err }]
                    };
                }
            } else {
                const result = quantizeTrellisBlock(chunkW, baseLevels, states, optScale, subsetIndices, allIdxs);
                chunkQ = result.chunkQ;
                pathData = result.pathData;
            }

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