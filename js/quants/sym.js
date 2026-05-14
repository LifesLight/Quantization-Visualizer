import { getErrStats } from '../mathUtils.js';

export default {
    id: 'sym',
    setupUI(ui) {
        ui.showBlockSettings(true);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showQuantBits(true);
        ui.showSuperBlockCard(false);
    },
    quantize(floats, settings) {
        const { weightBits, blockSize } = settings;
        const basePrecision = 16;
        const bpw = weightBits + (basePrecision / blockSize);

        const qFloats = [...floats];
        const qMathStrings = new Array(floats.length).fill('');
        const blockMeta = [];

        for (let i = 0; i < floats.length; i += blockSize) {
            const chunk = floats.slice(i, i + blockSize);
            let scale;
            const chunkQ = [];

            if (weightBits === 1) {
                const maxAbs = Math.max(...chunk.map(Math.abs));
                scale = maxAbs || 1e-9;
                for (let j = 0; j < chunk.length; j++) {
                    const qVal = (chunk[j] >= 0 ? 1 : -1) * scale;
                    chunkQ.push(qVal);
                    qFloats[i + j] = qVal;
                    qMathStrings[i + j] = `${chunk[j] >= 0 ? 1 : -1} &times; ${scale.toFixed(4)}`;
                }
            } else {
                const maxQ = Math.pow(2, weightBits - 1);
                let maxVal = chunk[0];
                for (let j = 1; j < chunk.length; j++) {
                    if (Math.abs(chunk[j]) > Math.abs(maxVal)) maxVal = chunk[j];
                }
                scale = maxVal / -maxQ;
                if (scale === 0) scale = 1e-9;
                for (let j = 0; j < chunk.length; j++) {
                    const q = Math.max(0, Math.min((maxQ * 2) - 1, Math.round(chunk[j] / scale + maxQ)));
                    const qVal = (q - maxQ) * scale;
                    chunkQ.push(qVal);
                    qFloats[i + j] = qVal;
                    qMathStrings[i + j] = `${q - maxQ} &times; ${scale.toFixed(4)}`;
                }
            }
            blockMeta.push({
                idx: i / blockSize,
                size: chunk.length,
                scale,
                ...getErrStats(chunk, chunkQ)
            });
        }

        return {
            qFloats,
            qMathStrings,
            bpw,
            blockMeta,
            superMeta: [],
            formulaHTML: `<span>Weight = <span class="eq-pill">Q_Weight<span class="bits">${weightBits}b</span></span> &times; <span class="eq-pill">Scale<span class="bits">${basePrecision}b</span></span></span><br><span style="color:var(--text-muted);font-size:0.8rem;">Every ${blockSize} weights share one scale.</span>`
        };
    },
    buildElements(floats, qFloats, settings, createBar) {
        const { blockSize } = settings;
        const frag = document.createDocumentFragment();
        let bIdx = 0;
        for (let i = 0; i < floats.length; i += blockSize) {
            const chunk = floats.slice(i, i + blockSize);
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
        const { blockSize } = settings;
        const bIdx = Math.floor(idx / blockSize);
        const bm = blockMeta[bIdx];
        let blockHtml = '';
        let blockIdxStr = '';
        if (bm) {
            blockIdxStr = `[${bm.idx}]`;
            blockHtml = `<div class="data-row"><span>MSE:</span> <span class="val-hl">${bm.mse.toFixed(6)}</span></div>
                         <div class="data-row"><span>MAE:</span> <span>${bm.mae.toFixed(6)}</span></div>
                         <div class="data-row" style="margin-top:4px"><span>Scale (FP):</span> <span>${bm.scale.toFixed(5)}</span></div>`;
        }
        return { blockHtml, blockIdxStr, superHtml: '', superIdxStr: '' };
    }
};