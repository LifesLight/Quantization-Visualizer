## Quantization-Visualizer

### Overview
A visualizer for floating-point quantization. It shows how different schemes map raw weights to low-bit integers and the resulting reconstruction error.

### Supported Quants

*   **Primitive (FP Types):** Standard floating-point formats (FP32, FP16, BF16, FP8) applied element-wise without additional block-level scaling.
*   **NVFP4 (Blackwell):** NVIDIA’s hardware-native 4-bit format using an E2M1 codebook. Employs hierarchical scaling where blocks of 16 share an FP8 scale, and entire tensors share a global FP32 scale.
*   **MXFP (OCP Microscaling):** Open standard for micro-granularity scaling. Blocks of 32 weights share an 8-bit exponent-only (E8M0) scale factor, supporting sub-8-bit element formats (MXFP4, MXFP6, and MXFP8).
*   **Symmetric (Block Scale):** Block-wise quantization where weights share a single scale factor per block.
*   **Asymmetric (Block Scale + Zero):** Block-wise quantization with both a scale and a minimum offset per block.
*   **K-Quant (Nested Scales):** Superblock structure where sub-block scales are themselves quantized by a shared super-scale.
*   **TurboQuant (Scalar WHT):** SRHT rotation normalizes the coefficient distribution, then a precomputed Lloyd-Max codebook quantizes each coordinate independently. Optional QJL adds a 1-bit residual correction to remove inner-product bias.
*   **Trellis (Viterbi WHT):** SRHT rotation followed by a multi-state Viterbi search that minimizes MSE over the full sequence rather than per value independently, using Ungerboeck set partitioning to increase effective codebook resolution.

### Interface

*   **Data Generation:** Generate synthetic weights using Normal, Laplace, Bimodal, or Outlier-heavy distributions.
*   **Inspector:** Hover over bars to see the exact reconstruction math (`Q * Scale + Offset`) and error stats for that specific block.
*   **Stats:** Live calculation of Bits-Per-Weight (BPW), compression ratio, and global Mean Squared Error (MSE).

### License
MIT. (c) Alexander Kurtz 2026.