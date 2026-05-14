import { getErrStats } from '../mathUtils.js';

export default {
    id: 'asym',
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
        const bpw = weightBits + (2 * basePrecision / blockSize);

        const qFloats = [...floats];
        const qMathStrings = new Array(floats.length).fill('');
        const blockMeta = [];

        for (let i = 0; i < floats.length; i += blockSize) {
            const chunk = floats.slice(i, i + blockSize);
            const min = Math.min(...chunk);
            const max = Math.max(...chunk);
            
            let scale = (max - min) / (Math.pow(2, weightBits) - 1);
            if (scale === 0) scale = 1e-9;
            const offset = min;
            
            const chunkQ = [];
            for (let j = 0; j < chunk.length; j++) {
                const q = Math.max(0, Math.min(Math.pow(2, weightBits) - 1, Math.round((chunk[j] - offset) / scale)));
                const qVal = q * scale + offset;
                chunkQ.push(qVal);
                qFloats[i + j] = qVal;
                qMathStrings[i + j] = `${q} &times; ${scale.toFixed(4)} + ${offset.toFixed(4)}`;
            }

            blockMeta.push({
                idx: i / blockSize,
                size: chunk.length,
                scale,
                min: offset,
                ...getErrStats(chunk, chunkQ)
            });
        }

        return {
            qFloats,
            qMathStrings,
            bpw,
            blockMeta,
            superMeta: [],
            formulaHTML: `<span>Weight = <span class="eq-pill">Q_Weight<span class="bits">${weightBits}b</span></span> &times; <span class="eq-pill">Scale<span class="bits">${basePrecision}b</span></span> + <span class="eq-pill">Min<span class="bits">${basePrecision}b</span></span></span><br><span style="color:var(--text-muted);font-size:0.8rem;">Every ${blockSize} weights share one global scale and one offset.</span>`
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
                         <div class="data-row" style="margin-top:4px"><span>Scale (FP):</span> <span>${bm.scale.toFixed(5)}</span></div>
                         <div class="data-row"><span>Min (FP):</span> <span>${bm.min.toFixed(5)}</span></div>`;
        }
        return { blockHtml, blockIdxStr, superHtml: '', superIdxStr: '' };
    }
};