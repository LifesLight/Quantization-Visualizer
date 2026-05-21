export default {
    id: 'nvfp4',
    importance: 'unused',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(true, 'Global Tensor');
    }
};