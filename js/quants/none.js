export default {
    id: 'none',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showQuantBits(true);
        ui.showSuperBlockCard(false);
    },
    quantize(floats) {
        return {
            qFloats: [...floats],
            qMathStrings: new Array(floats.length).fill(''),
            bpw: 32,
            blockMeta: [],
            superMeta: [],
            formulaHTML: `No Quantization applied.`
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