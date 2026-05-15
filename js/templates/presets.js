export const presetGroups = [
    {
        label: "Symmetric (_0)",
        presets: {
            'Q2_0': { label: 'Q2_0', 'quant-type': 'sym', 'quant-bits': 2, 'block-size': 32 },
            'Q4_0': { label: 'Q4_0', 'quant-type': 'sym', 'quant-bits': 4, 'block-size': 32 },
            'Q8_0': { label: 'Q8_0', 'quant-type': 'sym', 'quant-bits': 8, 'block-size': 32 },
        }
    },
    {
        label: "Asymmetric (_1)",
        presets: {
            'Q2_1': { label: 'Q2_1', 'quant-type': 'asym', 'quant-bits': 2, 'block-size': 32 },
            'Q4_1': { label: 'Q4_1', 'quant-type': 'asym', 'quant-bits': 4, 'block-size': 32 },
        }
    },
    {
        label: "K-Quants (_K)",
        presets: {
            'Q2_K': { label: 'Q2_K', 'quant-type': 'kquant', 'quant-bits': 2, 'block-size': 16, 'superblock-size': 256, 'subblock-size': 16, 'subblock-bits': 4, 'subblock-offset': true },
            'Q3_K': { label: 'Q3_K', 'quant-type': 'kquant', 'quant-bits': 3, 'block-size': 16, 'superblock-size': 256, 'subblock-size': 16, 'subblock-bits': 6, 'subblock-offset': false },
            'Q4_K': { label: 'Q4_K', 'quant-type': 'kquant', 'quant-bits': 4, 'block-size': 32, 'superblock-size': 256, 'subblock-size': 32, 'subblock-bits': 6, 'subblock-offset': true },
            'Q5_K': { label: 'Q5_K', 'quant-type': 'kquant', 'quant-bits': 5, 'block-size': 32, 'superblock-size': 256, 'subblock-size': 32, 'subblock-bits': 6, 'subblock-offset': true },
            'Q6_K': { label: 'Q6_K', 'quant-type': 'kquant', 'quant-bits': 6, 'block-size': 16, 'superblock-size': 256, 'subblock-size': 16, 'subblock-bits': 8, 'subblock-offset': false },
        }
    },
    {
        label: "TurboQuant / Llama.cpp",
        presets: {
            'turbo2': { label: 'Turbo2', 'quant-type': 'turbo', 'turbo-bits': 2, 'turbo-block-size': 128, 'turbo-wht': true, 'turbo-qjl': false },
            'turbo3': { label: 'Turbo3', 'quant-type': 'turbo', 'turbo-bits': 3, 'turbo-block-size': 128, 'turbo-wht': true, 'turbo-qjl': false },
            'turbo4': { label: 'Turbo4', 'quant-type': 'turbo', 'turbo-bits': 4, 'turbo-block-size': 128, 'turbo-wht': true, 'turbo-qjl': false },
        }
    },
    {
        label: "TurboQuant / Paper",
        presets: {
            'turbo2_qjl': { label: 'Turbo2', 'quant-type': 'turbo', 'turbo-bits': 1, 'turbo-block-size': 32, 'turbo-wht': true, 'turbo-qjl': true },
            'turbo3_qjl': { label: 'Turbo3', 'quant-type': 'turbo', 'turbo-bits': 2, 'turbo-block-size': 32, 'turbo-wht': true, 'turbo-qjl': true },
            'turbo4_qjl': { label: 'Turbo4', 'quant-type': 'turbo', 'turbo-bits': 3, 'turbo-block-size': 32, 'turbo-wht': true, 'turbo-qjl': true },
        }
    },
    {
        label: "Trellis (TCQ)",
        presets: {
            'tcq2': { label: 'TCQ2', 'quant-type': 'trellis', 'trellis-bits': 2, 'trellis-block-size': 256, 'trellis-states': 4, 'trellis-cb-type': 'lloyd', 'trellis-wht': true },
            'tcq3': { label: 'TCQ3', 'quant-type': 'trellis', 'trellis-bits': 3, 'trellis-block-size': 256, 'trellis-states': 4, 'trellis-cb-type': 'lloyd', 'trellis-wht': true },
            'tcq4': { label: 'TCQ4', 'quant-type': 'trellis', 'trellis-bits': 4, 'trellis-block-size': 256, 'trellis-states': 4, 'trellis-cb-type': 'lloyd', 'trellis-wht': true }
        }
    }
];

export const getPresetById = (id) => {
    for (const group of presetGroups) {
        if (group.presets[id]) return group.presets[id];
    }
    return null;
};