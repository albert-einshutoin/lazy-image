# Evaluating performance

Choose a representative workload before comparing engines. lazy-image's product
focus is [verified artifact preparation](./PROJECT_PHILOSOPHY.md); this page does
not establish a universal speed, quality, size or memory advantage.

## What evidence exists

| Evidence | What it establishes | What it does not establish |
|---|---|---|
| [Canonical historical baseline](./TRUE_BENCHMARKS.md) | Recorded codec/resize results on its named machine and versions | Performance of the current package or every input |
| [Compiler and quality runners](./BENCHMARK_OPERATIONS.md#release-compiler-and-constrained-quality-evidence-769) | How to reproduce E2E acceptance, budget and constrained-quality measurements | A successful run merely because the scripts exist |
| [Historical snapshots](./history/BENCHMARK_RESULTS.md) | Earlier operation, memory and cold-start observations | Fresh release evidence |
| [Wasm measurements](./WASM_BENCHMARKING.md) | Browser/Edge preflight methodology | Native compiler performance or full API parity |

The 2026-07-17 baseline used lazy-image 0.16.0 and sharp 0.34.5 on Node.js 24.2.0,
macOS 26.3 / Apple M4. Its two simple PNG-to-JPEG scenarios produced smaller
lazy-image files at the same numeric quality setting; they were not exact
perceptual-quality matches. WebP and AVIF favored sharp in the recorded timing
scenarios. Read the original tables for dimensions, operations and quality context.
No new measurements accompany this documentation cleanup.

## Compare your workload

1. Fix the input corpus, source/license hashes, versions, platform, orientation,
   resize geometry, transparency and metadata requirements.
2. Record actual encoder options. A default-setting comparison is not a comparison
   against every tuned configuration. Equal numeric quality is not equal perception.
3. Compare size at a documented quality constraint, and quality at a byte cap.
   Retain failed and unmet cases rather than counting only successful matches.
4. Separate codec measurements from the complete compiler job, including staging,
   verification and local publication. Record timing variation and memory scope.
5. Assess total compute, temporary storage and delivery cost with your own usage.
   A smaller image or binary alone does not prove lower total cost or cold-start latency.

Use the existing [benchmark commands and artifact procedure](./BENCHMARK_OPERATIONS.md).
Public claims must follow [BENCHMARK_CLAIMS](./BENCHMARK_CLAIMS.md).

## Interpret memory correctly

`fromPathAsync()` and file output avoid full source/output Node Buffers, but Rust
buffers, decoded pixels and codecs still consume memory. Bound concurrency and
measure representative inputs in the actual deployment environment.

`ProcessingMetrics.peakRss` can be the process lifetime high-water mark, not one
image's allocation. The release runner's sampled process RSS has a different scope.
See [metrics boundaries](./metrics-api.md#measurement-boundaries),
[file I/O](./ZERO_COPY.md) and [thread model](./THREAD_MODEL.md).
