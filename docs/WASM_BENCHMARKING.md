# Wasm Upload Benchmarking

This document defines how to evaluate the browser/Edge upload-preflight track.
It complements the native sharp comparison benchmarks; it does not replace
`TRUE_BENCHMARKS.md` as the source for public native performance claims.
The published package and policy API is defined in
[WASM_PACKAGE_API.md](./WASM_PACKAGE_API.md).
The initial implementation package lives in `packages/lazy-image-wasm`; this
benchmark remains the comparison artifact generator rather than a replacement
for browser-runner validation.

## Commands

```bash
npm run test:bench:wasm
npm run bench:wasm:browser
npm run bench:wasm:edge
npm run bench:wasm:report
```

The commands write stable artifacts to:

- `artifacts/benchmark/wasm-upload-summary.json`
- `artifacts/benchmark/wasm-upload-summary.md`

`artifacts/` is ignored by git, so benchmark snapshots should be copied into a
dedicated documentation update only when the numbers are intended to become
public evidence.

## What The Harness Measures

The first harness covers representative upload-preflight scenarios:

- large photo-style JPEG input resized to WebP under a byte budget
- large PNG input resized to JPEG under a byte budget

The report schema includes:

- browser bundle size and gzip transfer size when a real browser bundle artifact is available
- npm package-directory size and gzip size as a local diagnostic for installed optional baselines
- instantiate or module-load time
- first encode latency
- encode time and total wall time
- output bytes
- target-byte hit rate
- chosen quality
- SSIM and PSNR against a resized source reference
- metadata stripping behavior
- memory information where the runtime exposes useful data

The native `@alberteinshutoin/lazy-image` row is a Node reference. It gives a
quality, byte-budget, and latency baseline for the Wasm package, but it
is not a browser runtime result.

## Competitor Baselines

The harness includes rows for the browser/Edge competitors that matter for this
strategy:

- Wasm codec packages: jSquash and Squoosh
- browser compressor packages: browser-image-compression and Compressor.js

If those optional packages are not installed, the row is reported as
`unavailable` with the missing package names. If a package is installed but no
runtime adapter exists yet, the row is reported as `not-run` and can still
contribute package-directory size diagnostics.

Do not publish performance comparisons against unavailable or `not-run` rows.
Those rows are evidence of benchmark coverage, not evidence of runtime wins.
Do not use package-directory gzip size as browser bundle evidence; browser
bundle claims require a bundled entrypoint and `browserBundleGzipBytes`.

## Public-Safe Claims

Safe public claims:

- "lazy-image provides a browser/Edge upload-preflight Wasm MVP; production
  suitability remains runtime- and workload-specific."
- "The Wasm benchmark harness records browser bundle size when a runtime adapter
  provides it, plus load latency, first encode latency, byte-budget hit rate,
  quality, metadata behavior, and memory where available."
- "The native package remains the recommended production path for Node.js,
  serverless Node functions, and batch pipelines."

Workload-specific claims that require fresh benchmark artifacts:

- smaller output than a browser compressor
- faster first encode than a Wasm codec package
- smaller gzip browser bundle than another browser package
- better byte-budget hit rate than a competitor
- acceptable Edge-isolate cold-start behavior

Do not turn a single browser upload result into a blanket claim about all image
processing, all codecs, or all runtimes.

## When To Refresh

Refresh the Wasm upload report when:

- a Wasm/browser package is added or changed
- codec defaults, byte-budget search, or resize behavior changes
- optional competitor packages are added or upgraded
- release docs mention browser/Edge or upload-preflight performance
- public comparison claims are being updated
