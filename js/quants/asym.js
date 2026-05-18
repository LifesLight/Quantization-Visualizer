export default {
    id: 'asym',
    setupUI(ui) {
        ui.showBlockSettings(true);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showQuantBits(true);
        ui.showSuperBlockCard(false);
    },
    buildElements(floats, qFloats, settings, createBar) {
        const { blockSize } = settings;
        const frag = document.createDocumentFragment();
        let bIdx = 0;
        for (let i = 0; i < floats.length; i += blockSize) {
            const actualLen = Math.min(blockSize, floats.length - i);
            const grp = document.createElement('div');
            grp.className = 'block-group';
            grp.style.flex = actualLen;
            grp.dataset.bIdx = bIdx++;
            for (let j = 0; j < actualLen; j++) grp.appendChild(createBar(floats[i + j], qFloats[i + j], i + j));
            frag.appendChild(grp);
        }
        return frag;
    }
};