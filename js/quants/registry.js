import primitiveQuant from './primitive.js';
import symQuant from './sym.js';
import asymQuant from './asym.js';
import kquantQuant from './kquant.js';
import turboQuant from './turbo.js';
import trellisQuant from './trellis.js';
import nvfp4Quant from './nvfp4.js';
import mxfpQuant from './mxfp.js';

primitiveQuant.label = "Primitive (FP Types)";

// Hardware
nvfp4Quant.label = "NVFP4 (Blackwell)";
mxfpQuant.label = "MXFP (OCP Microscaling)";

// Block
symQuant.label = "Symmetric (Block Scale)";
asymQuant.label = "Asymmetric (Block Scale + Zero)";
kquantQuant.label = "K-Quant (Nested Scales)";

// Complex
turboQuant.label = "TurboQuant (Scalar WHT)";
trellisQuant.label = "Trellis (Viterbi WHT)";


const registry = {
    'primitive': primitiveQuant,
    'nvfp4': nvfp4Quant,
    'mxfp': mxfpQuant,
    'sym': symQuant,
    'asym': asymQuant,
    'kquant': kquantQuant,
    'turbo': turboQuant,
    'trellis': trellisQuant,
};

export default registry;