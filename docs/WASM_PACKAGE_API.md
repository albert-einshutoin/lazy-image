# Wasm Package And Policy API

This document defines the package shape and shared policy contract for the
Wasm/browser/Edge track. The MVP was first published in v1.0.0 as
`@alberteinshutoin/lazy-image-wasm` and lives under `packages/lazy-image-wasm`.
Published v1.4.0 has been exercised in a Chrome Web Worker and a fixed local
workerd configuration, as well as in Node with an `ImageData` shim and explicit
Wasm bytes. See the [v1.4.0 verification record](./history/V1.4.0_VERIFICATION.md)
for the separate runtime conditions. The native package remains the recommended
production path for Node.js build-time, batch, serverless, and file workflows:
it supplies those Node-oriented APIs, while the measured Node-Wasm path needs a
shim and manual codec injection. The selected runtime measurements do not
establish a general performance advantage; the remaining evidence for public
performance claims is tracked in [#645](https://github.com/albert-einshutoin/lazy-image/issues/645).

## Package Shape

Package name:

- `@alberteinshutoin/lazy-image-wasm`

Published entrypoints:

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
import type { CreateUploadOptimizerOptions } from '@alberteinshutoin/lazy-image-wasm/shared';

declare const wasmModules: NonNullable<CreateUploadOptimizerOptions['wasmModules']>;

const optimizer = await createUploadOptimizer({ wasmModules });
```

Each value is a `WebAssembly.Module`, `ArrayBuffer`, or `Uint8Array` resolved by
the target bundler or runtime. The verified local workerd configuration uses
**static `WebAssembly.Module` imports**: accepting Wasm bytes at the API boundary
does not imply that an isolate permits dynamic compilation. See the
[workerd verification](./history/WASM_1.4.0_EDGE_VERIFICATION.md) for the five
module assignments and the shipped Edge example for deployment binding names
and isolate-level optimizer caching. In the verified Chrome module Worker,
the published codec loader instead resolves copied `.wasm` assets without
passing `wasmModules`; that loading mode has its own [HTTP and bundle requirements](../packages/lazy-image-wasm/README.md#worker).

The package may use `WebAssembly.instantiateStreaming()` when available, but it
must provide a fallback for runtimes that return the wrong MIME type for Wasm
assets.

The first implementation uses jSquash Wasm codecs as the low-level browser/V8
codec layer and keeps lazy-image's differentiation in policy normalization,
byte-budget search, metrics, and structured errors. A future Rust-owned codec
core can replace that adapter without changing the public policy shape.

## Public API

The public API is high-level and policy-oriented:

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

Edge usage should use the same policy shape, but avoid DOM-only assumptions.
For the verified local workerd setup, the bundler/workerd configuration embeds
the unmodified codec Wasm files as static `WebAssembly.Module` imports; the
five names below match the [verified Worker](../test/benchmarks/wasm-edge-worker.mjs):

```typescript
import { createUploadOptimizer } from '@alberteinshutoin/lazy-image-wasm/edge';
import jpegDecode from './mozjpeg_dec.wasm';
import jpegEncode from './mozjpeg_enc.wasm';
import pngDecode from './squoosh_png_bg.wasm';
import resize from './squoosh_resize_bg.wasm';
import webpEncode from './webp_enc.wasm';

const optimizer = await createUploadOptimizer({
  wasmModules: { jpegDecode, jpegEncode, pngDecode, resize, webpEncode },
});
const input = new Uint8Array(await request.arrayBuffer());
const result = await optimizer.optimizeUpload(input, {
  format: 'jpeg',
  maxWidth: 1200,
  maxHeight: 1200,
  targetBytes: 250_000,
  output: 'arrayBuffer',
});
```

These five modules cover the verified JPEG/PNG inputs and JPEG/WebP outputs.
Processing WebP input also needs a static `webpDecode` module from
`webp_dec.wasm` in this workerd setup.

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

`fit: "inside"` preserves aspect ratio within the provided max dimensions.
`fit: "cover"` crops and resizes to exact `maxWidth` and `maxHeight`.
`fit: "fill"` stretches to the provided dimensions. `limits.timeoutMs` is a
best-effort, inter-stage checkpoint across input, decode, resize, and encode;
it does not preempt a codec while that codec is already running.

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

The Wasm package uses the native package as the source of truth for error
codes and categories. A shared code always has the same meaning in both
packages. The `E5xx` range is reserved for failures that exist only at a Wasm
runtime boundary.

| Code | Meaning | Category | Previous Wasm code |
|---|---|---|---|
| `E111` | Unsupported input image format | `CodecError` | `E103` |
| `E122` | Input exceeds the pixel limit | `ResourceLimit` | `E105` |
| `E123` | Input-byte or timeout policy violation | `ResourceLimit` | `E104` / `E205` |
| `E131` | Codec failed to decode the input | `CodecError` | `E102` |
| `E203` | Cover resize is missing a required dimension | `UserError` | `E202` |
| `E204` | Unsupported resize fit | `UserError` | `E202` |
| `E300` | Codec failed to encode the output | `CodecError` | `E303` |
| `E400` | Invalid input type, option, output format, or output kind | `UserError` | `E101` / `E301` / `E302` / `E402` |
| `E401` | Invalid Wasm profile | `UserError` | `E401` |
| `E500` | Operation aborted through `AbortSignal` | `UserError` | `E204` |
| `E501` | Requested output kind is unavailable in the runtime | `ResourceLimit` | `E303` |
| `E502` | Strict target-byte budget cannot be met above the quality floor | `ResourceLimit` | `E201` |
| `E503` | Resize codec failed | `CodecError` | — |
| `E901` | Codec returned an invalid buffer type | `InternalBug` | `E901` |

`UserError` and `ResourceLimit` are recoverable by default. `CodecError` and
`InternalBug` are not. Callers should branch on both `code` and `category`
instead of deriving a category from the numeric range.

For `E131` (decode), `E503` (resize), and `E300` (encode), `message` identifies the codec stage
and retains the available underlying exception text. That stage can include
codec Wasm loading/initialization or image processing; the exception alone
does not prove which one failed. `recoveryHint` points browser Worker users to
the codec `.wasm` beside the bundled `worker.js`, DevTools Network's request
URL/status, and the response body. If delivery is valid, inspect input
corruption or codec processing instead. The package does not observe the
codec's HTTP response directly, so it does not report a URL/status or assert
that an asset is missing. The Worker forwards these existing fields; no new
error property is required.

Native filesystem errors such as file-not-found, mmap failure, and file-write
failure must not appear in the browser/Edge API.

```typescript
interface LazyImageWasmError extends Error {
  code: string;
  category: 'UserError' | 'CodecError' | 'ResourceLimit' | 'InternalBug';
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

## v1.0.0 Scope

In scope:

- JPEG and WebP output
- JPEG, PNG, and WebP input
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

## Compatibility Guardrails

The package is public as of v1.0.0. Changes to its policy shape, error taxonomy,
entrypoints, or default behavior follow the same SemVer policy as the native
package and must not be introduced as incidental benchmark or build-system work.
