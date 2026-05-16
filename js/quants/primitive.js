import { fp32, fp16, bf16, fp8_e4m3, fp8_e5m2 } from '../mathUtils.js';


export default {
    id: 'primitive',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(false);
        ui.showBlockCard(false);
        if (ui.showPrimitiveSettings) ui.showPrimitiveSettings(true);
    },
    quantize(floats, settings) {
        const format = settings.primitiveFormat || 'fp32';
        let bpw = 32;
        let quantFunc = fp32;
        let formatName = 'FP32';

        if (format === 'fp16') { bpw = 16; quantFunc = fp16; formatName = 'FP16'; }
        else if (format === 'bf16') { bpw = 16; quantFunc = bf16; formatName = 'BF16'; }
        else if (format === 'fp8_e4m3') { bpw = 8; quantFunc = fp8_e4m3; formatName = 'FP8 (E4M3)'; }
        else if (format === 'fp8_e5m2') { bpw = 8; quantFunc = fp8_e5m2; formatName = 'FP8 (E5M2)'; }

        const qFloats = floats.map(v => quantFunc(v));

        return {
            qFloats: qFloats,
            qMathStrings: new Array(floats.length).fill(''),
            bpw: bpw,
            blockMeta: [],
            superMeta: [],
            formulaHTML: `Weights in ${formatName} precision.`
        };
    },
    buildElements(floats, qFloats, settings, createBar) {
        const frag = document.createDocumentFragment();
        const grp = document.createElement('div');
        grp.className = 'block-group';
        grp.style.flex = floats.length;
        floats.forEach((v, i) => grp.appendChild(createBar(v, qFloats[i], i)));
        frag.appendChild(grp);
        return frag;
    },
    formatInspector() {
        return { blockHtml: '', blockIdxStr: '', superHtml: '', superIdxStr: '' };
    }
};