export default {
    id: 'qtip',
    importance: 'unused',
    setupUI(ui) {
        ui.showBlockSettings(false);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showQtipSettings(true);
        ui.showQuantBits(false);
        ui.showSuperBlockCard(true, 'QTIP Decode');
        ui.showBlockCard(true, 'Tile stats');
    }
};
