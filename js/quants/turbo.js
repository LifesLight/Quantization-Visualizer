export default {
    id: 'turbo',
    importance: 'unused',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(false);
    }
};