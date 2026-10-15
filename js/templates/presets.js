export const presetGroups = [
    {
        label: "Hardware",
        short: 'HW',
        presets: {
            'nvfp4': { label: 'NVFP4 (Blackwell)', bpw: 4.5, 'quant-type': 'nvfp4' },
            'mxfp4': { label: 'MXFP4 (E2M1)', bpw: 4.25, 'quant-type': 'mxfp', 'mxfp-format': 'mxfp4_e2m1' },
            'mxfp6': { label: 'MXFP6 (E2M3)', bpw: 6.25, 'quant-type': 'mxfp', 'mxfp-format': 'mxfp6_e2m3' },
            'mxfp6_e3m2': { label: 'MXFP6 (E3M2)', bpw: 6.25, 'quant-type': 'mxfp', 'mxfp-format': 'mxfp6_e3m2' },
            'fp8_e4m3': { label: 'FP8 (E4M3)', bpw: 8, 'quant-type': 'primitive', 'primitive-format': 'fp8_e4m3' },
            'fp8_e5m2': { label: 'FP8 (E5M2)', bpw: 8, 'quant-type': 'primitive', 'primitive-format': 'fp8_e5m2' },
        }
    },
    {
        label: "llama.cpp: Legacy",
        short: 'Legacy',
        presets: {
            'Q4_0': { label: 'Q4_0', bpw: 4.5, 'quant-type': 'sym', 'quant-bits': 4, 'block-size': 32 },
            'Q5_0': { label: 'Q5_0', bpw: 5.5, 'quant-type': 'sym', 'quant-bits': 5, 'block-size': 32 },
            'Q8_0': { label: 'Q8_0', bpw: 8.5, 'quant-type': 'sym', 'quant-bits': 8, 'block-size': 32 },
            'Q4_1': { label: 'Q4_1', bpw: 5, 'quant-type': 'asym', 'quant-bits': 4, 'block-size': 32 },
            'Q5_1': { label: 'Q5_1', bpw: 6, 'quant-type': 'asym', 'quant-bits': 5, 'block-size': 32 },
        }
    },
    {
        label: "llama.cpp: K-Quants",
        short: 'K',
        presets: {
            'Q2_K': { label: 'Q2_K', bpw: 2.625, 'quant-type': 'kquant', 'quant-bits': 2, 'superblock-size': 256, 'subblock-size': 16, 'subblock-bits': 4, 'subblock-offset': true },
            'Q3_K': { label: 'Q3_K', bpw: 3.4375, 'quant-type': 'kquant', 'quant-bits': 3, 'superblock-size': 256, 'subblock-size': 16, 'subblock-bits': 6, 'subblock-offset': false },
            'Q4_K': { label: 'Q4_K', bpw: 4.5, 'quant-type': 'kquant', 'quant-bits': 4, 'superblock-size': 256, 'subblock-size': 32, 'subblock-bits': 6, 'subblock-offset': true },
            'Q5_K': { label: 'Q5_K', bpw: 5.5, 'quant-type': 'kquant', 'quant-bits': 5, 'superblock-size': 256, 'subblock-size': 32, 'subblock-bits': 6, 'subblock-offset': true },
            'Q6_K': { label: 'Q6_K', bpw: 6.5625, 'quant-type': 'kquant', 'quant-bits': 6, 'superblock-size': 256, 'subblock-size': 16, 'subblock-bits': 8, 'subblock-offset': false },
        }
    },
    {
        label: "llama.cpp: IQ",
        short: 'IQ',
        presets: {
            'IQ1_S': { label: 'IQ1_S', bpw: 1.5625, 'quant-type': 'iq', 'iq-type': 'iq1_s' },
            'IQ1_M': { label: 'IQ1_M', bpw: 1.75, 'quant-type': 'iq', 'iq-type': 'iq1_m' },
            'IQ2_XXS': { label: 'IQ2_XXS', bpw: 2.0625, 'quant-type': 'iq', 'iq-type': 'iq2_xxs' },
            'IQ2_XS': { label: 'IQ2_XS', bpw: 2.3125, 'quant-type': 'iq', 'iq-type': 'iq2_xs' },
            'IQ2_S': { label: 'IQ2_S', bpw: 2.5625, 'quant-type': 'iq', 'iq-type': 'iq2_s' },
            'IQ3_XXS': { label: 'IQ3_XXS', bpw: 3.0625, 'quant-type': 'iq', 'iq-type': 'iq3_xxs' },
            'IQ3_S': { label: 'IQ3_S', bpw: 3.4375, 'quant-type': 'iq', 'iq-type': 'iq3_s' },
            'IQ4_XS': { label: 'IQ4_XS', bpw: 4.25, 'quant-type': 'iq', 'iq-type': 'iq4_xs' },
            'IQ4_NL': { label: 'IQ4_NL', bpw: 4.5, 'quant-type': 'iq', 'iq-type': 'iq4_nl' },
        }
    },
    {
        label: "ik_llama.cpp: IQ_K",
        short: 'IQ_K',
        presets: {
            'IQ2_KS': { label: 'IQ2_KS', bpw: 2.1875, 'quant-type': 'iqk', 'iqk-type': 'iq2_ks' },
            'IQ2_K': { label: 'IQ2_K', bpw: 2.375, 'quant-type': 'iqk', 'iqk-type': 'iq2_k' },
            'IQ2_KL': { label: 'IQ2_KL', bpw: 2.6875, 'quant-type': 'iqk', 'iqk-type': 'iq2_kl' },
            'IQ3_KS': { label: 'IQ3_KS', bpw: 3.1875, 'quant-type': 'iqk', 'iqk-type': 'iq3_ks' },
            'IQ3_K': { label: 'IQ3_K', bpw: 3.4375, 'quant-type': 'iqk', 'iqk-type': 'iq3_k' },
            'IQ4_KSS': { label: 'IQ4_KSS', bpw: 4, 'quant-type': 'iqk', 'iqk-type': 'iq4_kss' },
            'IQ4_KS': { label: 'IQ4_KS', bpw: 4.25, 'quant-type': 'iqk', 'iqk-type': 'iq4_ks' },
            'IQ4_K': { label: 'IQ4_K', bpw: 4.5, 'quant-type': 'iqk', 'iqk-type': 'iq4_k' },
            'IQ5_KS': { label: 'IQ5_KS', bpw: 5.25, 'quant-type': 'iqk', 'iqk-type': 'iq5_ks' },
            'IQ5_K': { label: 'IQ5_K', bpw: 5.5, 'quant-type': 'iqk', 'iqk-type': 'iq5_k' },
            'IQ6_K': { label: 'IQ6_K', bpw: 6.625, 'quant-type': 'iqk', 'iqk-type': 'iq6_k' },
        }
    },
    {
        label: "ik_llama.cpp: Trellis (KT)",
        short: 'KT',
        presets: {
            'IQ1_KT': { label: 'IQ1_KT', bpw: 1.75, 'quant-type': 'trellis', 'trellis-type': 'iq1_kt' },
            'IQ2_KT': { label: 'IQ2_KT', bpw: 2.125, 'quant-type': 'trellis', 'trellis-type': 'iq2_kt' },
            'IQ3_KT': { label: 'IQ3_KT', bpw: 3.125, 'quant-type': 'trellis', 'trellis-type': 'iq3_kt' },
            'IQ4_KT': { label: 'IQ4_KT', bpw: 4, 'quant-type': 'trellis', 'trellis-type': 'iq4_kt' },
        }
    },
    {
        label: "Paper: TurboQuant",
        short: 'Turbo',
        presets: {
            'turbo2': { label: 'Turbo2', bpw: 2.125, 'quant-type': 'turbo', 'turbo-bits': 2, 'turbo-block-size': 128, 'turbo-qjl': false, 'turbo-seed': 42, 'rotation-type': 'orthogonal', 'rotation-seed': 42 },
            'turbo3': { label: 'Turbo3', bpw: 3.125, 'quant-type': 'turbo', 'turbo-bits': 3, 'turbo-block-size': 128, 'turbo-qjl': false, 'turbo-seed': 42, 'rotation-type': 'orthogonal', 'rotation-seed': 42 },
            'turbo4': { label: 'Turbo4', bpw: 4.125, 'quant-type': 'turbo', 'turbo-bits': 4, 'turbo-block-size': 128, 'turbo-qjl': false, 'turbo-seed': 42, 'rotation-type': 'orthogonal', 'rotation-seed': 42 },
        }
    },
    {
        label: "Paper: QTIP",
        short: 'QTIP',
        presets: {
            'qtip2': { label: 'QTIP2', bpw: 2, 'quant-type': 'qtip', 'qtip-code': 'hyb', 'qtip-bits': 2, 'qtip-l': 16, 'qtip-scale': 0.9, 'rotation-type': 'rht', 'rotation-size': 4096, 'rotation-seed': 42 },
            'qtip3': { label: 'QTIP3', bpw: 3, 'quant-type': 'qtip', 'qtip-code': 'hyb', 'qtip-bits': 3, 'qtip-l': 16, 'qtip-scale': 0.9, 'rotation-type': 'rht', 'rotation-size': 4096, 'rotation-seed': 42 },
            'qtip4': { label: 'QTIP4', bpw: 4, 'quant-type': 'qtip', 'qtip-code': 'hyb', 'qtip-bits': 4, 'qtip-l': 16, 'qtip-scale': 0.9, 'rotation-type': 'rht', 'rotation-size': 4096, 'rotation-seed': 42 },
        }
    }
];

export const getPresetById = (id) => {
    for (const group of presetGroups) {
        if (group.presets[id]) return group.presets[id];
    }
    return null;
};