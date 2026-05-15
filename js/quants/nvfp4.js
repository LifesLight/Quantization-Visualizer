import { getErrStats, fp8_e4m3, fp32 } from '../mathUtils.js';

export default {
    id: 'nvfp4',
    setupUI(ui) {
        // NVFP4 uses a fixed hardware spec for elements, but we allow simulating different tensor sizes
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        if (ui.showNvfp4Settings) ui.showNvfp4Settings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(true, 'Global Tensor');
    },
    quantize(floats, settings) {
        const blockSize = 16;
        const weightBits = 4;
        const scaleBits = 8;     // E4M3 scale per block
        const tensorBits = 32;   // FP32 global tensor scale

        const tensorSize = settings.nvfp4TensorSize || 256;
        // Exact hardware BPW limit calculation mapping FP32 scale over the size of the chosen tensor
        const bpw = weightBits + (scaleBits / blockSize) + (tensorBits / tensorSize);

        const qFloats = new Array(floats.length);
        const qMathStrings = new Array(floats.length).fill('');
        const blockMeta = [];
        const superMeta = [];

        // E2M1 Codebook mapping perfectly onto 4 bits
        const cb = [0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

        let bIdxCounter = 0;

        for (let s = 0; s < floats.length; s += tensorSize) {
            const tensorChunk = floats.slice(s, s + tensorSize);
            let actualTensorLen = tensorChunk.length;

            // Retrieve max absolute global value for this FP32 tensor segment
            let maxAbsGlobal = 0;
            for (let i = 0; i < actualTensorLen; i++) {
                if (Math.abs(tensorChunk[i]) > maxAbsGlobal) maxAbsGlobal = Math.abs(tensorChunk[i]);
            }

            // Set Global Scale such that the highest block scale needed is perfectly 1.0 (easily fitting E4M3 range)
            let globalScale = fp32(maxAbsGlobal > 0 ? (maxAbsGlobal / 6.0) : 1.0);
            const tensorQ = [];

            // Process hardware blocks within this tensor
            for (let i = 0; i < actualTensorLen; i += blockSize) {
                const chunk = tensorChunk.slice(i, i + blockSize);
                let actualLen = chunk.length;

                let maxAbs = 0;
                for (let j = 0; j < actualLen; j++) {
                    let v = Math.abs(chunk[j] / globalScale);
                    if (v > maxAbs) maxAbs = v;
                }

                // Map the block max to the E2M1 max value (6.0)
                let scaleRaw = maxAbs / 6.0;
                let scaleE4M3 = fp8_e4m3(scaleRaw);
                if (scaleE4M3 === 0) scaleE4M3 = 0.001953;

                const chunkQ = [];
                for (let j = 0; j < actualLen; j++) {
                    const normVal = (chunk[j] / globalScale) / scaleE4M3;
                    const sign = normVal >= 0 ? 1 : -1;
                    const absVal = Math.abs(normVal);

                    // Snap to nearest hardware E2M1 codebook value
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
                    chunkQ.push(qVal);
                    qFloats[s + i + j] = qVal;
                    tensorQ.push(qVal);

                    // Determine underlying binary format details for the visualizer tooltip
                    let E = 0, M = 0;
                    if (best === 0.5) { E = 0; M = 1; }
                    else if (best === 1.0) { E = 1; M = 0; }
                    else if (best === 1.5) { E = 1; M = 1; }
                    else if (best === 2.0) { E = 2; M = 0; }
                    else if (best === 3.0) { E = 2; M = 1; }
                    else if (best === 4.0) { E = 3; M = 0; }
                    else if (best === 6.0) { E = 3; M = 1; }

                    const signStr = sign < 0 ? '-' : '+';
                    const sDisp = best === 0 ? (sign < 0 ? '-' : '+') : signStr;

                    qMathStrings[s + i + j] = `${sDisp}${best} &times; ${scaleE4M3.toFixed(4)} &times; ${globalScale.toFixed(4)} <span style="font-size:0.8rem; color:var(--text-muted);">(E:${E} M:${M})</span>`;
                }

                blockMeta.push({
                    idx: bIdxCounter++,
                    tensorIdx: s / tensorSize,
                    size: actualLen,
                    scale: scaleE4M3,
                    ...getErrStats(chunk, chunkQ)
                });
            }

            superMeta.push({
                idx: s / tensorSize,
                size: actualTensorLen,
                globalScale: globalScale,
                ...getErrStats(tensorChunk, tensorQ)
            });
        }

        return {
            qFloats,
            qMathStrings,
            bpw,
            blockMeta,
            superMeta,
            formulaHTML: `<span>Weight = <span class="eq-pill">Global_FP32<span class="bits">32b</span></span> &times; ( <span class="eq-pill">NVFP4_Value<span class="bits">4b</span></span> &times; <span class="eq-pill" title="8-Bit E4M3 scaling factor">FP8_Scale<span class="bits">8b</span></span> )</span><br><span style="color:var(--text-muted);font-size:0.8rem;">Blackwell NVFP4: Every 16 weights share an FP8 scale. Simulated tensors of size ${tensorSize} share one FP32 scale.</span>`
        };
    },
    buildElements(floats, qFloats, settings, createBar) {
        const blockSize = 16;
        const tensorSize = settings.nvfp4TensorSize || 256;
        const frag = document.createDocumentFragment();

        let bIdx = 0, sbIdx = 0;
        for (let s = 0; s < floats.length; s += tensorSize) {
            const tensorChunk = floats.slice(s, s + tensorSize);
            const sGrp = document.createElement('div');
            sGrp.className = 'sb-group';
            sGrp.style.flex = tensorChunk.length;
            sGrp.dataset.sbIdx = sbIdx++;

            for (let i = 0; i < tensorChunk.length; i += blockSize) {
                const chunk = tensorChunk.slice(i, i + blockSize);
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
        const blockSize = 16;
        const tensorSize = settings.nvfp4TensorSize || 256;

        const bIdx = Math.floor(idx / blockSize);
        const bm = blockMeta[bIdx];

        let blockHtml = '';
        let blockIdxStr = '';
        if (bm) {
            blockIdxStr = `[${bm.idx}]`;
            blockHtml = `<div class="data-row"><span>MSE:</span> <span class="val-hl">${bm.mse.toFixed(6)}</span></div>
                         <div class="data-row"><span>MAE:</span> <span>${bm.mae.toFixed(6)}</span></div>
                         <div class="data-row" style="margin-top:4px"><span>Block Scale (FP8 E4M3):</span> <span>${bm.scale.toFixed(5)}</span></div>`;
        }

        const tIdx = Math.floor(idx / tensorSize);
        const sm = superMeta[tIdx];

        let superHtml = '';
        let superIdxStr = '';
        if (sm) {
            superIdxStr = `[Tensor ${sm.idx}]`;
            superHtml = `<div class="data-row"><span>Tensor MSE:</span> <span class="val-hl">${sm.mse.toFixed(6)}</span></div>
                         <div class="data-row"><span>Tensor MAE:</span> <span>${sm.mae.toFixed(6)}</span></div>
                         <div class="data-row" style="margin-top:4px"><span>Global Scale (FP32):</span> <span>${sm.globalScale.toFixed(6)}</span></div>`;
        }

        return { blockHtml, blockIdxStr, superHtml, superIdxStr };
    }
};