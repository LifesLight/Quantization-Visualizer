import { getErrStats, fp8_e4m3, fp32 } from '../mathUtils.js';

export default {
    id: 'nvfp4',
    label: 'Blackwell NVFP4',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(true, 'Global Tensor');
    },
    quantize(floats, settings) {
        const blockSize = 16;
        const weightBits = 4;
        const scaleBits = 8;     // E4M3 scale per block
        const tensorBits = 32;   // FP32 global tensor scale

        // BPW calculation: weights + block scales + one global FP32 scale divided by total count
        const bpw = weightBits + (scaleBits / blockSize) + (tensorBits / floats.length);

        const qFloats = new Array(floats.length);
        const qMathStrings = new Array(floats.length).fill('');
        const blockMeta = [];

        // E2M1 Codebook
        const cb = [0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

        // 1. Calculate the single Global Scale for the entire input
        let maxAbsGlobal = 0;
        for (let i = 0; i < floats.length; i++) {
            const absV = Math.abs(floats[i]);
            if (absV > maxAbsGlobal) maxAbsGlobal = absV;
        }
        // Normalize such that the max value fits the hardware codebook range (6.0)
        let globalScale = fp32(maxAbsGlobal > 0 ? (maxAbsGlobal / 6.0) : 1.0);

        // 2. Process hardware blocks of 16
        for (let i = 0; i < floats.length; i += blockSize) {
            const chunk = floats.slice(i, i + blockSize);
            const actualLen = chunk.length;

            let maxAbsNormalized = 0;
            for (let j = 0; j < actualLen; j++) {
                let v = Math.abs(chunk[j] / globalScale);
                if (v > maxAbsNormalized) maxAbsNormalized = v;
            }

            // Quantize block scale to FP8 E4M3
            let scaleRaw = maxAbsNormalized / 6.0;
            let scaleE4M3 = fp8_e4m3(scaleRaw);
            if (scaleE4M3 === 0) scaleE4M3 = 0.001953; // Min subnormal E4M3

            const chunkQ = [];
            for (let j = 0; j < actualLen; j++) {
                const normVal = (chunk[j] / globalScale) / scaleE4M3;
                const sign = normVal >= 0 ? 1 : -1;
                const absVal = Math.abs(normVal);

                let best = cb[0];
                let bestDist = Math.abs(absVal - best);
                for (let k = 1; k < cb.length; k++) {
                    let dist = Math.abs(absVal - cb[k]);
                    if (dist < bestDist) {
                        bestDist = dist;
                        best = cb[k];
                    }
                }

                const qVal = sign * best * scaleE4M3 * globalScale;
                qFloats[i + j] = qVal;
                chunkQ.push(qVal);

                // Tooltip info
                let E = 0, M = 0;
                if (best === 0.5) { E = 0; M = 1; }
                else if (best === 1.0) { E = 1; M = 0; }
                else if (best === 1.5) { E = 1; M = 1; }
                else if (best === 2.0) { E = 2; M = 0; }
                else if (best === 3.0) { E = 2; M = 1; }
                else if (best === 4.0) { E = 3; M = 0; }
                else if (best === 6.0) { E = 3; M = 1; }

                const sDisp = (best === 0 || sign >= 0) ? '+' : '-';
                qMathStrings[i + j] = `${sDisp}${best} &times; ${scaleE4M3.toFixed(4)} &times; ${globalScale.toFixed(4)} <span style="font-size:0.8rem; color:var(--text-muted);">(E:${E} M:${M})</span>`;
            }

            blockMeta.push({
                idx: i / blockSize,
                size: actualLen,
                scale: scaleE4M3,
                ...getErrStats(chunk, chunkQ)
            });
        }

        const superMeta = [{
            idx: 0,
            size: floats.length,
            globalScale: globalScale,
            ...getErrStats(floats, qFloats)
        }];

        return {
            qFloats,
            qMathStrings,
            bpw,
            blockMeta,
            superMeta,
            formulaHTML: `<span>Weight = <span class="eq-pill">Global_FP32<span class="bits">32b</span></span> &times; ( <span class="eq-pill">NVFP4_Value<span class="bits">4b</span></span> &times; <span class="eq-pill">FP8_Scale<span class="bits">8b</span></span> )</span><br><span style="color:var(--text-muted);font-size:0.8rem;">NVFP4: 16 weights share one FP8 scale. All weights share one global FP32 scale.</span>`
        };
    },
    buildElements(floats, qFloats, settings, createBar) {
        const blockSize = 16;
        const frag = document.createDocumentFragment();

        // Exactly one superblock group for the entire weight set
        const sGrp = document.createElement('div');
        sGrp.className = 'sb-group';
        sGrp.style.flex = floats.length;
        sGrp.dataset.sbIdx = 0;

        for (let i = 0; i < floats.length; i += blockSize) {
            const chunk = floats.slice(i, i + blockSize);
            const bGrp = document.createElement('div');
            bGrp.className = 'block-group';
            bGrp.style.flex = chunk.length;
            bGrp.dataset.bIdx = i / blockSize;
            chunk.forEach((v, j) => bGrp.appendChild(createBar(v, qFloats[i + j], i + j)));
            sGrp.appendChild(bGrp);
        }
        frag.appendChild(sGrp);
        return frag;
    },
    formatInspector(idx, blockMeta, superMeta, settings) {
        const bIdx = Math.floor(idx / 16);
        const bm = blockMeta[bIdx];
        const sm = superMeta[0];

        let blockHtml = bm ? `<div class="data-row"><span>MSE:</span> <span class="val-hl">${bm.mse.toFixed(6)}</span></div>
                              <div class="data-row"><span>MAE:</span> <span>${bm.mae.toFixed(6)}</span></div>
                              <div class="data-row" style="margin-top:4px"><span>Block Scale (FP8):</span> <span>${bm.scale.toFixed(5)}</span></div>` : '';

        let superHtml = sm ? `<div class="data-row"><span>Global MSE:</span> <span class="val-hl">${sm.mse.toFixed(6)}</span></div>
                              <div class="data-row"><span>Global MAE:</span> <span>${sm.mae.toFixed(6)}</span></div>
                              <div class="data-row" style="margin-top:4px"><span>Global Scale (FP32):</span> <span>${sm.globalScale.toFixed(6)}</span></div>` : '';

        return { blockHtml, blockIdxStr: `[${bIdx}]`, superHtml, superIdxStr: `[Global]` };
    }
};