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
    buildElements(floats, qFloats, settings, createBar) {
        const frag = document.createDocumentFragment();
        const grp = document.createElement('div');
        grp.className = 'block-group';
        grp.style.flex = floats.length;
        for (let i = 0; i < floats.length; i++) grp.appendChild(createBar(floats[i], qFloats[i], i));
        frag.appendChild(grp);
        return frag;
    }
};