export default {
    id: 'sym',
    importance: 'optional',
    setupUI(ui) {
        ui.showBlockSettings(true);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showQuantBits(true);
        ui.showSuperBlockCard(false);
    }
};