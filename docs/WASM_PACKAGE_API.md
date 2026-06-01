# Wasm Package And Policy API

This document defines the intended package shape and shared policy contract for
the future Wasm/browser/Edge track. It is a design contract, not a published
package announcement. The native Node package remains the production package
until a Wasm package is implemented and benchmarked.

## Package Shape

Planned package name:

- `@alberteinshutoin/lazy-image-wasm`

Planned entrypoints:

| Entrypoint | Runtime | Purpose |
|---|---|---|
| `@alberteinshutoin/lazy-image-wasm/browser` | Browser main thread or Web Worker | Upload preflight optimizer for `File`, `Blob`, `ArrayBuffer`, and `Uint8Array` inputs |
| `@alberteinshutoin/lazy-image-wasm/edge` | V8-isolate Edge runtimes | Isolate-safe optimizer with no Node.js native APIs and no filesystem dependency |
| `@alberteinshutoin/lazy-image-wasm/shared` | Browser, Edge, build tooling | Shared TypeScript types, policy normalization, error helpers, and metrics vocabulary |
| `@alberteinshutoin/lazy-image-wasm/worker` | Browser Web Worker | Optional worker helper for off-main-thread upload optimization |

The package should be ESM-first. CommonJS compatibility is not a first-release
goal because many Edge runtimes and modern browser bundlers expect ESM.

## Runtime Constraints

The Wasm package must not depend on:

- Node.js NAPI
- `fs`, `path`, `os`, `worker_threads`, or native platform packages
- file descriptors, mmap, advisory file locks, or cgroup detection
- native thread pools
- dynamic filesystem loading of codec assets

The browser and Edge entrypoints should support an explicit Wasm asset loading
strategy so bundlers and Edge runtimes can decide how the `.wasm` asset is
fetched or embedded.

```typescript
import { createUploadOptimizer } from '@alberteinshutoin/lazy-image-wasm/browser';

const optimizer = await createUploadOptimizer({
  wasmUrl: new URL('./lazy_image_wasm_bg.wasm', import.meta.url),
});
```

The package may use `WebAssembly.instantiateStreaming()` when available, but it
must provide a fallback for runtimes that return the wrong MIME type for Wasm
assets.

## Public API

The first public API should be high-level and policy-oriented:

```typescript
import { optimizeUpload } from '@alberteinshutoin/lazy-image-wasm/browser';

const result = await optimizeUpload(file, {
  format: 'webp',
  maxWidth: 1600,
  maxHeight: 1600,
  targetBytes: 350_000,
  stripMetadata: true,
  profile: 'upload-safe',
  output: 'blob',
});

console.log(result.metrics.bytesIn, result.metrics.bytesOut, result.metrics.qualityUsed);
```

Edge usage should use the same policy shape, but avoid DOM-only assumptions:

```typescript
import { optimizeUpload } from '@alberteinshutoin/lazy-image-wasm/edge';

const input = new Uint8Array(await request.arrayBuffer());
const result = await optimizeUpload(input, {
  format: 'jpeg',
  maxWidth: 1200,
  maxHeight: 1200,
  targetBytes: 250_000,
  output: 'arrayBuffer',
});
```

## Shared Policy Types

```typescript
type WasmImageInput = File | Blob | ArrayBuffer | Uint8Array;
type WasmOutputFormat = 'jpeg' | 'webp';
type WasmOutputKind = 'blob' | 'arrayBuffer' | 'uint8Array';
type WasmResizeFit = 'inside' | 'cover' | 'fill';
type WasmQualityFloorPolicy = 'best-effort' | 'strict';
type WasmProfile = 'upload-safe' | 'avatar' | 'attachment' | 'balanced';

interface WasmOptimizeOptions {
  format: WasmOutputFormat;
  output?: WasmOutputKind;
  maxWidth?: number;
  maxHeight?: number;
  fit?: WasmResizeFit;
  targetBytes?: number;
  minQuality?: number;
  maxQuality?: number;
  qualityFloorPolicy?: WasmQualityFloorPolicy;
  stripMetadata?: boolean;
  profile?: WasmProfile;
  limits?: WasmLimits;
  signal?: AbortSignal;
}

interface WasmLimits {
  maxBytes?: number;
  maxPixels?: number;
  timeoutMs?: number;
}
```

Default policy:

- `fit`: `inside`
- `output`: `blob` for browser, `arrayBuffer` for Edge
- `stripMetadata`: `true`
- `qualityFloorPolicy`: `best-effort`
- `profile`: `upload-safe`

The Wasm policy names should map to the native package's safety and byte-budget
vocabulary, but the Wasm API should not expose native-only file or thread
concepts.

## Result And Metrics

```typescript
interface WasmOptimizeResult {
  data: Blob | ArrayBuffer | Uint8Array;
  format: WasmOutputFormat;
  qualityUsed: number;
  bytesOut: number;
  budgetMet: boolean;
  targetBytes?: number;
  metrics: WasmProcessingMetrics;
}

interface WasmProcessingMetrics {
  version: string;
  runtime: 'browser' | 'browser-worker' | 'edge-isolate';
  bytesIn: number;
  bytesOut: number;
  formatIn?: string | null;
  formatOut: WasmOutputFormat;
  widthIn?: number;
  heightIn?: number;
  widthOut?: number;
  heightOut?: number;
  qualityUsed: number;
  budgetMet: boolean;
  targetBytes?: number;
  instantiateMs?: number;
  decodeMs: number;
  opsMs: number;
  encodeMs: number;
  totalMs: number;
  firstEncodeMs?: number;
  metadataStripped: boolean;
  policyViolations: string[];
  memory?: {
    peakBytes?: number;
    source: 'performance-api' | 'runtime-estimate' | 'unavailable';
  };
}
```

Metrics should preserve the native field names where they mean the same thing:
`decodeMs`, `opsMs`, `encodeMs`, `totalMs`, `bytesIn`, `bytesOut`,
`formatIn`, `formatOut`, `metadataStripped`, and `policyViolations`.

Wasm-specific fields such as `instantiateMs`, `firstEncodeMs`, and
`browserBundleGzipBytes` belong in benchmark reports, not every application
result, unless the runtime can report them cheaply and accurately.

## Errors

The Wasm package should keep the same broad error taxonomy as the native
package:

| Range | Category | Wasm examples |
|---|---|---|
| `E1xx` | Input | unsupported input type, corrupt image, unsupported input format |
| `E2xx` | Processing | invalid resize, byte-budget search failure in strict mode |
| `E3xx` | Output | unsupported output format, encode failure |
| `E4xx` | Config | invalid profile, invalid policy, invalid limits |
| `E9xx` | Internal | Wasm initialization failure, unexpected codec panic boundary |

Native filesystem errors such as file-not-found, mmap failure, and file-write
failure must not appear in the browser/Edge API.

```typescript
interface LazyImageWasmError extends Error {
  code: string;
  category: 'Input' | 'Processing' | 'Output' | 'Config' | 'Internal';
  recoverable: boolean;
  recoveryHint?: string;
}
```

## Native APIs Not In Wasm

The Wasm package should not expose:

- `ImageEngine.fromPath()`
- `inspectFile()`
- `toFile()` / `encodeToFile()`
- `toFileTargetBytes()`
- `processBatch()` / `processBatchWithMetrics()`
- `createStreamingPipeline()`
- mmap or zero-copy file I/O controls
- cgroup memory detection
- native thread pool or concurrency controls
- platform-specific optional binary packages

If a future Wasm API needs batch behavior, it should be a browser/Edge-specific
helper built around repeated `optimizeUpload()` calls and explicit caller-side
concurrency.

## First Release Scope

In scope:

- JPEG and WebP output
- resize to max width and/or max height
- metadata stripping by default
- best-effort target-byte search
- strict byte-budget failure mode
- structured errors
- metrics compatible with the native vocabulary
- Web Worker example
- benchmark reports using `npm run test:bench:wasm`

Out of scope:

- full native `ImageEngine` parity
- raw codec-wrapper-only API
- native-throughput parity claims
- AVIF as an MVP blocker
- drawing, compositing, filters, animation, TIFF, PDF, SVG, RAW
- filesystem or Node-only APIs

## Acceptance For Implementation Work

Implementation-heavy Wasm issues should not start until this contract is either
accepted as-is or updated by a dedicated design PR. After implementation begins,
changes to the public policy shape should be treated as API design changes, not
incidental benchmark or build-system work.
