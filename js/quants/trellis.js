import { getErrStats, getLloydMaxCentroids, fwht, getSignFlip } from '../mathUtils.js';

const transitions4 = [
    [ {from: 0, sub: 0}, {from: 1, sub: 2} ], 
    [ {from: 2, sub: 1}, {from: 3, sub: 3} ], 
    [ {from: 0, sub: 2}, {from: 1, sub: 0} ], 
    [ {from: 2, sub: 3}, {from: 3, sub: 1} ]  
];

const transitions8 = [
    [ {from: 0, sub: 0}, {from: 1, sub: 2} ], 
    [ {from: 2, sub: 1}, {from: 3, sub: 3} ], 
    [ {from: 4, sub: 2}, {from: 5, sub: 0} ], 
    [ {from: 6, sub: 3}, {from: 7, sub: 1} ], 
    [ {from: 0, sub: 2}, {from: 1, sub: 0} ], 
    [ {from: 2, sub: 3}, {from: 3, sub: 1} ], 
    [ {from: 4, sub: 0}, {from: 5, sub: 2} ], 
    [ {from: 6, sub: 1}, {from: 7, sub: 3} ]  
];

const transitions16 = [
    [{from: 0, sub: 0}, {from: 1, sub: 2}],  [{from: 2, sub: 1}, {from: 3, sub: 3}], 
    [{from: 4, sub: 2}, {from: 5, sub: 0}],  [{from: 6, sub: 3}, {from: 7, sub: 1}], 
    [{from: 8, sub: 2}, {from: 9, sub: 0}],  [{from: 10, sub: 3}, {from: 11, sub: 1}],
    [{from: 12, sub: 0}, {from: 13, sub: 2}], [{from: 14, sub: 1}, {from: 15, sub: 3}],
    [{from: 0, sub: 2}, {from: 1, sub: 0}],  [{from: 2, sub: 3}, {from: 3, sub: 1}], 
    [{from: 4, sub: 0}, {from: 5, sub: 2}],  [{from: 6, sub: 1}, {from: 7, sub: 3}], 
    [{from: 8, sub: 0}, {from: 9, sub: 2}],  [{from: 10, sub: 1}, {from: 11, sub: 3}],
    [{from: 12, sub: 2}, {from: 13, sub: 0}], [{from: 14, sub: 3}, {from: 15, sub: 1}] 
];

function findClosestInSubset(target, subVals, subIdxs) {
    let low = 0, high = subVals.length - 1;
    if (target <= subVals[low]) return { val: subVals[low], idx: subIdxs[low] };
    if (target >= subVals[high]) return { val: subVals[high], idx: subIdxs[high] };

    while (low <= high) {
        let mid = (low + high) >>> 1;
        if (subVals[mid] < target) low = mid + 1;
        else if (subVals[mid] > target) high = mid - 1;
        else return { val: subVals[mid], idx: subIdxs[mid] };
    }
    
    if (Math.abs(subVals[low] - target) < Math.abs(subVals[high] - target)) {
        return { val: subVals[low], idx: subIdxs[low] };
    }
    return { val: subVals[high], idx: subIdxs[high] };
}

export default {
    id: 'trellis',
    label: "Trellis Quantization (TCQ)",
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
        const bpw = trellisBits + (16 / trellisBlockSize);
        
        const codebookSize = trellisStates === 1 ? Math.pow(2, trellisBits) : Math.pow(2, trellisBits + 1); 

        let baseLevels;
        if (trellisCbType === 'uniform') {
            baseLevels = [];
            for (let i = 0; i < codebookSize; i++) baseLevels.push(-3 + (6 * (i + 0.5)) / codebookSize);
        } else {
            baseLevels = getLloydMaxCentroids(trellisStates === 1 ? trellisBits : trellisBits + 1);
        }

        let transitions = [];
        if (trellisStates === 4) transitions = transitions4;
        else if (trellisStates === 8) transitions = transitions8;
        else if (trellisStates === 16) transitions = transitions16;

        const qFloats = new Float64Array(floats.length);
        const qMathStrings = new Array(floats.length);
        const blockMeta = [];

        for (let i = 0; i < floats.length; i += trellisBlockSize) {
            const chunk = floats.slice(i, i + trellisBlockSize);
            let actualLen = chunk.length;
            
            let padLen = 1; 
            while (padLen < actualLen) padLen *= 2;
            let chunkPadded = [...chunk];
            while (chunkPadded.length < padLen) chunkPadded.push(0);

            if (trellisUseWht) {
                for (let j = 0; j < padLen; j++) chunkPadded[j] *= getSignFlip(j);
            }

            let chunkW = trellisUseWht ? fwht(chunkPadded) : [...chunkPadded];

            let sumSq = 0;
            for (let j = 0; j < padLen; j++) sumSq += chunkW[j] * chunkW[j];
            let rms = Math.sqrt(sumSq / padLen) || 1e-9;

            const scaledLevels = baseLevels.map(c => c * rms);
            
            let chunkQ = [], pathData = [];

            if (trellisStates === 1) {
                for (let t = 0; t < padLen; t++) {
                    const match = findClosestInSubset(chunkW[t], scaledLevels, Array.from({length: codebookSize}, (_,k)=>k));
                    chunkQ.push(match.val);
                    pathData.push({ state: 0, subset: 0, cbIdx: match.idx, cwVal: match.val });
                }
            } else {
                const subsets = [[], [], [], []], subsetIndices = [[], [], [], []];
                for(let k = 0; k < codebookSize; k++) {
                    subsets[k % 4].push(scaledLevels[k]);
                    subsetIndices[k % 4].push(k);
                }

                let costs = new Float64Array(trellisStates).fill(0);
                const dpPaths = [];
                
                for (let t = 0; t < padLen; t++) {
                    const nextCosts = new Float64Array(trellisStates).fill(Infinity), stepMemory = [];
                    for (let s = 0; s < trellisStates; s++) {
                        let bestPrev = -1, bestCbVal = 0, bestCbIdx = -1, bestSub = -1, minCost = Infinity;
                        
                        for (const edge of transitions[s]) {
                            if (costs[edge.from] === Infinity) continue;
                            const match = findClosestInSubset(chunkW[t], subsets[edge.sub], subsetIndices[edge.sub]);
                            const total = costs[edge.from] + Math.pow(chunkW[t] - match.val, 2);
                            
                            if (total < minCost) { 
                                minCost = total; 
                                bestPrev = edge.from; 
                                bestCbVal = match.val; 
                                bestCbIdx = match.idx; 
                                bestSub = edge.sub; 
                            }
                        }
                        nextCosts[s] = minCost;
                        stepMemory.push({ prevState: bestPrev, cbVal: bestCbVal, cbIdx: bestCbIdx, subset: bestSub });
                    }
                    costs = nextCosts; dpPaths.push(stepMemory);
                }
                
                let finalState = 0, minFinal = Infinity;
                for (let s = 0; s < trellisStates; s++) { 
                    if (costs[s] < minFinal) { minFinal = costs[s]; finalState = s; } 
                }
                
                let curr = finalState;
                chunkQ = new Array(padLen); pathData = new Array(padLen);
                for (let t = padLen - 1; t >= 0; t--) {
                    const m = dpPaths[t][curr];
                    chunkQ[t] = m.cbVal; 
                    pathData[t] = { state: curr, subset: m.subset, cbIdx: m.cbIdx, cwVal: m.cbVal, prevState: m.prevState, nextState: pathData[t+1] ? pathData[t+1].state : 'End' };
                    curr = m.prevState;
                }
            }

            let chunkOut = trellisUseWht ? fwht(chunkQ) : chunkQ;
            if (trellisUseWht) {
                for (let j = 0; j < padLen; j++) chunkOut[j] *= getSignFlip(j);
            }

            for (let t = 0; t < actualLen; t++) {
                qFloats[i + t] = chunkOut[t];
                
                let cwStr = pathData[t].cwVal.toFixed(3);
                let subsetStr = trellisStates === 1 ? `Codeword [Idx ${pathData[t].cbIdx}]` : `Subset D${pathData[t].subset} [Idx ${pathData[t].cbIdx}]`;
                qMathStrings[i + t] = trellisUseWht 
                    ? `${subsetStr} = ${cwStr} &rarr; FWHT Mixing` 
                    : `${subsetStr} = ${cwStr}`;
            }
            
            blockMeta.push({ idx: i / trellisBlockSize, size: actualLen, scale: rms, pathData, ...getErrStats(chunk, chunkOut.slice(0, actualLen)) });
        }

        const srhtStr = trellisUseWht ? `<span class="eq-pill" title="Diagonal Random Sign Array">D</span> &times; <span class="eq-pill" title="Orthogonal Fast Walsh-Hadamard Transform">FWHT</span> &times; ` : ``;
        const formulaHTML = `
            <span>Weight = ${srhtStr}[ <span class="eq-pill">TCQ_Codeword<span class="bits">${trellisBits}b</span></span> &times; <span class="eq-pill">RMS_Scale<span class="bits">16b</span></span> ]</span>
            <br><span style="color:var(--text-muted);font-size:0.8rem;">Every ${trellisBlockSize} weights share one RMS scale. Viterbi decoding expands the codebook dynamically.</span>`;

        return { qFloats, qMathStrings, bpw, blockMeta, superMeta: [], formulaHTML };
    },
    buildElements(floats, qFloats, settings, createBar) {
        const frag = document.createDocumentFragment();
        let bIdx = 0;
        for (let i = 0; i < floats.length; i += settings.trellisBlockSize) {
            const chunk = floats.slice(i, i + settings.trellisBlockSize);
            const grp = document.createElement('div');
            grp.className = 'block-group'; grp.style.flex = chunk.length;
            grp.dataset.bIdx = bIdx++;
            chunk.forEach((v, j) => grp.appendChild(createBar(v, qFloats[i + j], i + j)));
            frag.appendChild(grp);
        }
        return frag;
    },
    formatInspector(idx, blockMeta, superMeta, settings) {
        const bIdx = Math.floor(idx / settings.trellisBlockSize);
        const bm = blockMeta[bIdx];
        let blockHtml = '', blockIdxStr = '', superHtml = '', superIdxStr = '';
        if (bm) {
            blockIdxStr = `[${bm.idx}]`;
            blockHtml = `<div class="data-row"><span>MSE:</span> <span class="val-hl">${bm.mse.toFixed(6)}</span></div>
                         <div class="data-row"><span>MAE:</span> <span>${bm.mae.toFixed(6)}</span></div>
                         <div class="data-row" style="margin-top:4px"><span>Scale (RMS):</span> <span>${bm.scale.toFixed(5)}</span></div>`;
            
            const localIdx = idx % settings.trellisBlockSize;
            const p = bm.pathData[localIdx];
            
            if (p) {
                document.getElementById('card-super').querySelector('h4').innerHTML = `Trellis Decoding <span id="ins-sb-idx">[-]</span>`;
                superIdxStr = `[t=${localIdx}]`;

                let stateStorage = settings.trellisStates > 1 ? 
                    `<span class="val-hl">1b</span> (Path) + <span class="val-hl">${Math.max(0, settings.trellisBits - 1)}b</span> (Idx)` : 
                    `<span class="val-hl">${settings.trellisBits}b</span> (Idx)`;

                let stateFlowUI = settings.trellisStates > 1 ? `
                    <div style="display:flex; align-items:center; justify-content:space-between; margin-bottom:12px; padding:6px; background:var(--card-bg); border-radius:6px; border:1px solid var(--border-color);">
                        <div style="text-align:center"><small style="display:block; font-size:9px; opacity:0.6;">FROM</small><b style="color:var(--text-muted)">S${p.prevState}</b></div>
                        <div style="font-size:16px; opacity:0.5;">➔</div>
                        <div style="text-align:center"><small style="display:block; font-size:9px; opacity:0.6;">CURR</small><b style="color:var(--primary-color)">S${p.state}</b></div>
                        <div style="font-size:16px; opacity:0.5;">➔</div>
                        <div style="text-align:center"><small style="display:block; font-size:9px; opacity:0.6;">NEXT</small><b style="color:var(--text-muted)">${p.nextState === 'End' ? 'End' : 'S'+p.nextState}</b></div>
                    </div>` : ``;

                let subsetDots = '';
                if (settings.trellisStates > 1) {
                    subsetDots = `<div class="data-row"><span>Active Subset:</span> <div>` + 
                        [0,1,2,3].map(s => `<span style="padding:2px 6px; border-radius:4px; font-size:10px; border:1px solid var(--border-color); ${p.subset === s ? 'background:var(--primary-color); color:white; border-color:var(--primary-color); font-weight:bold;' : 'opacity:0.4;'}">D${s}</span>`).join(' ') + 
                        `</div></div>`;
                }

                superHtml = `
                    ${stateFlowUI}
                    ${subsetDots}
                    <div class="data-row" style="margin-top:4px;">
                        <span>Data Stored:</span> 
                        <div style="text-align:right; font-size:12px;">${stateStorage}</div>
                    </div>
                `;
            }
        }
        return { blockHtml, blockIdxStr, superHtml, superIdxStr };
    }
};