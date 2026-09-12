# Documentation

lazy-image prepares web images in your own Node.js upload or build pipeline.
Choose a workflow, produce the first output, then check the relevant limits.

## Get started

| Goal | Guide |
|---|---|
| Install and convert one image | [Quick start](../README.md#quick-start) · [日本語](../README.ja.md) |
| Generate a verified image set and manifest | [Adoption guide](./ADOPTION_GUIDE.md) |
| Move a sharp workflow | [Migration](./MIGRATION_FROM_SHARP.md) |
| Check formats and runtime support | [Compatibility](./COMPATIBILITY.md) |
| Use browser/Edge upload preflight | [Wasm API](./WASM_PACKAGE_API.md) |

## Use the API

- [Node API](./API.md) — constructors, compiler policy, output helpers and errors.
- [Examples](../examples/README.md) and [TypeScript](./TYPESCRIPT.md).
- [Metadata](./METADATA_SUPPORT.md), [quality](./QUALITY_SEMANTICS.md), [input validation](./INPUT_VALIDATION.md).
- [Lazy execution](./LAZY_SEMANTICS.md) — when work runs.

## Operate and evaluate

- [Troubleshooting](./TROUBLESHOOTING.md) and [error codes](./ERROR_CODES.md).
- [Metrics](./metrics-api.md), [memory/file I/O](./ZERO_COPY.md), [thread model](./THREAD_MODEL.md).
- [Performance](./PERFORMANCE.md) — what the recorded results do and do not establish.
- [Product direction](./PROJECT_PHILOSOPHY.md), [roadmap](./ROADMAP.md), [competitor comparison / 日本語](./COMPETITIVE_ANALYSIS.md).

## Contribute

[Contributor documentation](./DEVELOPMENT.md) covers architecture, specifications,
benchmark evidence and releases. [CHANGELOG](../CHANGELOG.md) records versioned
changes. [Security policy](../SECURITY.md) explains vulnerability reporting.
