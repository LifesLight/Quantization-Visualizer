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
    }
};