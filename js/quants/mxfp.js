import { getErrStats, fp8_e4m3, fp8_e5m2, snapToCodebook } from '../mathUtils.js';

// Pre-compute OCP MX Codebooks (Absolute values)
const CB_MXFP4_E2M1 = [0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

const CB_MXFP6_E2M3 = [];
for (let e = 0; e < 4; e++) {
    for (let m = 0; m < 8; m++) {
        if (e === 0) CB_MXFP6_E2M3.push(m / 8);
        else CB_MXFP6_E2M3.push(Math.pow(2, e - 1) * (1 + m / 8));
    }
}

const CB_MXFP6_E3M2 = [];
for (let e = 0; e < 8; e++) {
    for (let m = 0; m < 4; m++) {
        if (e === 0) CB_MXFP6_E3M2.push(Math.pow(2, -2) * (m / 4));
        else CB_MXFP6_E3M2.push(Math.pow(2, e - 3) * (1 + m / 4));
    }
}

export default {
    id: 'mxfp',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        if (ui.showMxfpSettings) ui.showMxfpSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(false);
        ui.showBlockCard(true, 'Micro-Block Stats');
    },
    quantize(floats, settings) {
        const formatStr = settings.mxfpFormat || 'mxfp4_e2m1';
        let max_fmt = 6.0;
        let bits = 4;
        let cb = null;
        let isFp8 = false;
        let fp8Func = null;

        if (formatStr === 'mxfp4_e2m1') { bits = 4; max_fmt = 6.0; cb = CB_MXFP4_E2M1; }
        else if (formatStr === 'mxfp6_e2m3') { bits = 6; max_fmt = 7.5; cb = CB_MXFP6_E2M3; }
        else if (formatStr === 'mxfp6_e3m2') { bits = 6; max_fmt = 28.0; cb = CB_MXFP6_E3M2; }
        else if (formatStr === 'mxfp8_e4m3') { bits = 8; max_fmt = 448.0; isFp8 = true; fp8Func = fp8_e4m3; }
        else if (formatStr === 'mxfp8_e5m2') { bits = 8; max_fmt = 57344.0; isFp8 = true; fp8Func = fp8_e5m2; }

        const blockSize = 32; // OCP Standard for Microscaling
        const bpw = bits + (8 / blockSize);

        const qFloats = new Array(floats.length);
        const qMathStrings = new Array(floats.length).fill('');
        const blockMeta = [];

        for (let i = 0; i < floats.length; i += blockSize) {
            const chunk = floats.slice(i, i + blockSize);
            const actualLen = chunk.length;

            let maxAbs = 0;
            for (let j = 0; j < actualLen; j++) {
                if (Math.abs(chunk[j]) > maxAbs) maxAbs = Math.abs(chunk[j]);
            }

            // Determine shared E8M0 scale (biased purely power-of-two multiplier)
            let E = -127;
            if (maxAbs > 0) {
                E = Math.ceil(Math.log2(maxAbs / max_fmt));
                E = Math.max(-127, Math.min(127, E));
            }
            const S = Math.pow(2, E);

            const chunkQ = [];
            for (let j = 0; j < actualLen; j++) {
                let scaled = chunk[j] / S;
                let qVal = 0;

                if (isFp8) {
                    qVal = fp8Func(scaled);
                } else {
                    qVal = snapToCodebook(scaled, cb);
                }

                const finalVal = qVal * S;
                chunkQ.push(finalVal);
                qFloats[i + j] = finalVal;

                const signStr = qVal < 0 ? '-' : '+';
                const absQ = Math.abs(qVal);
                const sDisp = qVal === 0 ? (chunk[j] < 0 ? '-' : '+') : signStr;

                qMathStrings[i + j] = `${sDisp}${absQ} &times; 2<sup>${E}</sup> <span style="font-size:0.8rem; color:var(--text-muted);">(scale)</span>`;
            }

            blockMeta.push({
                idx: i / blockSize,
                size: actualLen,
                scaleE: E,
                scale: S,
                ...getErrStats(chunk, chunkQ)
            });
        }

        const formatLabel = formatStr.toUpperCase().replace('_', ' ');

        return {
            qFloats,
            qMathStrings,
            bpw,
            blockMeta,
            superMeta: [],
            formulaHTML: `<span>Weight = <span class="eq-pill">Micro_Value<span class="bits">${bits}b</span></span> &times; <span class="eq-pill" title="8-Bit E8M0 scaling factor">2^E<span class="bits">8b</span></span></span><br><span style="color:var(--text-muted);font-size:0.8rem;">OCP ${formatLabel}: Every ${blockSize} weights share one block scale.</span>`
        };
    },
    buildElements(floats, qFloats, settings, createBar) {
        const blockSize = 32;
        const frag = document.createDocumentFragment();
        let bIdx = 0;

        for (let i = 0; i < floats.length; i += blockSize) {
            const chunk = floats.slice(i, i + blockSize);
            const bGrp = document.createElement('div');
            bGrp.className = 'block-group';
            bGrp.style.flex = chunk.length;
            bGrp.dataset.bIdx = bIdx++;
            chunk.forEach((v, j) => bGrp.appendChild(createBar(v, qFloats[i + j], i + j)));
            frag.appendChild(bGrp);
        }

        return frag;
    },
    formatInspector(idx, blockMeta, superMeta, settings) {
        const blockSize = 32;
        const bIdx = Math.floor(idx / blockSize);
        const bm = blockMeta[bIdx];

        let blockHtml = '';
        let blockIdxStr = '';
        if (bm) {
            blockIdxStr = `[${bm.idx}]`;
            blockHtml = `<div class="data-row"><span>MSE:</span> <span class="val-hl">${bm.mse.toFixed(6)}</span></div>
                         <div class="data-row"><span>MAE:</span> <span>${bm.mae.toFixed(6)}</span></div>
                         <div class="data-row" style="margin-top:4px"><span>Block Scale (E8M0):</span> <span>2<sup>${bm.scaleE}</sup> (${bm.scale.toExponential(2)})</span></div>`;
        }

        return { blockHtml, blockIdxStr, superHtml: '', superIdxStr: '' };
    }
};