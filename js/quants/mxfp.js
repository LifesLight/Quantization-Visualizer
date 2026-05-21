export default {
    id: 'mxfp',
    importance: 'unused',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        if (ui.showMxfpSettings) ui.showMxfpSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(false);
        ui.showBlockCard(true, 'Micro-Block Stats');
    }
};