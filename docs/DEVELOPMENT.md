# Contributor documentation

Start with [CONTRIBUTING](../CONTRIBUTING.md) for setup and contribution rules.
User-facing installation and workflows live in the [documentation index](./README.md).

## Implementation and contracts

- [Architecture](./ARCHITECTURE.md), [operation contracts](./OPERATIONS.md), [lazy semantics](./LAZY_SEMANTICS.md).
- Specifications: [pipeline](../spec/pipeline.md), [resize](../spec/resize.md), [metadata](../spec/metadata.md), [quality](../spec/quality.md), [limits](../spec/limits.md), [errors](../spec/errors.md).
- [Metrics API and measurement boundaries](./metrics-api.md), [metrics schema](./metrics-schema.json).
- [Encoder parameter mapping](./QUALITY_EFFORT_SPEED_MAPPING.md), [performance guardrails](./PERF_STRATEGY_GUARDRAILS.md).
- [Wasm strategy](./WASM_STRATEGY.md) and [Wasm measurements](./WASM_BENCHMARKING.md).

## Verification and release

- [Benchmark operations](./BENCHMARK_OPERATIONS.md): existing commands, corpus, quality/compiler evidence and artifact retention.
- [Benchmark claim rules](./BENCHMARK_CLAIMS.md): public statements must match recorded versions and scope.
- [Selective testing](./SELECTIVE_TESTING.md), [fuzzing](../FUZZING.md).
- [Release procedure](./RELEASE.md) and [SemVer policy](./SEMVER_POLICY.md).

A runner's existence does not establish a successful run. Review retained results
with their commit, environment, failed/unmet cases and exact measurement scope.

## Historical evidence

These records retain their original measurement dates; they are not current
release benchmarks or product promises:

- [Canonical numeric baseline](./TRUE_BENCHMARKS.md).
- [Benchmark snapshots](./history/BENCHMARK_RESULTS.md).
- [Sharp performance investigation](./history/SHARP_PERF_INVESTIGATION_2026-02-17.md).
- [Binary size measurement](./history/BINARY_SIZE.md).

Superseded implementation plans and duplicate version summaries were removed;
their original text remains in Git history. Current behavior belongs in API/spec,
versioned changes in CHANGELOG, and release checks in the release guide.

## Keep documentation focused

README introduces the product; the index routes readers; adoption explains tasks;
API/spec owns contracts; philosophy owns positioning; roadmap owns priorities.
Link to the owner instead of copying tables, scripts, measurements or release status.
Keep the Japanese README aligned with the English entrypoint. Date external
comparisons and label historical evidence. When removing a page, repair references
in both user documentation and development material.
