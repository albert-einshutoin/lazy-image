# Evaluating performance

Choose a representative workload before comparing engines. lazy-image's product
focus is [verified artifact preparation](./PROJECT_PHILOSOPHY.md); this page does
not establish a universal speed, quality, size or memory advantage.

## What evidence exists

| Evidence | What it establishes | What it does not establish |
|---|---|---|
| [Canonical historical baseline](./TRUE_BENCHMARKS.md) | Recorded codec/resize results on its named machine and versions | Performance of the current package or every input |
| [Compiler and quality runners](./BENCHMARK_OPERATIONS.md#release-compiler-and-constrained-quality-evidence-769) | How to reproduce E2E acceptance, budget and constrained-quality measurements | A successful run merely because the scripts exist |
| [Native compiler case evaluation](./history/native-compiler-cost-20261011/EVALUATION.md) | Retained 1.4.2 checkout evidence, case-by-case costs/unmet targets, source applicability and one next-work decision | Current published 1.4.3 measurements, user SLA or universal codec superiority |
| [JPEG quality setting-order correction](./history/jpeg-quality-order-20261011/EVALUATION.md) | Output DQT regression and same-machine before/after costs on three JPEG inputs | Published npm measurements, all-input gains or simultaneous quality/byte superiority |
| [Upload/build byte-budget operational evaluation](./history/target-bytes-operational-20261012/EVALUATION.md) | Fixed 18-condition M4 acceptance, E300/E123, outer timing/RSS, and a concrete publication-preparation blocker | Universal SLA/p99, parallel scaling, published npm equivalence, or the correctness of every old-search success |
| [Historical snapshots](./history/BENCHMARK_RESULTS.md) | Earlier operation, memory and cold-start observations | Fresh release evidence |
| [Wasm measurements](./WASM_BENCHMARKING.md) | Browser/Edge preflight methodology | Native compiler performance or full API parity |

The 2026-07-17 baseline used lazy-image 0.16.0 and sharp 0.34.5 on Node.js 24.2.0,
macOS 26.3 / Apple M4. Its two simple PNG-to-JPEG scenarios produced smaller
lazy-image files at the same numeric quality setting; they were not exact
perceptual-quality matches. WebP and AVIF favored sharp in the recorded timing
scenarios. Read the original tables for dimensions, operations and quality context.
The dated operational evaluation measures different byte-budget workloads and
does not refresh this historical engine comparison.

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
