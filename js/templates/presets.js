export const presetGroups = [
    {
        label: "Hardware: NVIDIA",
        presets: {
            'nvfp4': { label: 'Blackwell NVFP4', 'quant-type': 'nvfp4', 'nvfp4-tensor-size': 256 }
        }
    },
    {
        label: "Hardware: OCP (Standard)",
        presets: {
            'mxfp4': { label: 'MXFP4 (E2M1)', 'quant-type': 'mxfp', 'mxfp-format': 'mxfp4_e2m1' },
            'mxfp6': { label: 'MXFP6 (E3M2)', 'quant-type': 'mxfp', 'mxfp-format': 'mxfp6_e2m3' },
        }
    },
    {
        label: "Llama.cpp: Symmetric",
        presets: {
            'Q2_0': { label: 'Q2_0', 'quant-type': 'sym', 'quant-bits': 2, 'block-size': 32 },
            'Q4_0': { label: 'Q4_0', 'quant-type': 'sym', 'quant-bits': 4, 'block-size': 32 },
            'Q8_0': { label: 'Q8_0', 'quant-type': 'sym', 'quant-bits': 8, 'block-size': 32 },
        }
    },
    {
        label: "Llama.cpp: Asymmetric",
        presets: {
            'Q2_1': { label: 'Q2_1', 'quant-type': 'asym', 'quant-bits': 2, 'block-size': 32 },
            'Q4_1': { label: 'Q4_1', 'quant-type': 'asym', 'quant-bits': 4, 'block-size': 32 },
        }
    },
    {
        label: "Llama.cpp: K-Quant",
        presets: {
            'Q2_K': { label: 'Q2_K', 'quant-type': 'kquant', 'quant-bits': 2, 'block-size': 16, 'superblock-size': 256, 'subblock-size': 16, 'subblock-bits': 4, 'subblock-offset': true },
            'Q3_K': { label: 'Q3_K', 'quant-type': 'kquant', 'quant-bits': 3, 'block-size': 16, 'superblock-size': 256, 'subblock-size': 16, 'subblock-bits': 6, 'subblock-offset': false },
            'Q4_K': { label: 'Q4_K', 'quant-type': 'kquant', 'quant-bits': 4, 'block-size': 32, 'superblock-size': 256, 'subblock-size': 32, 'subblock-bits': 6, 'subblock-offset': true },
            'Q5_K': { label: 'Q5_K', 'quant-type': 'kquant', 'quant-bits': 5, 'block-size': 32, 'superblock-size': 256, 'subblock-size': 32, 'subblock-bits': 6, 'subblock-offset': true },
            'Q6_K': { label: 'Q6_K', 'quant-type': 'kquant', 'quant-bits': 6, 'block-size': 16, 'superblock-size': 256, 'subblock-size': 16, 'subblock-bits': 8, 'subblock-offset': false },
        }
    },
    {
        label: "Llama.cpp: IQ Quants",
        presets: {
            'IQ1_S': { label: 'IQ1_S', 'quant-type': 'iq', 'iq-type': 'iq1_s' },
            'IQ1_M': { label: 'IQ1_M', 'quant-type': 'iq', 'iq-type': 'iq1_m' },
            'IQ2_XXS': { label: 'IQ2_XXS', 'quant-type': 'iq', 'iq-type': 'iq2_xxs' },
            'IQ2_XS': { label: 'IQ2_XS', 'quant-type': 'iq', 'iq-type': 'iq2_xs' },
            'IQ2_S': { label: 'IQ2_S', 'quant-type': 'iq', 'iq-type': 'iq2_s' },
            'IQ3_XXS': { label: 'IQ3_XXS', 'quant-type': 'iq', 'iq-type': 'iq3_xxs' },
            'IQ3_S': { label: 'IQ3_S', 'quant-type': 'iq', 'iq-type': 'iq3_s' },
            'IQ4_XS': { label: 'IQ4_XS', 'quant-type': 'iq', 'iq-type': 'iq4_xs' },
            'IQ4_NL': { label: 'IQ4_NL', 'quant-type': 'iq', 'iq-type': 'iq4_nl' },
        }
    },
    {
        label: "Llama.cpp: TurboQuant",
        presets: {
            'turbo2': { label: 'Turbo2', 'quant-type': 'turbo', 'turbo-bits': 2, 'turbo-block-size': 128, 'turbo-wht': true, 'turbo-qjl': false, 'turbo-sign-seed': 42 },
            'turbo3': { label: 'Turbo3', 'quant-type': 'turbo', 'turbo-bits': 3, 'turbo-block-size': 128, 'turbo-wht': true, 'turbo-qjl': false, 'turbo-sign-seed': 42 },
            'turbo4': { label: 'Turbo4', 'quant-type': 'turbo', 'turbo-bits': 4, 'turbo-block-size': 128, 'turbo-wht': true, 'turbo-qjl': false, 'turbo-sign-seed': 42 },
        }
    },
    {
        label: "Paper: TurboQuant",
        presets: {
            'turbo2_qjl': { label: 'Turbo2', 'quant-type': 'turbo', 'turbo-bits': 1, 'turbo-block-size': 32, 'turbo-wht': true, 'turbo-qjl': true, 'turbo-sign-seed': 42 },
            'turbo3_qjl': { label: 'Turbo3', 'quant-type': 'turbo', 'turbo-bits': 2, 'turbo-block-size': 32, 'turbo-wht': true, 'turbo-qjl': true, 'turbo-sign-seed': 42 },
            'turbo4_qjl': { label: 'Turbo4', 'quant-type': 'turbo', 'turbo-bits': 3, 'turbo-block-size': 32, 'turbo-wht': true, 'turbo-qjl': true, 'turbo-sign-seed': 42 },
        }
    },
    {
        label: "ik_llama.cpp: Trellis (KT)",
        presets: {
            'IQ1_KT': { label: 'IQ1_KT', 'quant-type': 'trellis', 'trellis-type': 'iq1_kt' },
            'IQ2_KT': { label: 'IQ2_KT', 'quant-type': 'trellis', 'trellis-type': 'iq2_kt' },
            'IQ3_KT': { label: 'IQ3_KT', 'quant-type': 'trellis', 'trellis-type': 'iq3_kt' },
            'IQ4_KT': { label: 'IQ4_KT', 'quant-type': 'trellis', 'trellis-type': 'iq4_kt' },
        }
    }
];

export const getPresetById = (id) => {
    for (const group of presetGroups) {
        if (group.presets[id]) return group.presets[id];
    }
    return null;
};