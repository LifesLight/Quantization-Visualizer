## Quantization-Visualizer

### Overview
A visualizer for floating-point quantization. It shows how different schemes map raw weights to low-bit integers and the resulting reconstruction error.

### Supported Quants

*   **Symmetric (Q_0 style):** Standard block-wise quantization. Weights in a block share a single scale factor.
*   **Asymmetric (Q_1 style):** Block-wise quantization with both a scale and a minimum offset.
*   **K-Quants (Q_K style):** Uses a superblock structure. Sub-blocks have their own small scales/mins which are themselves quantized by a "super" scale/min.
*   **TurboQuant:** High-performance KV-cache optimized quantization. 
    *   **SRHT:** Applies a random sign flip + Fast Walsh-Hadamard Transform to normalize the distribution.
    *   **Lloyd-Max:** Uses an MSE-optimal non-uniform scalar quantizer per coordinate, adapted to the post-rotation distribution.
    *   **QJL:** Adds a 1-bit residual correction step to remove inner-product bias from the MSE quantizer.
*   **Trellis Quantization (TCQ):** Trellis-coded quantization that minimizes error over a sequence rather than independently per value.
    *   **Viterbi Path:** Uses a multi-state trellis (up to 16 states) to calculate the MSE-optimal path of quantized values.
    *   **Set Partitioning:** Splits the codebook into Ungerboeck-style subsets to increase the effective resolution of the quantizer.
    *   **SRHT:** Can apply the Fast Walsh-Hadamard Transform to normalize the weight distribution prior to quantization.

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