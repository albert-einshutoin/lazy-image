# Product direction

lazy-image builds verified web-image artifacts inside a Node.js upload or build
pipeline. Its primary workflow turns one local source and an explicit policy into
responsive files, an optional placeholder, and a manifest, verifying the set before
committing it to a local output directory.

## Who it serves

Serve developers of CMS uploads, static sites, and media preprocessing jobs who
want to own execution, policies and output files, and already have a storage and
delivery path. `compileImage()` and `lazy-image compile` are the primary entrypoints
for that job. `ImageEngine` remains the lower-level choice for individual outputs
and existing editing pipelines. See the [adoption guide](./ADOPTION_GUIDE.md).

## Priorities

1. A clear artifact contract: explicit policy, library-owned filenames, verified
   metadata and bytes, a manifest, and failure diagnostics.
2. Resource limits and file-oriented execution with measurable native memory and
   concurrency costs; avoiding a V8 copy is not zero total memory usage.
3. Smaller delivery payloads subject to workload-specific quality and compute
   constraints. Treat byte-budget failure as a result, not a hidden success.
4. Straightforward integration with existing upload/build jobs and storage.
5. A separate, narrower Wasm upload-preflight path where browser/Edge measurements
   justify it; the native compiler is not a V8-isolate API.

## What differentiates it

The useful distinction is packaging public artifact preparation into one contract,
reducing application glue around multiple outputs, verification and publication.
Metadata stripping, resource limits, CLI access and multiple outputs individually
also exist in competing products. This is a focused workflow advantage to validate
with users, not a claim that competitors cannot implement the same outcome.

See the dated [competitor analysis](./COMPETITIVE_ANALYSIS.md) for primary sources,
known overlap, documentation decisions and unproven assumptions.

## Boundaries

- Directory publication is local: the parent must be trusted, without concurrent
  replacement, and staging must share its filesystem. It is not an object-store,
  database or CDN transaction. See the [compiler contract](./API.md#transactional-public-upload-compilation).
- The application owns authentication, upload admission, job scheduling, storage,
  delivery URLs, CDN caching and recovery. lazy-image does not provide a hosted service.
- Keep rich editing, compositing, animation and broad format support with tools
  designed for them. Do not promise sharp API compatibility.
- Do not pursue universal throughput leadership, a CDN/DAM platform, or native/Wasm
  parity as an implicit consequence of adding a codec or helper.
- Keep public API compatibility decisions under [SemVer policy](./SEMVER_POLICY.md).

## Evidence before claims

JPEG size results in the [historical baseline](./TRUE_BENCHMARKS.md) apply to its
recorded versions and two simple scenarios, not every JPEG pipeline or matched
perceptual quality. No blanket WebP/AVIF size or speed win is established.
Rust and resource controls do not prove the absence of codec or operational faults.

Use [benchmark operations](./BENCHMARK_OPERATIONS.md) to record normal-input
acceptance, hostile rejection, strict budgets, quality, timing and RSS with exact
scope. Keep compiler E2E and codec comparisons separate. Quality measurements are
not a runtime perceptual-quality guarantee. The [roadmap](./ROADMAP.md) defines
investment order; new release dates and publication status belong to release evidence.
