# Wasm Strategy

`lazy-image` should not try to beat sharp in the same native throughput lane.
The native package is already positioned around smaller web outputs, bounded
memory, and safe defaults. The Wasm track should extend those strengths into
runtimes where sharp cannot run or is not the relevant competitor.

## Positioning

The best first Wasm product slice is browser upload preflight:

- accept `File`, `Blob`, `ArrayBuffer`, or `Uint8Array`
- resize large user uploads before they leave the browser
- strip metadata by default
- search for a target byte budget with best-effort quality
- return metrics that map to the native metrics vocabulary
- run in a Web Worker and Edge-style V8 isolate where feasible

This competes with browser-image-compression, Compressor.js, jSquash, and
Squoosh-style packages more directly than it competes with sharp.

The package and policy contract is defined in
[WASM_PACKAGE_API.md](./WASM_PACKAGE_API.md).

## Where Native Remains The Right Answer

Use the native Node package for:

- build-time and batch optimization
- serverless Node functions
- CDN generation jobs
- large file-to-file workflows
- production paths where native binaries are allowed

Native should remain the higher-capability, lower-risk production path for
Node.js runtimes.

## Where Wasm Can Differentiate

Wasm can become differentiated OSS if it focuses on policy and workflow instead
of only exposing raw codecs:

- high-level upload optimizer API instead of low-level encode/decode functions
- metadata stripping and privacy defaults
- byte-budget search as a first-class feature
- Web Worker example and browser-safe input/output types
- reproducible browser/Edge benchmark evidence
- shared policy vocabulary with the native package

The language/runtime advantage is portability, not raw performance. Rust still
helps by sharing validation, policy, and codec orchestration logic between
native and Wasm builds, but Wasm should be judged by browser/Edge fit, bundle
cost, cold-start behavior, output bytes, and user-upload ergonomics.

## Non-Goals For The First Wasm Slice

- Full `ImageEngine` parity with the native package
- Replacing sharp in Node.js
- Text, drawing, compositing, filters, GIF/APNG, TIFF, PDF, or RAW workflows
- AVIF as a blocker for the MVP
- Blanket "faster than native" claims

## Benchmark Gate

Wasm work should not ship based on Node-local encode time alone. Before public
performance claims, collect benchmark artifacts for:

- browser bundle size and gzip transfer size from a bundled runtime artifact
- instantiate time
- first encode latency
- total encode time
- output bytes
- target-byte hit rate
- SSIM/PSNR against a resized source reference
- metadata stripping behavior
- memory behavior where the runtime exposes useful data

Use `npm run test:bench:wasm` as the first local artifact generator, then add
real browser and Edge runtime adapters as the Wasm package matures.
