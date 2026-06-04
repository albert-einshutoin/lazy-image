# Benchmark Operations Guide

This document defines how benchmark regression monitoring is operated.

## Workflow

- Workflow file: `.github/workflows/benchmark-regression.yml`
- Triggers:
  - `workflow_dispatch` (manual)
  - `schedule` (weekly, Monday 03:00 UTC)
- Main execution command set:
  - `npm run test:bench:compare`
  - `npm run test:bench:extended`
- Wasm upload-preflight evidence command:
  - `npm run test:bench:wasm`
- Optional JPEG backend bake-off command:
  - `npm run test:bench:jpegli`
- Optional AVIF backend bake-off command:
  - `npm run test:bench:avif-backends`

## Baseline Source

- Baseline file: `.github/benchmarks/baseline.json`
- Result comparison input: `artifacts/benchmark/sharp-comparison.json`
- Result summary outputs:
  - `artifacts/benchmark/regression-summary.md`
  - `artifacts/benchmark/regression-summary.json`
- Wasm upload report outputs:
  - `artifacts/benchmark/wasm-upload-summary.md`
  - `artifacts/benchmark/wasm-upload-summary.json`
- JPEG backend bake-off outputs:
  - `artifacts/benchmark/jpegli-bakeoff.md`
  - `artifacts/benchmark/jpegli-bakeoff.json`
- AVIF backend bake-off outputs:
  - `artifacts/benchmark/avif-backend-bakeoff.md`
  - `artifacts/benchmark/avif-backend-bakeoff.json`

The baseline file is the single source of truth for threshold comparison.

## Threshold Policy

- Speed regression threshold: `+15%` (default)
- Output-size regression threshold: `+5%` (default)
- You can override both thresholds on manual runs (`workflow_dispatch` inputs).

Regression means "current metric value is higher than baseline by more than threshold".

## Failure Handling Policy

- Scheduled runs: `warn` mode (do not fail workflow on regression).
- Manual runs:
  - default is `warn`
  - can be switched to `fail` mode using `fail_on_regression=true`

## Artifacts and Review Flow

- Always upload `benchmark-regression` artifact from workflow run.
- Review `regression-summary.md` in workflow summary first.
- If regressions are expected (intentional change), update `.github/benchmarks/baseline.json` in a dedicated PR.

## Practical Notes

- Benchmark jobs are intentionally separated from required CI gates to avoid noisy PR failures.
- Run manually before releases or after significant image pipeline/perf changes.
- Run the Wasm upload benchmark before making browser/Edge upload-preflight claims. See [WASM_BENCHMARKING.md](./WASM_BENCHMARKING.md).
- Run the JPEG backend bake-off before considering a mozjpeg-to-jpegli backend switch.
  The harness compares current lazy-image MozJPEG output against `cjpegli` on the same resized PNG intermediate and records exact package/crate versions, settings, command shape, output bytes, encode time, SSIM, and PSNR.
- The JPEG bake-off requires `cjpegli` from jpegli. Set `JPEGLI_CJPEGLI=/path/to/cjpegli` or pass `-- --cjpegli /path/to/cjpegli` when the binary is not on `PATH`.
- Do not substitute JPEG XL's `cjxl` for `cjpegli`; `cjxl` produces JPEG XL output, while this bake-off is for JPEG-compatible jpegli output.
- If `cjpegli --version` does not report an exact tag or commit, set `JPEGLI_VERSION=<tag-or-commit>` or pass `-- --jpegli-version <tag-or-commit>` so the artifact remains reproducible.
- Cross-platform impact for this harness is limited to benchmark operation: lazy-image runtime and package contents are unchanged, and `cjpegli` is an operator-provided CLI. Any future backend switch proposal must attach macOS, Linux, and Windows build/package-size impact because that would introduce a production build dependency instead of an optional benchmark tool.
- `JPEGLI_ALLOW_MISSING=1 npm run test:bench:jpegli` is only for smoke-checking script wiring in environments without jpegli. Do not use skip-mode output as benchmark evidence.
- Run the AVIF backend bake-off before changing the production libavif backend away from the current rav1e feature.
  The harness compares the current lazy-image AVIF path against `avifenc --codec rav1e|aom|svt` on the same resized PNG intermediates and records exact package/crate/tool versions, quality, speed, jobs/thread count, YUV/range settings, output bytes, encode/total wall time, peak RSS when observable, SSIM, PSNR, alpha behavior, and ICC preservation.
- The AVIF bake-off requires libavif tools with all candidate encoder backends enabled. Set `AVIFENC=/path/to/avifenc` or pass `-- --avifenc /path/to/avifenc` when the binary is not on `PATH`.
- `AVIF_BACKEND_ALLOW_MISSING=1 npm run test:bench:avif-backends` is only for smoke-checking script wiring in environments without `avifenc`. `--allow-missing-backends` is only for partial local experiments. Do not use skip-mode or partial-backend output as benchmark evidence.
- Cross-platform impact for this harness is limited to benchmark operation: lazy-image runtime and package contents are unchanged, and `avifenc` is an operator-provided CLI. Any future backend switch proposal must attach macOS, Linux, and Windows build/package-size impact because that would introduce a production build dependency instead of an optional benchmark tool.
