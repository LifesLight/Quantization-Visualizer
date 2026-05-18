export default {
    id: 'turbo',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(false);
    },
    buildElements(floats, qFloats, settings, createBar) {
        const { turboBlockSize } = settings;
        const frag = document.createDocumentFragment();
        let bIdx = 0;
        for (let i = 0; i < floats.length; i += turboBlockSize) {
            const actualLen = Math.min(turboBlockSize, floats.length - i);
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