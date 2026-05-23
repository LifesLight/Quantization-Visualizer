export default {
    id: 'iq',
    importance: 'optional',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        if (ui.showTrellisSettings) ui.showTrellisSettings(false);
        if (ui.showMxfpSettings) ui.showMxfpSettings(false);
        if (ui.showPrimitiveSettings) ui.showPrimitiveSettings(false);
        if (ui.showIqSettings) ui.showIqSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(false);
        ui.showBlockCard(true, 'Block Stats');
    }
};