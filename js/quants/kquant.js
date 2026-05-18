export default {
    id: 'kquant',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(true);
        ui.showTurboSettings(false);
        ui.showQuantBits(true);
        ui.showSuperBlockCard(true);
    },
    buildElements(floats, qFloats, settings, createBar) {
        const { sbSize, subSize } = settings;
        const frag = document.createDocumentFragment();
        let bIdx = 0, sbIdx = 0;
        for (let s = 0; s < floats.length; s += sbSize) {
            const superLen = Math.min(sbSize, floats.length - s);
            const sGrp = document.createElement('div');
            sGrp.className = 'sb-group';
            sGrp.style.flex = superLen;
            sGrp.dataset.sbIdx = sbIdx++;
            for (let i = 0; i < superLen; i += subSize) {
                const subLen = Math.min(subSize, superLen - i);
                const bGrp = document.createElement('div');
                bGrp.className = 'block-group';
                bGrp.style.flex = subLen;
                bGrp.dataset.bIdx = bIdx++;
                for (let j = 0; j < subLen; j++) bGrp.appendChild(createBar(floats[s + i + j], qFloats[s + i + j], s + i + j));
                sGrp.appendChild(bGrp);
            }
            frag.appendChild(sGrp);
        }
        return frag;
    }
};