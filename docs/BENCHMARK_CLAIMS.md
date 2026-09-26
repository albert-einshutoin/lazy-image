# Public performance claims

Public claims must identify the measured workload, versions and evidence. This
page defines quoting rules; [benchmark operations](./BENCHMARK_OPERATIONS.md)
owns execution commands and artifact retention.

## Sources

- [TRUE_BENCHMARKS](./TRUE_BENCHMARKS.md) is the canonical historical numeric baseline:
  2026-07-17, lazy-image 0.16.0, sharp 0.34.5, Node.js 24.2.0, macOS 26.3 / Apple M4.
- [Historical snapshots](./history/BENCHMARK_RESULTS.md) retain their own scope and dates.
- New compiler/quality results must cite retained evidence for the exact commit
  and environment, including failed and unmet outcomes.

Do not describe historical values as measurements of the current release. A
checked-in runner, changelog entry or version bump is not a successful benchmark.

## Quoting rules

1. Include codec, input dimensions, operations, settings, versions and environment.
   Do not combine resized and non-resized scenarios into an unqualified claim.
2. The historical “17–20% smaller JPEG” summary is limited to the two recorded
   PNG-to-JPEG cases at equal numeric quality. Retain the resized source-reference
   quality context; neither case proves exact perceptual matching or current performance.
3. No general “faster than sharp”, “always smaller”, “no OOM” or automatic cost
   savings claim is established. File-path V8 bypass is not zero total allocation.
4. Report byte-budget and quality targets that could not be met. Expected hostile
   rejection is separate from valid-input acceptance.
5. Distinguish compiler E2E, codec-only, native and Wasm scope. Record RSS measurement
   semantics and trial counts; a process peak is not per-codec allocation.
6. Record tuned encoder settings. Comparing defaults does not establish superiority
   over all alternative configurations or equal perceptual quality.

## Updating a claim

Update evidence first, using the existing commands in the benchmark operations
guide. Review source/license hashes, measurement scope and failures, then update
consumer documentation in the same change. Link summaries to the evidence instead
of duplicating numeric tables across README, migration and adoption guides.

Check [PERFORMANCE](./PERFORMANCE.md), both READMEs and migration guidance for
inconsistent summaries. Benchmark scripts validate measurements; they do not
replace editorial review of the wording and scope of every public claim.
