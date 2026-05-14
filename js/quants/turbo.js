import { getErrStats, getLloydMaxCentroids, fwht, getSignFlip } from '../mathUtils.js';

export default {
    id: 'turbo',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(false);
    },
    quantize(floats, settings) {
        const { turboBits, turboBlockSize, useWht, useQjl } = settings;
        const basePrecision = 16;
        const centroids = getLloydMaxCentroids(turboBits);
        const bpw = turboBits + (useQjl ? 1 : 0) + (basePrecision / turboBlockSize);

        const qFloats = [...floats];
        const qMathStrings = new Array(floats.length).fill('');
        const blockMeta = [];

        for (let i = 0; i < floats.length; i += turboBlockSize) {
            const chunk = floats.slice(i, i + turboBlockSize);
            let actualLen = chunk.length;
            let padLen = 1;
            while (padLen < actualLen) padLen *= 2;

            let chunkPadded = [...chunk];
            while (chunkPadded.length < padLen) chunkPadded.push(0);

            if (useWht) {
                for (let j = 0; j < padLen; j++) chunkPadded[j] *= getSignFlip(j);
            }

            let chunkW = useWht ? fwht(chunkPadded) : [...chunkPadded];

            let sumSq = 0;
            for (let j = 0; j < padLen; j++) sumSq += chunkW[j] * chunkW[j];
            let rms = Math.sqrt(sumSq / padLen) || 1e-9;

            let chunkQ = [];
            let residuals = [];
            let bestCs = [];

            for (let j = 0; j < padLen; j++) {
                let normVal = chunkW[j] / rms;
                let bestC = centroids[0];
                let bestDist = Math.abs(normVal - bestC);
                for (let c = 1; c < centroids.length; c++) {
                    let d = Math.abs(normVal - centroids[c]);
                    if (d < bestDist) {
                        bestDist = d;
                        bestC = centroids[c];
                    }
                }
                let qv = bestC * rms;
                bestCs.push(bestC);
                chunkQ.push(qv);
                residuals.push(chunkW[j] - qv);
            }

            let meanAbsRes = 0;
            let qjlSigns = [];
            if (useQjl) {
                let absResSum = residuals.reduce((s, v) => s + Math.abs(v), 0);
                meanAbsRes = absResSum / padLen;
                for (let j = 0; j < padLen; j++) {
                    let sgn = residuals[j] >= 0 ? 1 : -1;
                    qjlSigns.push(sgn);
                    chunkQ[j] += sgn * meanAbsRes;
                }
            }

            let chunkOut = useWht ? fwht(chunkQ) : chunkQ;

            if (useWht) {
                for (let j = 0; j < padLen; j++) chunkOut[j] *= getSignFlip(j);
            }

            for (let j = 0; j < actualLen; j++) {
                qFloats[i + j] = chunkOut[j];

                let mathStr = `C(${bestCs[j].toFixed(2)}) &times; ${rms.toFixed(2)}`;
                if (useQjl) {
                    mathStr += ` ${qjlSigns[j] > 0 ? '+' : '-'} QJL`;
                }

                if (useWht) {
                    const signStr = getSignFlip(j) > 0 ? '+1' : '-1';
                    qMathStrings[i + j] = `D(${signStr}) &times; FWHT( ${mathStr} )[${j}]`;
                } else {
                    qMathStrings[i + j] = mathStr;
                }
            }

            blockMeta.push({
                idx: i / turboBlockSize,
                size: actualLen,
                scale: rms,
                qjlScale: meanAbsRes,
                ...getErrStats(chunk, chunkOut.slice(0, actualLen))
            });
        }

        let qjlStr = useQjl ? ` + <span class="eq-pill">QJL_1bit<span class="bits">1b</span></span>` : ``;
        let srhtStr = useWht ? `<span class="eq-pill" title="Diagonal Random Sign Array">D</span> &times; <span class="eq-pill" title="Orthogonal Fast Walsh-Hadamard Transform">FWHT</span> &times; ` : ``;

        let footerDesc = `Every ${turboBlockSize} weights share one RMS scale.`;
        if (useWht && useQjl) {
            footerDesc += ` SRHT forces Gaussian distribution; QJL adds 1-bit bias correction.`;
        } else if (useWht) {
            footerDesc += ` SRHT rotates features into a Gaussian distribution.`;
        } else if (useQjl) {
            footerDesc += ` QJL adds 1-bit bias correction to raw values.`;
        } else {
            footerDesc += ` Applying static Lloyd-Max to raw distribution.`;
        }

        return {
            qFloats,
            qMathStrings,
            bpw,
            blockMeta,
            superMeta: [],
            formulaHTML: `<span>Weight = ${srhtStr}[ ( <span class="eq-pill">LloydMax<span class="bits">${turboBits}b</span></span> &times; <span class="eq-pill">RMS_Scale<span class="bits">${basePrecision}b</span></span> )${qjlStr} ]</span><br><span style="color:var(--text-muted);font-size:0.8rem;">${footerDesc}</span>`
        };
    },
    buildElements(floats, qFloats, settings, createBar) {
        const { turboBlockSize } = settings;
        const frag = document.createDocumentFragment();
        let bIdx = 0;
        for (let i = 0; i < floats.length; i += turboBlockSize) {
            const chunk = floats.slice(i, i + turboBlockSize);
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
        const bIdx = Math.floor(idx / settings.turboBlockSize);
        const bm = blockMeta[bIdx];
        let blockHtml = '';
        let blockIdxStr = '';
        if (bm) {
            blockIdxStr = `[${bm.idx}]`;
            blockHtml = `<div class="data-row"><span>MSE:</span> <span class="val-hl">${bm.mse.toFixed(6)}</span></div>
                         <div class="data-row"><span>MAE:</span> <span>${bm.mae.toFixed(6)}</span></div>
                         <div class="data-row" style="margin-top:4px"><span>Scale (FP):</span> <span>${bm.scale.toFixed(5)}</span></div>`;
            if (settings.useQjl && bm.qjlScale > 0) {
                blockHtml += `<div class="data-row"><span>QJL Scale:</span> <span>${bm.qjlScale.toFixed(5)}</span></div>`;
            }
        }
        return { blockHtml, blockIdxStr, superHtml: '', superIdxStr: '' };
    }
};