document.addEventListener('DOMContentLoaded', () => {
    const toggleLeft = document.getElementById('toggle-left');
    const toggleRight = document.getElementById('toggle-right');
    const wrapperLeft = document.getElementById('panel-left-wrapper');
    const wrapperRight = document.getElementById('panel-right-wrapper');

    toggleLeft.addEventListener('click', () => {
        wrapperLeft.classList.toggle('collapsed');
        toggleLeft.querySelector('svg').style.transform = wrapperLeft.classList.contains('collapsed') ? 'rotate(180deg)' : 'rotate(0deg)';
    });
    toggleRight.addEventListener('click', () => {
        wrapperRight.classList.toggle('collapsed');
        toggleRight.querySelector('svg').style.transform = wrapperRight.classList.contains('collapsed') ? 'rotate(180deg)' : 'rotate(0deg)';
    });

    const inputEl = document.getElementById('float-input');
    const modeEl = document.getElementById('centering-mode');
    const qTypeEl = document.getElementById('quant-type');
    const qBitsEl = document.getElementById('quant-bits');
    const presetEl = document.getElementById('preset-select');
    const chartArea = document.getElementById('chart-area');
    const formulaBox = document.getElementById('equation-box');
    const distEl = document.getElementById('gen-dist');
    const advToggleBtn = document.getElementById('gen-adv-toggle');
    const advPanel = document.getElementById('gen-adv-panel');

    advToggleBtn.addEventListener('click', () => {
        const isHidden = advPanel.style.display === 'none';
        advPanel.style.display = isHidden ? 'flex' : 'none';
        advToggleBtn.classList.toggle('open', isHidden);
    });

    distEl.addEventListener('change', (e) => {
        document.querySelectorAll('.dist-params').forEach(el => el.style.display = 'none');
        const targetParams = document.getElementById(`param-${e.target.value}`);
        if (targetParams) targetParams.style.display = 'flex';
    });

    function updateUI() {
        const qType = qTypeEl.value;
        document.getElementById('block-settings').style.display = (qType === 'sym' || qType === 'asym') ? 'flex' : 'none';
        document.getElementById('kquant-settings').style.display = qType === 'kquant' ? 'flex' : 'none';
        document.getElementById('card-super').style.display = qType === 'kquant' ? 'flex' : 'none';
    }

    const loadPreset = (preset) => {
        switch (preset) {
            case 'Q2_0':
                qTypeEl.value = 'sym'; qBitsEl.value = 2; document.getElementById('block-size').value = '32';
                break;
            case 'Q2_1':
                qTypeEl.value = 'asym'; qBitsEl.value = 2; document.getElementById('block-size').value = '32';
                break;
            case 'Q4_0':
                qTypeEl.value = 'sym'; qBitsEl.value = 4; document.getElementById('block-size').value = '32';
                break;
            case 'Q4_1':
                qTypeEl.value = 'asym'; qBitsEl.value = 4; document.getElementById('block-size').value = '32';
                break;
            case 'Q5_0':
                qTypeEl.value = 'sym'; qBitsEl.value = 5; document.getElementById('block-size').value = '32';
                break;
            case 'Q5_1':
                qTypeEl.value = 'asym'; qBitsEl.value = 5; document.getElementById('block-size').value = '32';
                break;
            case 'Q8_0':
                qTypeEl.value = 'sym'; qBitsEl.value = 8; document.getElementById('block-size').value = '32';
                break;
            case 'Q2_K':
                qTypeEl.value = 'kquant'; qBitsEl.value = 2;
                document.getElementById('block-size').value = '16';
                document.getElementById('superblock-size').value = '256';
                document.getElementById('subblock-size').value = '16';
                document.getElementById('subblock-bits').value = 4;
                document.getElementById('subblock-offset').checked = true;
                break;
            case 'Q3_K':
                qTypeEl.value = 'kquant'; qBitsEl.value = 3;
                document.getElementById('block-size').value = '16';
                document.getElementById('superblock-size').value = '256';
                document.getElementById('subblock-size').value = '16';
                document.getElementById('subblock-bits').value = 6;
                document.getElementById('subblock-offset').checked = false;
                break;
            case 'Q4_K':
                qTypeEl.value = 'kquant'; qBitsEl.value = 4;
                document.getElementById('block-size').value = '32';
                document.getElementById('superblock-size').value = '256';
                document.getElementById('subblock-size').value = '32';
                document.getElementById('subblock-bits').value = 6;
                document.getElementById('subblock-offset').checked = true;
                break;
            case 'Q5_K':
                qTypeEl.value = 'kquant'; qBitsEl.value = 5;
                document.getElementById('block-size').value = '32';
                document.getElementById('superblock-size').value = '256';
                document.getElementById('subblock-size').value = '32';
                document.getElementById('subblock-bits').value = 6;
                document.getElementById('subblock-offset').checked = true;
                break;
            case 'Q6_K':
                qTypeEl.value = 'kquant'; qBitsEl.value = 6;
                document.getElementById('block-size').value = '16';
                document.getElementById('superblock-size').value = '256';
                document.getElementById('subblock-size').value = '16';
                document.getElementById('subblock-bits').value = 8;
                document.getElementById('subblock-offset').checked = false;
                break;
        }
        updateUI();
        render();
    };

    const generateData = () => {
        const dist = document.getElementById('gen-dist').value;
        const count = parseInt(document.getElementById('gen-count').value) || 256;
        const bias = parseFloat(document.getElementById('gen-bias').value) || 0.0;
        let arr = [];

        for (let i = 0; i < count; i++) {
            let val = 0;
            if (dist === 'uniform') {
                const range = parseFloat(document.getElementById('gen-uni-range').value) || 10.0;
                val = (Math.random() * range) - (range / 2);
            } else if (dist === 'laplace') {
                const scale = parseFloat(document.getElementById('gen-scale').value) || 1.0;
                const u = Math.random() - 0.5;
                val = -scale * Math.sign(u) * Math.log(1 - 2 * Math.abs(u));
            } else if (dist === 'bimodal') {
                const pDist = parseFloat(document.getElementById('gen-bimodal-dist').value) || 4.0;
                const pSpread = parseFloat(document.getElementById('gen-bimodal-spread').value) || 1.2;
                const peak = Math.random() > 0.5 ? pDist : -pDist;
                let u = 0, v = 0; while (u === 0) u = Math.random(); while (v === 0) v = Math.random();
                val = peak + Math.sqrt(-2.0 * Math.log(u)) * Math.cos(2.0 * Math.PI * v) * pSpread;
            } else if (dist === 'outliers') {
                const oProb = parseFloat(document.getElementById('gen-outlier-prob').value) || 0.02;
                const oMult = parseFloat(document.getElementById('gen-outlier-mult').value) || 5.0;
                let u = 0, v = 0; while (u === 0) u = Math.random(); while (v === 0) v = Math.random();
                val = Math.sqrt(-2.0 * Math.log(u)) * Math.cos(2.0 * Math.PI * v);
                if (Math.random() < oProb) val *= (Math.random() > 0.5 ? oMult : -oMult);
            } else {
                const std = parseFloat(document.getElementById('gen-std').value) || 1.0;
                let u = 0, v = 0; while (u === 0) u = Math.random(); while (v === 0) v = Math.random();
                val = Math.sqrt(-2.0 * Math.log(u)) * Math.cos(2.0 * Math.PI * v) * std;
            }

            val += bias;
            arr.push(val.toFixed(4));
        }
        inputEl.value = arr.join(', ');
        render();
    };

    const autoUpdateElements = [
        modeEl, qTypeEl, qBitsEl,
        document.getElementById('block-size'), document.getElementById('superblock-size'),
        document.getElementById('subblock-size'), document.getElementById('subblock-bits'),
        document.getElementById('subblock-offset'), document.getElementById('gen-dist'),
        document.getElementById('gen-bias'), document.getElementById('gen-std'),
        document.getElementById('gen-scale'), document.getElementById('gen-bimodal-dist'),
        document.getElementById('gen-bimodal-spread'), document.getElementById('gen-outlier-prob'),
        document.getElementById('gen-outlier-mult'), document.getElementById('gen-uni-range')
    ];

    autoUpdateElements.forEach(el => el.addEventListener('change', () => {
        if (el.id.startsWith('gen-')) generateData();
        else {
            presetEl.value = 'custom';
            updateUI();
            render();
        }
    }));

    presetEl.addEventListener('change', () => {
        if (presetEl.value !== 'custom') loadPreset(presetEl.value);
    });

    document.getElementById('gen-btn').addEventListener('click', generateData);
    inputEl.addEventListener('input', () => render());

    function render() {
        const floats = inputEl.value.split(/[, \n\t]+/).map(s => s.trim()).filter(s => s !== '' && !isNaN(s)).map(Number);
        if (floats.length === 0) return;

        const qType = qTypeEl.value;
        const weightBits = Math.max(1, Math.min(8, parseInt(qBitsEl.value) || 4));
        const basePrecision = 16;

        let qFloats = [...floats];
        let bpw = 32;

        let blockSize = parseInt(document.getElementById('block-size').value) || 32;
        let sbSize = parseInt(document.getElementById('superblock-size').value) || 256;
        let subSize = parseInt(document.getElementById('subblock-size').value) || 32;
        const subBits = parseInt(document.getElementById('subblock-bits').value) || 6;
        const hasOffset = document.getElementById('subblock-offset').checked;

        const blockMeta = [];
        const superMeta = [];

        const getErrStats = (arrO, arrQ) => {
            let se = 0, ae = 0;
            for (let i = 0; i < arrO.length; i++) { se += Math.pow(arrO[i] - arrQ[i], 2); ae += Math.abs(arrO[i] - arrQ[i]); }
            return { mse: se / arrO.length, mae: ae / arrO.length };
        };

        if (qType === 'sym' || qType === 'asym') {
            const isAsym = qType === 'asym';
            bpw = weightBits + ((isAsym ? 2 : 1) * basePrecision / blockSize);

            for (let i = 0; i < floats.length; i += blockSize) {
                const chunk = floats.slice(i, i + blockSize);
                const min = Math.min(...chunk), max = Math.max(...chunk), maxAbs = Math.max(...chunk.map(Math.abs));

                let scale, offset = 0;
                const chunkQ = [];

                if (isAsym) {
                    scale = (max - min) / (Math.pow(2, weightBits) - 1);
                    if (scale === 0) scale = 1e-9;
                    offset = min;
                    for (let j = 0; j < chunk.length; j++) {
                        const q = Math.max(0, Math.min(Math.pow(2, weightBits) - 1, Math.round((chunk[j] - offset) / scale)));
                        const qVal = q * scale + offset;
                        chunkQ.push(qVal);
                        qFloats[i + j] = qVal;
                    }
                } else {
                    if (weightBits === 1) {
                        scale = maxAbs || 1e-9;
                        for (let j = 0; j < chunk.length; j++) {
                            const qVal = (chunk[j] >= 0 ? 1 : -1) * scale;
                            chunkQ.push(qVal);
                            qFloats[i + j] = qVal;
                        }
                    } else {
                        const qmax = Math.pow(2, weightBits - 1) - 1;
                        scale = maxAbs / qmax;
                        if (scale === 0) scale = 1e-9;
                        const qmin = -Math.pow(2, weightBits - 1);
                        for (let j = 0; j < chunk.length; j++) {
                            const q = Math.max(qmin, Math.min(qmax, Math.round(chunk[j] / scale)));
                            const qVal = q * scale;
                            chunkQ.push(qVal);
                            qFloats[i + j] = qVal;
                        }
                    }
                }
                blockMeta.push({ idx: i / blockSize, size: chunk.length, scale, min: offset, ...getErrStats(chunk, chunkQ) });
            }
        } else if (qType === 'kquant') {
            bpw = weightBits + ((hasOffset ? 2 : 1) * subBits / subSize) + ((hasOffset ? 3 : 1) * basePrecision / sbSize);

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
                        const min = Math.min(...chunk), max = Math.max(...chunk);
                        subScales.push((max - min) / qmaxWeight || 1e-9);
                        subMins.push(min);
                    }
                    const qmaxSub = Math.pow(2, subBits) - 1;
                    const superScale = Math.max(...subScales) / qmaxSub || 1e-9;
                    const superMinScale = (Math.max(...subMins) - Math.min(...subMins)) / qmaxSub || 1e-9;
                    const superMinOffset = Math.min(...subMins);
                    for (let k = 0; k < subScales.length; k++) {
                        const intScale = Math.max(0, Math.min(qmaxSub, Math.round(subScales[k] / superScale)));
                        const intMin = Math.max(0, Math.min(qmaxSub, Math.round((subMins[k] - superMinOffset) / superMinScale)));
                        qSubScales.push(intScale * superScale);
                        qSubMins.push(intMin * superMinScale + superMinOffset);
                        subMetaTmp.push({ intScale, intMin, qScale: intScale * superScale, qMin: intMin * superMinScale + superMinOffset });
                    }
                    for (let i = 0; i < superChunk.length; i++) {
                        const subIdx = Math.floor(i / subSize);
                        const q = Math.max(0, Math.min(qmaxWeight, Math.round((superChunk[i] - qSubMins[subIdx]) / qSubScales[subIdx])));
                        const qV = q * qSubScales[subIdx] + qSubMins[subIdx];
                        superChunkQ.push(qV);
                        qFloats[s + i] = qV;
                    }
                    superMeta.push({ idx: s / sbSize, size: superChunk.length, superScale, superMinScale, superMinOffset, ...getErrStats(superChunk, superChunkQ) });
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
                        }
                        superMeta.push({ idx: s / sbSize, size: superChunk.length, superScale, ...getErrStats(superChunk, superChunkQ) });
                    } else {
                        const qmaxWeightSym = Math.pow(2, weightBits - 1) - 1;
                        const qminWeightSym = -Math.pow(2, weightBits - 1);
                        const qmaxSub = Math.pow(2, subBits) - 1;
                        for (let i = 0; i < superChunk.length; i += subSize) {
                            const chunk = superChunk.slice(i, i + subSize);
                            const maxAbs = Math.max(...chunk.map(Math.abs));
                            subScales.push(maxAbs / qmaxWeightSym || 1e-9);
                        }
                        const superScale = Math.max(...subScales) / qmaxSub || 1e-9;
                        for (let k = 0; k < subScales.length; k++) {
                            const intScale = Math.max(0, Math.min(qmaxSub, Math.round(subScales[k] / superScale)));
                            qSubScales.push(intScale * superScale);
                            subMetaTmp.push({ intScale, qScale: intScale * superScale });
                        }
                        for (let i = 0; i < superChunk.length; i++) {
                            const subIdx = Math.floor(i / subSize);
                            const q = Math.max(qminWeightSym, Math.min(qmaxWeightSym, Math.round(superChunk[i] / qSubScales[subIdx])));
                            const qV = q * qSubScales[subIdx];
                            superChunkQ.push(qV);
                            qFloats[s + i] = qV;
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
        }

        const glbErr = getErrStats(floats, qFloats);
        document.getElementById('quant-stats').textContent = `BPW Limit : ${qType === 'none' ? '32.000' : bpw.toFixed(3)} bits\nRatio     : ${qType === 'none' ? '1.00' : (32 / bpw).toFixed(2)}x smaller\nGlobal MSE: ${glbErr.mse.toFixed(6)}`;

        if (qType === 'none') {
            formulaBox.innerHTML = `No Quantization applied.`;
        } else if (qType === 'sym') {
            formulaBox.innerHTML = `<span>Weight = <span class="eq-pill">Q_Weight<span class="bits">${weightBits}b</span></span> &times; <span class="eq-pill">Scale<span class="bits">${basePrecision}b</span></span></span><br><span style="color:var(--text-muted);font-size:0.8rem;">Every ${blockSize} weights share one global scale.</span>`;
        } else if (qType === 'asym') {
            formulaBox.innerHTML = `<span>Weight = <span class="eq-pill">Q_Weight<span class="bits">${weightBits}b</span></span> &times; <span class="eq-pill">Scale<span class="bits">${basePrecision}b</span></span> + <span class="eq-pill">Min<span class="bits">${basePrecision}b</span></span></span><br><span style="color:var(--text-muted);font-size:0.8rem;">Every ${blockSize} weights share one global scale and one offset.</span>`;
        } else if (qType === 'kquant') {
            const kDesc = `<br><span style="color:var(--text-muted);font-size:0.8rem;">Every ${subSize} weights share a sub-scale, and every ${sbSize} weights share a super-scale.</span>`;
            if (hasOffset) {
                formulaBox.innerHTML = `<span>Weight = <span class="eq-pill">Q_Weight<span class="bits">${weightBits}b</span></span> &times; ( <span class="eq-pill">SubScale_Int<span class="bits">${subBits}b</span></span> &times; <span class="eq-pill">SuperScale<span class="bits">${basePrecision}b</span></span> ) + ( <span class="eq-pill">SubMin_Int<span class="bits">${subBits}b</span></span> &times; <span class="eq-pill">SuperMinScale<span class="bits">${basePrecision}b</span></span> + <span class="eq-pill">SuperMin<span class="bits">${basePrecision}b</span></span> )</span>` + kDesc;
            } else {
                formulaBox.innerHTML = `<span>Weight = <span class="eq-pill">Q_Weight<span class="bits">${weightBits}b</span></span> &times; ( <span class="eq-pill">SubScale_Int<span class="bits">${subBits}b</span></span> &times; <span class="eq-pill">SuperScale<span class="bits">${basePrecision}b</span></span> )</span>` + kDesc;
            }
        }

        let dMax = Math.max(...floats);
        let dMin = Math.min(...floats);
        if (dMax === dMin) {
            dMax += 0.1;
            dMin -= 0.1;
        }
        const spread = dMax - dMin;

        const sMax = modeEl.value === 'mid' ? dMax + spread * 0.1 : Math.max(0.1, ...floats.map(Math.abs)) * 1.1;
        const sMin = modeEl.value === 'mid' ? dMin - spread * 0.1 : -sMax;

        document.getElementById('chart-max-lbl').textContent = `Max: ${sMax.toFixed(2)}`;
        document.getElementById('chart-min-lbl').textContent = `Min: ${sMin.toFixed(2)}`;

        chartArea.innerHTML = '<div class="baseline" id="baseline"></div>';

        const baselineY = ((0 - sMin) / (sMax - sMin)) * 100;
        const baselineEl = document.getElementById('baseline');
        if (baselineY >= 0 && baselineY <= 100) {
            baselineEl.style.bottom = `${baselineY}%`;
            baselineEl.style.display = 'block';
        } else {
            baselineEl.style.display = 'none';
        }

        const frag = document.createDocumentFragment();
        let bIdx = 0, sbIdx = 0;

        const clamp = (val) => Math.max(0, Math.min(100, val));

        const createBar = (val, valQ, globalIdx) => {
            const bar = document.createElement('div');
            bar.className = 'bar';
            bar.dataset.idx = globalIdx;

            const yCenter = clamp(((0 - sMin) / (sMax - sMin)) * 100);
            const yVQ = clamp(((valQ - sMin) / (sMax - sMin)) * 100);
            const yV = clamp(((val - sMin) / (sMax - sMin)) * 100);

            const qFill = document.createElement('div');
            qFill.className = 'bar-fill';
            qFill.style.bottom = `${Math.min(yCenter, yVQ)}%`;
            qFill.style.height = `${Math.abs(yVQ - yCenter)}%`;
            qFill.style.backgroundColor = valQ >= 0 ? 'var(--accent-color)' : 'var(--negative-color)';
            bar.appendChild(qFill);

            const errH = Math.abs(yV - yVQ);
            if (errH > 0.05) {
                const errFill = document.createElement('div');
                errFill.className = 'error-fill';
                errFill.style.bottom = `${Math.min(yV, yVQ)}%`;
                errFill.style.height = `${errH}%`;
                bar.appendChild(errFill);
            }
            return bar;
        };

        if (qType === 'none') {
            const grp = document.createElement('div');
            grp.className = 'block-group'; grp.style.flex = floats.length;
            floats.forEach((v, i) => grp.appendChild(createBar(v, qFloats[i], i)));
            frag.appendChild(grp);
        } else if (qType === 'sym' || qType === 'asym') {
            for (let i = 0; i < floats.length; i += blockSize) {
                const chunk = floats.slice(i, i + blockSize);
                const grp = document.createElement('div');
                grp.className = 'block-group'; grp.style.flex = chunk.length;
                grp.dataset.bIdx = bIdx++;
                chunk.forEach((v, j) => grp.appendChild(createBar(v, qFloats[i + j], i + j)));
                frag.appendChild(grp);
            }
        } else if (qType === 'kquant') {
            for (let s = 0; s < floats.length; s += sbSize) {
                const superChunk = floats.slice(s, s + sbSize);
                const sGrp = document.createElement('div');
                sGrp.className = 'sb-group'; sGrp.style.flex = superChunk.length;
                sGrp.dataset.sbIdx = sbIdx++;
                for (let i = 0; i < superChunk.length; i += subSize) {
                    const chunk = superChunk.slice(i, i + subSize);
                    const bGrp = document.createElement('div');
                    bGrp.className = 'block-group'; bGrp.style.flex = chunk.length;
                    bGrp.dataset.bIdx = bIdx++;
                    chunk.forEach((v, j) => bGrp.appendChild(createBar(v, qFloats[s + i + j], s + i + j)));
                    sGrp.appendChild(bGrp);
                }
                frag.appendChild(sGrp);
            }
        }
        chartArea.appendChild(frag);

        const insWData = document.getElementById('ins-w-data'), insBData = document.getElementById('ins-b-data'), insSBData = document.getElementById('ins-sb-data');
        const iWIdx = document.getElementById('ins-w-idx'), iBIdx = document.getElementById('ins-b-idx'), iSBIdx = document.getElementById('ins-sb-idx');

        chartArea.addEventListener('mouseover', (e) => {
            const bar = e.target.closest('.bar');
            if (!bar) return;
            const idx = parseInt(bar.dataset.idx);
            const val = floats[idx], valQ = qFloats[idx];
            iWIdx.textContent = `[${idx}]`;
            insWData.innerHTML = `<div class="data-row"><span>Original:</span> <span class="val-hl">${val.toFixed(5)}</span></div><div class="data-row"><span>Quantized:</span> <span class="val-hl">${valQ.toFixed(5)}</span></div><div class="data-row" style="margin-top:4px"><span>Abs Error:</span> <span>${Math.abs(val - valQ).toFixed(6)}</span></div>`;
            const bGrp = bar.closest('.block-group');
            if (bGrp && bGrp.dataset.bIdx) {
                const bm = blockMeta[bGrp.dataset.bIdx];
                iBIdx.textContent = `[${bm.idx}]`;
                let htm = `<div class="data-row"><span>MSE:</span> <span class="val-hl">${bm.mse.toFixed(6)}</span></div><div class="data-row"><span>MAE:</span> <span>${bm.mae.toFixed(6)}</span></div>`;
                if (qType === 'sym' || qType === 'asym') {
                    htm += `<div class="data-row" style="margin-top:4px"><span>Scale (FP):</span> <span>${bm.scale.toFixed(5)}</span></div>`;
                    if (qType === 'asym') htm += `<div class="data-row"><span>Min (FP):</span> <span>${bm.min.toFixed(5)}</span></div>`;
                } else if (qType === 'kquant') {
                    htm += `<div style="margin-top:6px; color:var(--primary-color);">Sub Scale Math:</div><div class="data-row"><span>Int (<span class="val-hl">${bm.intScale}</span>) &times; SuperScale &nbsp;&nbsp;=&nbsp;</span> <span>${bm.qScale.toFixed(5)}</span></div>`;
                    if (bm.intMin !== undefined) {
                        htm += `<div style="margin-top:6px; color:var(--primary-color);">Sub Min Math:</div><div class="data-row"><span>Int (<span class="val-hl">${bm.intMin}</span>) &times; SMinScale + SMin =&nbsp;</span> <span>${bm.qMin.toFixed(5)}</span></div>`;
                    }
                }
                insBData.innerHTML = htm;
            }
            if (qType === 'kquant') {
                const sbGrp = bar.closest('.sb-group');
                if (sbGrp && sbGrp.dataset.sbIdx) {
                    const sm = superMeta[sbGrp.dataset.sbIdx];
                    iSBIdx.textContent = `[${sm.idx}]`;
                    let htm = `<div class="data-row"><span>Super MSE:</span> <span class="val-hl">${sm.mse.toFixed(6)}</span></div><div class="data-row"><span>Super MAE:</span> <span>${sm.mae.toFixed(6)}</span></div><div class="data-row" style="margin-top:4px"><span>SuperScale:</span> <span>${sm.superScale.toFixed(6)}</span></div>`;
                    if (sm.superMinScale !== undefined) {
                        htm += `<div class="data-row"><span>SuperMinScale:</span> <span>${sm.superMinScale.toFixed(6)}</span></div><div class="data-row"><span>SuperMin Offset:</span> <span>${sm.superMinOffset.toFixed(6)}</span></div>`;
                    }
                    insSBData.innerHTML = htm;
                }
            }
        });
    }

    loadPreset('Q4_0');
    generateData();
});