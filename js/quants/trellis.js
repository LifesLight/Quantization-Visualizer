export default {
    id: 'trellis',
    importance: 'optional',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showTrellisSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(true, 'KT Decode');
        ui.showBlockCard(true, 'Sub-Block stats');
    }
};