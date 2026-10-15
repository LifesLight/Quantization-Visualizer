export default {
    id: 'iqk',
    importance: 'optional',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        if (ui.showTrellisSettings) ui.showTrellisSettings(false);
        if (ui.showMxfpSettings) ui.showMxfpSettings(false);
        if (ui.showPrimitiveSettings) ui.showPrimitiveSettings(false);
        if (ui.showIqSettings) ui.showIqSettings(false);
        if (ui.showIqkSettings) ui.showIqkSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(true, 'IQ_K Decode');
        ui.showBlockCard(true, 'Sub-Block stats');
    }
};