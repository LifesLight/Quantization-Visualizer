import noneQuant from './none.js';
import symQuant from './sym.js';
import asymQuant from './asym.js';
import kquantQuant from './kquant.js';
import turboQuant from './turbo.js';
import trellisQuant from './trellis.js';

noneQuant.label = "None (FP32 baseline)";
symQuant.label  = "Symmetric (Block Scale)";
asymQuant.label = "Asymmetric (Block Scale + Zero)";
kquantQuant.label = "K-Quant (Nested Scales)";
turboQuant.label  = "TurboQuant (Scalar WHT)";
trellisQuant.label = "Trellis (Viterbi WHT)";

const registry = {
    'none': noneQuant,
    'sym': symQuant,
    'asym': asymQuant,
    'kquant': kquantQuant,
    'turbo': turboQuant,
    'trellis': trellisQuant
};

export default registry;