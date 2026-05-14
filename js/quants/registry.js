import noneQuant from './none.js';
import symQuant from './sym.js';
import asymQuant from './asym.js';
import kquantQuant from './kquant.js';
import turboQuant from './turbo.js';

noneQuant.label = "None (FP32 baseline)";
symQuant.label = "Symmetric (Q4_0 style)";
asymQuant.label = "Asymmetric (Q4_1 style)";
kquantQuant.label = "Superblock (Q4_K style)";
turboQuant.label = "TurboQuant (Polar+SRHT)";

const registry = {
    'none': noneQuant,
    'sym': symQuant,
    'asym': asymQuant,
    'kquant': kquantQuant,
    'turbo': turboQuant
};

export default registry;