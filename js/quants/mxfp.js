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
    buildElements(floats, qFloats, settings, createBar) {
        const blockSize = 32;
        const frag = document.createDocumentFragment();
        let bIdx = 0;
        for (let i = 0; i < floats.length; i += blockSize) {
            const actualLen = Math.min(blockSize, floats.length - i);
            const bGrp = document.createElement('div');
            bGrp.className = 'block-group';
            bGrp.style.flex = actualLen;
            bGrp.dataset.bIdx = bIdx++;
            for (let j = 0; j < actualLen; j++) bGrp.appendChild(createBar(floats[i + j], qFloats[i + j], i + j));
            frag.appendChild(bGrp);
        }
        return frag;
    }
};