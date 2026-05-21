export default {
    id: 'kquant',
    importance: 'optional',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(true);
        ui.showTurboSettings(false);
        ui.showQuantBits(true);
        ui.showSuperBlockCard(true);
    }
};