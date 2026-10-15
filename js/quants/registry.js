import primitiveQuant from './primitive.js';
import symQuant from './sym.js';
import asymQuant from './asym.js';
import kquantQuant from './kquant.js';
import turboQuant from './turbo.js';
import trellisQuant from './trellis.js';
import nvfp4Quant from './nvfp4.js';
import mxfpQuant from './mxfp.js';
import iqQuant from './iq.js';

primitiveQuant.label = "Primitive (FP Types)";

// Hardware
nvfp4Quant.label = "NVFP4 (Blackwell)";
mxfpQuant.label = "MXFP (OCP Microscaling)";

// Block
symQuant.label = "Symmetric (Block Scale)";
asymQuant.label = "Asymmetric (Block Scale + Zero)";
kquantQuant.label = "K-Quant (Nested Scales)";
iqQuant.label = "IQ Quants (llama.cpp)";
trellisQuant.label = "Trellis (ik_llama.cpp KT)";

// Complex
turboQuant.label = "TurboQuant (Paper)";


const registry = {
    'primitive': primitiveQuant,
    'nvfp4': nvfp4Quant,
    'mxfp': mxfpQuant,
    'sym': symQuant,
    'asym': asymQuant,
    'kquant': kquantQuant,
    'iq': iqQuant,
    'trellis': trellisQuant,
    'turbo': turboQuant,
};

export default registry;