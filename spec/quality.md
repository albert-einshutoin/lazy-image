# Quality Semantics (v0.9.x)

## Bench/CI gates
- **SSIM ≥ 0.995** and **PSNR ≥ 40 dB** compared to sharp outputs under identical settings.
- Enforced by `test/benchmarks/sharp-comparison.bench.js` (run via `npm run test:bench:compare`) and smoke-tested in `test/integration/quality-metrics.test.js`.
- Bench fixtures: 4.5MB 5000×5000 PNG → JPEG/WebP/AVIF with typical qualities (JPEG 80, WebP 80, AVIF 60) and resize 800×600 cases.

## Interpretation
- Thresholds are stability guards: regressions below either threshold fail CI.
- Metrics are currently internal; no public API returns SSIM/PSNR. Consumers wanting exposed metrics should propose an API.

## Color accuracy
- ICC profiles preserved by default for JPEG/PNG/WebP; AVIF preserves ICC on libavif builds (v0.9.x). Conversions rely on preserved ICC to maintain SSIM/PSNR targets.

## Repro guidance
- Run `npm test` to execute integration + rust tests.
- For detailed measurements, use the benchmark repo `lazy-image-test` or `npm run test:bench:compare` in this repo.
