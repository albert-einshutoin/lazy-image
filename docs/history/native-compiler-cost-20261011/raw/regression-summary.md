# Benchmark Regression Summary

- Mode: **warn**
- Speed threshold: **+15%**
- Size threshold: **+5%**

| Metric | Kind | Baseline | Current | Change | Threshold | Status |
|---|---|---:|---:|---:|---:|---|
| JPEG size (lazy-image) | size | 31518 | 31518 | +0.00% | +5% | ok |
| JPEG time (lazy-image) | speed | 87 | 106 | +21.84% | +15% | regression |
| WebP size (lazy-image) | size | 32448 | 32448 | +0.00% | +5% | ok |
| WebP time (lazy-image) | speed | 176 | 404 | +129.55% | +15% | regression |
| AVIF size (lazy-image) | size | 28678 | 28666 | -0.04% | +5% | ok |
| AVIF time (lazy-image) | speed | 535 | 817 | +52.71% | +15% | regression |
| Complex pipeline size (lazy-image) | size | 27129 | 27129 | +0.00% | +5% | ok |
| Complex pipeline time (lazy-image) | speed | 84 | 126 | +50.00% | +15% | regression |

## Regressions Detected
- JPEG time (lazy-image)
- WebP time (lazy-image)
- AVIF time (lazy-image)
- Complex pipeline time (lazy-image)
