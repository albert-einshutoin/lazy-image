# Project Philosophy

`lazy-image` is a focused web-image optimization engine, not a general-purpose image toolkit.

## Priorities

1. Smaller outputs than typical `sharp` defaults for web delivery.
2. Predictable memory behavior and bounded-resource execution.
3. Safe defaults for metadata, decompression limits, and operational use.
4. A non-destructive, lazy pipeline API that fits batch and build workflows.

## Non-goals

- Winning raw JPEG/WebP throughput benchmarks across all workloads.
- Matching the full breadth of `sharp` or ImageMagick-style editing APIs.
- Being a drop-in replacement for `sharp`.

## How to interpret performance claims

- For JPEG/WebP, `lazy-image` intentionally trades encode latency for smaller files.
- For AVIF, `lazy-image` may be faster than `sharp` in some real workloads, but that is an advantage within a narrower target area, not the project's primary identity.
- The project should describe benchmark wins precisely by codec and workload, and avoid broad claims like "faster than sharp" without scope.
