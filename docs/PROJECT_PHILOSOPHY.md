# Project Philosophy

`lazy-image` is a focused web-image optimization engine, not a general-purpose image toolkit.

## Priorities

1. Smaller JPEG outputs than typical `sharp` defaults for simple web-delivery benchmarks, with codec- and pipeline-specific claims for other formats or operation chains.
2. Predictable memory behavior and bounded-resource execution.
3. Safe defaults for metadata, decompression limits, and operational use.
4. A non-destructive, lazy pipeline API that fits batch and build workflows.
5. Portable upload-preflight policies where Wasm/browser/Edge runtimes can benefit from the same safety and byte-budget defaults.

## Non-goals

- Winning raw JPEG/WebP throughput benchmarks across all workloads.
- Matching the full breadth of `sharp` or ImageMagick-style editing APIs.
- Being a drop-in replacement for `sharp`.
- Treating Wasm as a native-performance replacement for the Node package.

## How to interpret performance claims

- For JPEG, `lazy-image` intentionally prioritizes smaller files in the two simple canonical benchmarked scenarios; additional operations can change the size result.
- For WebP, do not assume smaller output or faster encoding; benchmark the target workload.
- For AVIF, do not assume a blanket speed or size win over `sharp`; benchmark the target workload and treat any advantage as scenario-specific, not the project's primary identity.
- The project should describe benchmark wins precisely by codec and workload, and avoid broad claims like "faster than sharp" without scope.

## Wasm direction

The Wasm track should compete in browser upload preflight and Edge-style V8 isolate workflows, not in native Node throughput. Its useful differentiators are policy and portability: metadata stripping, target-byte search, safe upload defaults, Web Worker ergonomics, and reproducible browser/Edge benchmarks.

See [WASM_STRATEGY.md](./WASM_STRATEGY.md) and [WASM_BENCHMARKING.md](./WASM_BENCHMARKING.md) for the current strategy and evidence requirements.
