export default {
    id: 'nvfp4',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(true, 'Global Tensor');
    },
    buildElements(floats, qFloats, settings, createBar) {
        const blockSize = 16;
        const frag = document.createDocumentFragment();
        const sGrp = document.createElement('div');
        sGrp.className = 'sb-group';
        sGrp.style.flex = floats.length;
        sGrp.dataset.sbIdx = 0;

        for (let i = 0; i < floats.length; i += blockSize) {
            const actualLen = Math.min(blockSize, floats.length - i);
            const bGrp = document.createElement('div');
            bGrp.className = 'block-group';
            bGrp.style.flex = actualLen;
            bGrp.dataset.bIdx = i / blockSize;
            for (let j = 0; j < actualLen; j++) bGrp.appendChild(createBar(floats[i + j], qFloats[i + j], i + j));
            sGrp.appendChild(bGrp);
        }
        frag.appendChild(sGrp);
        return frag;
    }
};