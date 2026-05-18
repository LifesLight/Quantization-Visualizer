export default {
    id: 'trellis',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showTrellisSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(true, 'Trellis Overview');
    },
    buildElements(floats, qFloats, settings, createBar) {
        const frag = document.createDocumentFragment();
        let bIdx = 0;
        for (let i = 0; i < floats.length; i += settings.trellisBlockSize) {
            const actualLen = Math.min(settings.trellisBlockSize, floats.length - i);
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