export default {
    id: 'template',
    setupUI(ui) {
        // Toggle specific panels for this engine
        ui.showBlockSettings(true);
        ui.showKQuantSettings(false);
        ui.showTurboSettings(false);
        ui.showQuantBits(true);
        ui.showSuperBlockCard(false);
    },
    quantize(floats, settings) {
        // Run quantization mathematics
        return { 
            qFloats: [],               // Same length as input floats
            qMathStrings: [],          // Math equation mapped to hover element
            bpw: 32,                   // Bit-per-weight computation
            blockMeta: [],             // Metadata for tooltip tracking (optional)
            superMeta: [],             // Metadata for superblock tracking (optional)
            formulaHTML: 'Formula'     // General Equation Display
        };
    },
    buildElements(floats, qFloats, settings, createBar) {
        // Create the visualization grouping DOM payload
        // Use createBar(valO, valQ, globalIdx) to instantiate nodes
        const frag = document.createDocumentFragment();
        return frag;
    },
    formatInspector(idx, blockMeta, superMeta, settings) {
        // Given hover index, populate inspector card fields
        return { 
            blockHtml: '', 
            blockIdxStr: '', 
            superHtml: '', 
            superIdxStr: '' 
        };
    }
};