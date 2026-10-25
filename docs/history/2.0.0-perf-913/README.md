# 2.0.0 three-warning investigation (#913)

**Publication decision: BLOCK.** The Linux alerts are not reproduced in a controlled same-job comparison, but the prespecified macOS first-complex interval remains inconclusive at the ten-pair limit. Do not call this candidate publication-ready. No product optimization is justified by these results.

## Question and conditions

The independent hosted runs 39988612791 and 40117957006 reported JPEG 73 → 110ms, AVIF 499 → 807ms and complex pipeline 78 → 126ms. The historical version-1 baseline is unchanged. Its CPU, binary, fixture, warmup and toolchain provenance are insufficient for matched comparison. The two later runs have identical output sizes/quality axes and same-job Node 22.23.3 / four-way parallelism, but their comparison process CPU model, loaded binary hash, cache/warmup and contention remain unknown. Sharp also increased (JPEG 102 → 168ms, AVIF 883 → 1553ms, complex 172 → 292ms); that supports a shared environmental effect without identifying its cause. All five prior warnings and past FAIL/unmet reports remain retained unchanged.

Product A is patched a0db8d7f2471c037c10dc2cdd521f2098ea4edae; local B is eafc2dfc48e19ed1af15f1f344583c2ae2d79961. Linux B is instrumentation commit 5b8a65805b80218403f2d81a196f89334bba6eeb, whose entire src tree equals eafc2dfc. This PR changes measurement/docs only. The canonical runner, fixture, Cargo/npm locks, build configuration and codec hot paths match across the original comparison; the deleted preset() method and EncodeOptions conversion are not used by this benchmark. Removing an unused engine field can still affect layout/code generation, so source inspection alone is not a performance verdict.

The preregistered [protocol](protocol.json) fixes five sequential AB/BA pairs, with one conditional five-pair extension only when uncertainty spans 1.15. Each fresh process runs one first-use and three warm canonical suites; the warm point is the process median. First-use excludes module loading, fixture hashing and lossless reference setup; it is not OS-cold I/O or process startup. Both products use the same measured runner overlay, recorded separately from product revision/tree and actual loaded binding hash. Output observations occur after the existing operation timers/quality evaluation. All samples are retained.

## Controlled results

Linux: one [hosted job](https://github.com/albert-einshutoin/lazy-image/actions/runs/40162982932), AMD EPYC 9V45, Ubuntu 24.04 image 20261023.334.3, four logical CPUs, Node 22.23.3, Rust 1.88.0, GCC 13.3.0, separate release/LTO targets, identical locks/fixture and UV_THREADPOOL_SIZE=4. Sharp reports concurrency 1; effective native thread counts are not directly instrumented. Five pairs passed the prespecified criterion, so no extension ran.

| Case | Phase | Paired median B/A | Exploratory 95% interval | Status |
|---|---|---:|---|---|
| JPEG | first | 0.9870 | 0.9494 to 1.0811 | pass |
| JPEG | warm | 1.0137 | 0.9861 to 1.0141 | pass |
| AVIF | first | 1.0058 | 0.9778 to 1.0347 | pass |
| AVIF | warm | 1.0000 | 0.9904 to 1.0135 | pass |
| Complex Pipeline | first | 1.0250 | 0.9878 to 1.0500 | pass |
| Complex Pipeline | warm | 0.9878 | 0.9643 to 1.0247 | pass |

Local: Mac16,10 / macOS 27.0 arm64 / ten CPUs, Node 24.22.1, Rust 1.88.0, Apple Clang 21 and SDK 26.5, distinct release/LTO targets. Five pairs crossed the uncertainty boundary; the one permitted extension ran all ten pairs. The total diagnostic exits 1 / INCONCLUSIVE.

| Case | Phase | Paired median B/A | Exploratory 95% interval | Status |
|---|---|---:|---|---|
| JPEG | first | 0.9786 | 0.9118 to 1.0286 | pass |
| JPEG | warm | 1.0294 | 0.9851 to 1.1023 | pass |
| AVIF | first | 0.9822 | 0.9530 to 1.0243 | pass |
| AVIF | warm | 0.9896 | 0.9771 to 1.0063 | pass |
| Complex Pipeline | first | 1.0376 | 0.9825 to 1.2747 | inconclusive |
| Complex Pipeline | warm | 1.0000 | 0.9870 to 1.0267 | pass |

Every A/B output SHA-256, byte count, format, dimensions, metadata hash and existing source-reference quality axis matched within each environment, including all four suites. No quality, resizing, privacy, budget or security semantics were weakened. Warm evidence cannot replace initial-latency evidence. Small-sample bootstrap intervals are exploratory and do not prove production percentiles or universal platform latency.

## Classification and release consequence

| Warning | Classification | Evidence and remaining limit |
|---|---|---|
| JPEG time | Environment/variation dominant is supported for the historical Linux difference; no reproducible product/build/dependency regression found | Linux first/warm and local first/warm meet +15%; prior sharp time also increased. Original VM cause remains unknown. |
| AVIF time | Environment/variation dominant is supported for the historical Linux difference; no reproducible product/build/dependency regression found | Linux and local first/warm meet +15%; original sharp and lazy times rise together. Compiler/CPU historical details are incomplete. |
| Complex pipeline time | Linux difference supports environment/variation dominance; macOS first-use cause remains unconfirmed | Linux first/warm and local warm meet +15%, but local initial upper ratio is 1.2747. That unresolved result blocks publication readiness. |

The canonical single-shot historical warning is kept separate from the matched performance decision. The original checker/thresholds/baseline and existing JPEG-size/WebP-time warnings are not changed. There is no affirmative, repeatable >15% product regression to patch. The minimal change supplies repeated matched evidence and provenance rather than speculative codec changes. The remaining work before publication approval is a new bounded initial-complex isolation/profiling protocol (including GC/scheduling observations) that explains this variability or obtains sufficient evidence; no additional local pairs or favorable reruns are silently added here.

## Evidence and final candidate binding

- [All 960 raw timed outputs](raw-timings.csv), [paired summaries including controls](controlled-summary.json), [historical observations/source hashes](historical-observations.json), and [hash/provenance index](evidence-index.json).
- The Linux archive retains every process report, canonical metrics/quality result, encoded object and build/environment log. Its ID, archive digest and 90-day expiration are in the index. Local full reports/objects and failed diagnostic remain in the task evidence folder; the checked CSV retains every local timed value and output hash. No binary/cache is committed.
- Original handoffs, five warnings, ENOSPC FAIL, quality/byte targets 37/39, Wasm budget 3/4 and strict expected refusals remain distinct from this investigation.
- Specialist review covers measurement/CI correctness, not the performance conclusion. The final PR and separate handoff must record main CI/Security, the exact merge SHA, eight candidate hashes/native 12 combinations, unchanged-Wasm hash-bound evidence reuse and publication status. Those final checks are separate from this comparison. Do not relabel these measured source SHAs as fresh executions of the later merge SHA.
- No tag creation/push, npm/Cargo publish, GitHub Release, deploy, deprecate, force push or branch deletion is part of this task.
