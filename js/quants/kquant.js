import { getErrStats } from '../mathUtils.js';

export default {
    id: 'kquant',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(true);
        ui.showTurboSettings(false);
        ui.showQuantBits(true);
        ui.showSuperBlockCard(true);
    },
    quantize(floats, settings) {
        const { weightBits, sbSize, subSize, subBits, hasOffset } = settings;
        const basePrecision = 16;
        const bpw = weightBits + ((hasOffset ? 2 : 1) * subBits / subSize) + ((hasOffset ? 2 : 1) * basePrecision / sbSize);

        const qFloats = [...floats];
        const qMathStrings = new Array(floats.length).fill('');
        const blockMeta = [];
        const superMeta = [];

        for (let s = 0; s < floats.length; s += sbSize) {
            const superChunk = floats.slice(s, s + sbSize);
            const subScales = [], subMins = [];
            const subMetaTmp = [];
            const qSubScales = [], qSubMins = [];
            const superChunkQ = [];

            if (hasOffset) {
                const qmaxWeight = Math.pow(2, weightBits) - 1;
                for (let i = 0; i < superChunk.length; i += subSize) {
                    const chunk = superChunk.slice(i, i + subSize);
                    const max = Math.max(...chunk);
                    const min = Math.min(0, Math.min(...chunk));
                    subScales.push((max - min) / qmaxWeight || 1e-9);
                    subMins.push(min);
                }

                const qmaxSub = Math.pow(2, subBits) - 1;
                const superScale = Math.max(...subScales) / qmaxSub || 1e-9;
                const superMinScale = Math.max(...subMins.map(m => -m)) / qmaxSub || 1e-9;

                for (let k = 0; k < subScales.length; k++) {
                    const intScale = Math.max(0, Math.min(qmaxSub, Math.round(subScales[k] / superScale)));
                    const intMin = Math.max(0, Math.min(qmaxSub, Math.round(-subMins[k] / superMinScale)));

                    qSubScales.push(intScale * superScale);
                    qSubMins.push(intMin * superMinScale);
                    subMetaTmp.push({ intScale, intMin, qScale: intScale * superScale, qMin: intMin * superMinScale });
                }
                for (let i = 0; i < superChunk.length; i++) {
                    const subIdx = Math.floor(i / subSize);
                    const q = Math.max(0, Math.min(qmaxWeight, Math.round((superChunk[i] + qSubMins[subIdx]) / (qSubScales[subIdx] || 1e-9))));
                    const qV = q * qSubScales[subIdx] - qSubMins[subIdx];
                    superChunkQ.push(qV);
                    qFloats[s + i] = qV;
                    const sm = subMetaTmp[subIdx];
                    qMathStrings[s + i] = `${q} &times; (${sm.intScale} &times; ${superScale.toFixed(4)}) - (${sm.intMin} &times; ${superMinScale.toFixed(4)})`;
                }
                superMeta.push({ idx: s / sbSize, size: superChunk.length, superScale, superMinScale, ...getErrStats(superChunk, superChunkQ) });
            } else {
                if (weightBits === 1) {
                    const qmaxSub = Math.pow(2, subBits) - 1;
                    for (let i = 0; i < superChunk.length; i += subSize) {
                        const chunk = superChunk.slice(i, i + subSize);
                        const maxAbs = Math.max(...chunk.map(Math.abs));
                        subScales.push(maxAbs || 1e-9);
                    }
                    const superScale = Math.max(...subScales) / qmaxSub || 1e-9;
                    for (let k = 0; k < subScales.length; k++) {
                        const intScale = Math.max(0, Math.min(qmaxSub, Math.round(subScales[k] / superScale)));
                        qSubScales.push(intScale * superScale);
                        subMetaTmp.push({ intScale, qScale: intScale * superScale });
                    }
                    for (let i = 0; i < superChunk.length; i++) {
                        const subIdx = Math.floor(i / subSize);
                        const qV = (superChunk[i] >= 0 ? 1 : -1) * qSubScales[subIdx];
                        superChunkQ.push(qV);
                        qFloats[s + i] = qV;
                        const sm = subMetaTmp[subIdx];
                        qMathStrings[s + i] = `${superChunk[i] >= 0 ? 1 : -1} &times; (${sm.intScale} &times; ${superScale.toFixed(4)})`;
                    }
                    superMeta.push({ idx: s / sbSize, size: superChunk.length, superScale, ...getErrStats(superChunk, superChunkQ) });
                } else {
                    const maxQ = Math.pow(2, weightBits - 1);
                    const qmaxSub = Math.pow(2, subBits) - 1;
                    for (let i = 0; i < superChunk.length; i += subSize) {
                        const chunk = superChunk.slice(i, i + subSize);
                        const maxAbs = Math.max(...chunk.map(Math.abs));
                        subScales.push(maxAbs / (maxQ - 1) || 1e-9);
                    }
                    const superScale = Math.max(...subScales) / qmaxSub || 1e-9;
                    for (let k = 0; k < subScales.length; k++) {
                        const intScale = Math.max(0, Math.min(qmaxSub, Math.round(subScales[k] / superScale)));
                        qSubScales.push(intScale * superScale);
                        subMetaTmp.push({ intScale, qScale: intScale * superScale });
                    }
                    for (let i = 0; i < superChunk.length; i++) {
                        const subIdx = Math.floor(i / subSize);
                        const qs = qSubScales[subIdx] || 1e-9;
                        const q = Math.max(0, Math.min((maxQ * 2) - 1, Math.round(superChunk[i] / qs + maxQ)));
                        const qV = (q - maxQ) * qs;
                        superChunkQ.push(qV);
                        qFloats[s + i] = qV;
                        const sm = subMetaTmp[subIdx];
                        qMathStrings[s + i] = `${q - maxQ} &times; (${sm.intScale} &times; ${superScale.toFixed(4)})`;
                    }
                    superMeta.push({ idx: s / sbSize, size: superChunk.length, superScale, ...getErrStats(superChunk, superChunkQ) });
                }
            }
            for (let i = 0; i < superChunk.length; i += subSize) {
                const chunk = superChunk.slice(i, i + subSize);
                const chunkQ = superChunkQ.slice(i, i + subSize);
                const sm = subMetaTmp[Math.floor(i / subSize)];
                blockMeta.push({ idx: blockMeta.length, sbIdx: s / sbSize, size: chunk.length, ...sm, ...getErrStats(chunk, chunkQ) });
            }
        }

        const shareText = hasOffset
            ? `Every ${subSize} weights share a sub-scale and sub-min, every ${sbSize} a super-scale and super-min-scale.`
            : `Every ${subSize} weights share a sub-scale, every ${sbSize} a super-scale.`;

        const kDesc = `<br><span style="color:var(--text-muted);font-size:0.8rem;">${shareText}</span>`;
        let formulaHTML = '';

        if (hasOffset) {
            formulaHTML = `<span>Weight = <span class="eq-pill">Q_Weight<span class="bits">${weightBits}b</span></span> &times; ( <span class="eq-pill">SubScale_Int<span class="bits">${subBits}b</span></span> &times; <span class="eq-pill">SuperScale<span class="bits">${basePrecision}b</span></span> ) - ( <span class="eq-pill">SubMin_Int<span class="bits">${subBits}b</span></span> &times; <span class="eq-pill">SuperMinScale<span class="bits">${basePrecision}b</span></span> )</span>` + kDesc;
        } else {
            formulaHTML = `<span>Weight = <span class="eq-pill">Q_Weight<span class="bits">${weightBits}b</span></span> &times; ( <span class="eq-pill">SubScale_Int<span class="bits">${subBits}b</span></span> &times; <span class="eq-pill">SuperScale<span class="bits">${basePrecision}b</span></span> )</span>` + kDesc;
        }

        return {
            qFloats,
            qMathStrings,
            bpw,
            blockMeta,
            superMeta,
            formulaHTML
        };
    },
    buildElements(floats, qFloats, settings, createBar) {
        const { sbSize, subSize } = settings;
        const frag = document.createDocumentFragment();
        let bIdx = 0, sbIdx = 0;
        for (let s = 0; s < floats.length; s += sbSize) {
            const superChunk = floats.slice(s, s + sbSize);
            const sGrp = document.createElement('div');
            sGrp.className = 'sb-group';
            sGrp.style.flex = superChunk.length;
            sGrp.dataset.sbIdx = sbIdx++;
            for (let i = 0; i < superChunk.length; i += subSize) {
                const chunk = superChunk.slice(i, i + subSize);
                const bGrp = document.createElement('div');
                bGrp.className = 'block-group';
                bGrp.style.flex = chunk.length;
                bGrp.dataset.bIdx = bIdx++;
                chunk.forEach((v, j) => bGrp.appendChild(createBar(v, qFloats[s + i + j], s + i + j)));
                sGrp.appendChild(bGrp);
            }
            frag.appendChild(sGrp);
        }
        return frag;
    },
    formatInspector(idx, blockMeta, superMeta, settings) {
        const { subSize, sbSize } = settings;
        const globalSubIdx = Math.floor(idx / subSize);
        const bm = blockMeta[globalSubIdx];
        
        let blockHtml = '', blockIdxStr = '';
        if (bm) {
            blockIdxStr = `[${bm.idx}]`;
            blockHtml = `<div class="data-row"><span>MSE:</span> <span class="val-hl">${bm.mse.toFixed(6)}</span></div>
                         <div class="data-row"><span>MAE:</span> <span>${bm.mae.toFixed(6)}</span></div>
                         <div class="data-row" style="margin-top:4px"><span>Int(<span class="val-hl">${bm.intScale}</span>) &times; SuperScale =</span> <span>${bm.qScale.toFixed(5)}</span></div>`;
            if (bm.intMin !== undefined) {
                blockHtml += `<div class="data-row"><span>Int(<span class="val-hl">${bm.intMin}</span>) &times; SMinScale =</span> <span>${bm.qMin.toFixed(5)}</span></div>`;
            }
        }

        const sbIdxMath = Math.floor(idx / sbSize);
        const sm = superMeta[sbIdxMath];
        
        let superHtml = '', superIdxStr = '';
        if (sm) {
            superIdxStr = `[${sm.idx}]`;
            superHtml = `<div class="data-row"><span>Super MSE:</span> <span class="val-hl">${sm.mse.toFixed(6)}</span></div>
                         <div class="data-row"><span>Super MAE:</span> <span>${sm.mae.toFixed(6)}</span></div>
                         <div class="data-row" style="margin-top:4px"><span>SuperScale:</span> <span>${sm.superScale.toFixed(6)}</span></div>`;
            if (sm.superMinScale !== undefined) {
                superHtml += `<div class="data-row"><span>SuperMinScale:</span> <span>${sm.superMinScale.toFixed(6)}</span></div>`;
            }
        }
        return { blockHtml, blockIdxStr, superHtml, superIdxStr };
    }
};