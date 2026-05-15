## Quantization-Visualizer

### Overview
A visualizer for floating-point quantization. It shows how different schemes map raw weights to low-bit integers and the resulting reconstruction error.

### Supported Quants

*   **Symmetric (Block Scale):** Block-wise quantization where weights share a single scale factor per block.
*   **Asymmetric (Block Scale + Zero):** Block-wise quantization with both a scale and a minimum offset per block.
*   **K-Quant (Nested Scales):** Superblock structure where sub-block scales are themselves quantized by a shared super-scale.
*   **TurboQuant (Scalar WHT):** SRHT rotation normalizes the coefficient distribution, then a precomputed Lloyd-Max codebook quantizes each coordinate independently. Optional QJL adds a 1-bit residual correction to remove inner-product bias.
*   **Trellis (Viterbi WHT):** SRHT rotation followed by a multi-state Viterbi search that minimizes MSE over the full sequence rather than per value independently, using Ungerboeck set partitioning to increase effective codebook resolution.

### Interface

*   **Data Generation:** Generate synthetic weights using Normal, Laplace, Bimodal, or Outlier-heavy distributions.
*   **Inspector:** Hover over bars to see the exact reconstruction math (`Q * Scale + Offset`) and error stats for that specific block.
*   **Stats:** Live calculation of Bits-Per-Weight (BPW), compression ratio, and global Mean Squared Error (MSE).

### Running

Because the project uses ES Modules, it requires a local web server to handle imports.

```bash
# Example using Python
python -m http.server 8000
```
Open `localhost:8000` in your browser.

### License
MIT. (c) Alexander Kurtz 2026.