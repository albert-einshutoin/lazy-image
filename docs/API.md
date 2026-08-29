# API Reference

Full API reference for lazy-image. For a quick start, see [README.md](../README.md#-basic-usage).

This page documents the native Node.js package. The published browser/Edge Wasm
MVP is intentionally narrower and is documented separately in
[WASM_PACKAGE_API.md](./WASM_PACKAGE_API.md).

## Constructors

| Method | Description |
|--------|-------------|
| `ImageEngine.from(buffer)` | Create engine from a Buffer. The bytes cross the V8 boundary and are copied into Rust-owned memory; metadata extraction and full pixel decode are deferred. |
| `await ImageEngine.fromPathAsync(path)` | **Recommended for servers**: performs file open/read/mmap setup on an N-API worker so a ≤256 MB source read does not block the Node.js event loop. It uses the same SIGBUS-safe source policy and error codes as `fromPath()`. |
| `ImageEngine.fromPath(path)` | Synchronous compatibility API (bypasses V8 heap but not the event loop). Files ≤ 256 MB are synchronously read into Rust-owned memory on the calling thread; files > 256 MB use `mmap` with an advisory lock and retained fd. **Note**: On Windows, mapped files cannot be deleted while open. See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md#windows-file-locking) and [ZERO_COPY.md](./ZERO_COPY.md). |

See [LAZY_SEMANTICS.md](./LAZY_SEMANTICS.md) for the exact deferred vs eager contract.

```javascript
const engine = await ImageEngine.fromPathAsync('input.jpg');
const bytes = await engine.resize(800).toFile('output.jpg', 'jpeg', 80);
```

## Pipeline Operations (chainable)

| Method | Description |
|--------|-------------|
| `.resize(options)` / `.resize(width?, height?, fit?)` | Resize image (`fit`: `'inside'` default, `'cover'` to crop + fill, `'fill'` to ignore aspect ratio) |
| `.crop(x, y, width, height)` | Crop a region |
| `.rotate(degrees)` | Rotate (90, 180, 270) |
| `.flipH()` | Flip horizontally |
| `.flipV()` | Flip vertically |
| `.grayscale()` | Convert to grayscale |
| `.keepMetadata(options?)` | Preserve ICC and EXIF metadata. GPS stripped by default for privacy. XMP is not currently preserved. See [METADATA_SUPPORT.md](./METADATA_SUPPORT.md). |
| `.brightness(value)` | Adjust brightness (-100 to 100) |
| `.contrast(value)` | Adjust contrast (-100 to 100) |
| `.normalizePixelFormat()` | Normalize pixel format to RGB/RGBA without color space conversion. |
| `.preset(name)` | ⚠️ **DEPRECATED** - Applies a preset (`'thumbnail'`, `'avatar'`, `'hero'`, `'social'`) by mutating the pipeline and returning a separate `PresetResult`. Prefer `encode({ preset: name })`, `.toBufferWithPreset(name)`, or `.toFileWithPreset(path, name)` for self-contained preset output. |
| `.sanitize({ policy? })` | Image Firewall: apply `strict`, `lenient`, or `public-upload`. The public-upload policy keeps strict resource limits, preserves only validated ICC up to 512 KiB, and always strips EXIF/GPS/XMP. See [METADATA_SUPPORT.md](./METADATA_SUPPORT.md). |
| `.limits({ maxPixels?, maxBytes?, timeoutMs? })` | Override firewall limits. |

For preset-based output, prefer the self-contained APIs above the deprecated `.preset(name)` command/query mix:

```javascript
const { data } = await ImageEngine.fromPath('photo.jpg').encode({ preset: 'thumbnail' });
const buffer = await ImageEngine.fromPath('photo.jpg').toBufferWithPreset('thumbnail');
const bytes = await ImageEngine.fromPath('photo.jpg').toFileWithPreset('thumb.webp', 'thumbnail');
```

## Output

| Method | Description |
|--------|-------------|
| `.encode({ format?, quality?, fastMode?, preset?, metrics? })` | Unified encode-to-buffer API. Use `preset` for self-contained preset output; when `preset` is set it takes precedence over `format`, `quality`, and `fastMode`. Returns `{ data, metrics? }`. |
| `.encodeToFile(path, { format?, quality?, fastMode?, preset?, metrics? })` | Unified encode-to-file API. Same options as `.encode()`, returning `{ bytesWritten, metrics? }`. |
| `.toBuffer(format, quality?)` | Encode to Buffer. Format: `'jpeg'`, `'png'`, `'webp'`, `'avif'` (AVIF requires build feature `avif`). Default quality: JPEG=85, WebP=80, AVIF=60. |
| `.toBufferWithPreset(name)` | Self-contained preset buffer output. Prefer this over deprecated `.preset(name)` when writing a preset result to a Buffer. |
| `.toBufferProfile(format, profile, quality?)` | Encode using high-level profile: `'size-first'`, `'balanced'`, `'speed-first'`. |
| `.toBufferWithMetrics(format, quality?)` | Encode with performance metrics. Returns `{ data: Buffer, metrics: ProcessingMetrics }`. |
| `.toBufferWithMetricsPreset(name)` | Self-contained preset buffer output with metrics. Returns `{ data: Buffer, metrics: ProcessingMetrics }`. |
| `.toBufferWithMetricsProfile(format, profile, quality?)` | Profile-based encode with metrics. |
| `.toFile(path, format, quality?)` | **Recommended**: Write directly to file (memory-efficient). Returns bytes written. |
| `.toFileWithPreset(path, name)` | Self-contained preset file output. Prefer this over deprecated `.preset(name)` when writing a preset result to a file. |
| `.toFileWithMetrics(path, format, quality?)` | File output with metrics. Returns `{ bytesWritten, metrics }`. |
| `.toBufferTargetBytes(format, options)` | Searches quality to stay under a byte budget. Returns `{ data, quality, bytesOut, budgetMet, targetBytes, metrics }`. |
| `.toFileTargetBytes(path, format, options)` | Native file variant of byte-budget encoding. Search candidates stay Rust-owned and the winner is atomically written without crossing V8 as a Buffer. Returns metadata only: `{ bytesWritten, quality, budgetMet, targetBytes, metrics }`. The current byte/count contract is `u32`; outputs above 4 GiB fail explicitly. |
| `.toFileProfile(path, format, profile, quality?)` | Profile-based file encode. |
| `.processBatch(inputs, outDir, { format, quality?, fastMode?, concurrency? })` | Process multiple images in parallel. Returns array of `BatchResult`. `concurrency`: workers (0 = CPU cores). |
| `.processBatchWithMetrics(inputs, outDir, { format, quality?, fastMode?, concurrency? })` | Batch processing with per-item metrics and a summary. |
| `processBatchChunked(inputs, outDir, { format, quality?, fastMode?, concurrency?, chunkSize?, onProgress?, signal? })` | Top-level batch helper that splits inputs into native `processBatch()` chunks. Reports progress after each chunk and checks `AbortSignal` before starting the next chunk. |
| `.clone()` | Clone the engine for multi-output (e.g. same pipeline to JPEG + WebP + AVIF). |

### Transactional public-upload compilation

`compileImage()` is the high-level native Node.js entrypoint for one untrusted
local image. It consumes the deterministic `compileArtifactPlan()` contract,
writes plan-owned artifacts and an optional 16px WebP placeholder into a
private sibling staging directory, verifies hashes/metadata, and publishes by
one directory rename. Existing output directories are rejected; failures and
abort signals remove the unpublished staging set.

```javascript
const { compileImage } = require('@alberteinshutoin/lazy-image');

const manifest = await compileImage({
  inputPath: '/srv/uploads/upload.bin',
  outputDir: '/srv/public/images/version-1',
  policy: { widths: [320, 640], formats: ['webp'], placeholder: true },
});
```

The input is content-sniffed (the filename is not trusted), artifact filenames
are library-owned, and full artifact bytes never cross into a V8 `Buffer`.
`manifest.json` is deterministic for the same source, policy, and compiler
identity. The output parent must be trusted and on the same filesystem as the
staging directory; portable Node.js has no cross-platform no-replace directory
rename primitive, so another process must not replace that parent concurrently.

## Utilities

| Method | Description |
|--------|-------------|
| `inspect(buffer)` | Inspect JPEG/PNG/WebP header traits without decoding pixels |
| `inspectFile(path)` | **Recommended**: Inspect the same traits from a path without copying encoded bytes into the Node.js heap |
| `.dimensions()` | Get `{ width, height }` (requires decode) |
| `.hasIccProfile()` | Returns ICC profile size in bytes, or null if none |
| `resolveEncodeProfile(format, profile, quality?)` | Resolve profile into concrete `{ format, quality, fastMode }` options. |
| `compilerIdentity()` | Return the stable native codec/build identity used by artifact fingerprints. |
| `createStreamingPipeline({ format, quality, ops, onMetrics? })` | Disk-backed bounded-memory pipeline. Not a true chunk-by-chunk transform stream. Optional `onMetrics` callback receives `ProcessingMetrics` after file output completes. |

### Upload preflight contract

`inspect()` and `inspectFile()` use format-specific JPEG, PNG, and WebP header
readers. They never decode pixel buffers. On every successful call:

- `hasAlpha` is authoritative: `false` means the supported container was
  confirmed opaque, not that an unknown state was coerced to false.
- `isAnimated` is authoritative: APNG and animated WebP return `true`; JPEG is
  always static.
- `orientation` is the EXIF Orientation value `1` through `8`, or `undefined`
  when the tag is absent, invalid, cannot be parsed, or is not found within the
  64 KiB preflight scan budget.
- `width` and `height` are the encoded dimensions. They are not swapped for
  Orientation values such as `6` or `8`.

Malformed supported headers reject with the typed decode error `E131`.
Unsupported containers reject with `E111`; callers must not invent safe trait
values after either failure. `inspectFile()` uses filesystem readers for header
and EXIF inspection, so encoded image bytes do not cross the V8 boundary as a
`Buffer`. Successful preflight proves these container traits, not that every
entropy-coded payload or animation frame will decode; upload pipelines must
still decode and re-encode before publishing an artifact.

---

## Chunked Batch Progress

Use `processBatchChunked()` when a large build-time batch needs chunk-level
progress or a safe stop point between native batch calls:

```javascript
const { processBatchChunked } = require('@alberteinshutoin/lazy-image');

const controller = new AbortController();
const results = await processBatchChunked(files, './optimized', {
  format: 'webp',
  quality: 80,
  concurrency: 4,
  chunkSize: 16,
  signal: controller.signal,
  onProgress: ({ completed, total, failed }) => {
    console.log(`${completed}/${total} complete (${failed} failed)`);
  },
});
```

`chunkSize` defaults to `16`. For long-running native batches, start with roughly
`concurrency * 2` to `concurrency * 4`; very small chunks add synchronization
overhead and can leave native workers idle.

`signal.aborted` is checked only before each chunk starts. Aborting does not
cancel work already running inside native `processBatch()`, and files written by
completed chunks remain on disk. If `onProgress` throws, lazy-image logs the
callback error and continues processing the next chunk.

---

## Quality Settings (v0.7.2+)

| Format | Default Quality | Recommended Range |
|--------|-----------------|-------------------|
| **JPEG** | 85 | 70-95 |
| **WebP** | 80 | 70-90 |
| **AVIF** | 60 | 50-80 |

See [QUALITY_EFFORT_SPEED_MAPPING.md](./QUALITY_EFFORT_SPEED_MAPPING.md) for cross-format equivalence.

---

## Return Types

```typescript
interface ImageMetadata {
  width: number;
  height: number;
  format?: InputFormat;
  hasAlpha: boolean;
  isAnimated: boolean;
  orientation?: number;
}

interface Dimensions {
  width: number;
  height: number;
}

interface ResizeOptions {
  width?: number;
  height?: number;
  fit?: 'inside' | 'cover' | 'fill';
}

type CaseInsensitive<T extends string> = T | Uppercase<T> | Capitalize<T>;
type OutputFormat = CaseInsensitive<'jpeg' | 'jpg' | 'png' | 'webp' | 'avif'>;
type PresetName = CaseInsensitive<'thumbnail' | 'avatar' | 'hero' | 'social'>;

interface EncodeOptionsInput {
  format?: OutputFormat;
  quality?: number;
  fastMode?: boolean;
  preset?: PresetName;
  metrics?: boolean;
}

interface EncodeResult {
  data: Buffer;
  metrics?: ProcessingMetrics;
}

interface FileEncodeResult {
  bytesWritten: number;
  metrics?: ProcessingMetrics;
}

interface PresetResult {
  format: string;
  quality?: number;
  width?: number;
  height?: number;
}

interface ProcessingMetrics {
  version: string;
  decodeMs: number;
  opsMs: number;
  encodeMs: number;
  totalMs: number;
  peakRss: number;
  cpuTime: number;
  processingTime: number;
  bytesIn: number;
  bytesOut: number;
  compressionRatio: number;
  formatIn?: string | null;
  formatOut: string;
  iccPreserved: boolean;
  iccOutcome: 'absent' | 'preserved' | 'unsafe-stripped' | 'policy-stripped' | 'unsupported';
  metadataStripped: boolean;
  policyViolations: string[];
}

interface OutputWithMetrics {
  data: Buffer;
  metrics: ProcessingMetrics;
}

interface FileOutputWithMetrics {
  bytesWritten: number;
  metrics: ProcessingMetrics;
}

interface TargetBytesOptions {
  // Positive integer; current native contract is capped at u32::MAX (4,294,967,295).
  targetBytes?: number;
  maxBytes?: number;
  minQuality?: number;
  maxQuality?: number;
  fastMode?: boolean;
  qualityFloorPolicy?: 'best-effort' | 'strict';
  strict?: boolean;
}

interface BufferTargetBytesResult {
  data: Buffer;
  quality: number;
  bytesOut: number;
  budgetMet: boolean;
  targetBytes: number;
  metrics: ProcessingMetrics;
}

interface FileTargetBytesResult {
  bytesWritten: number;
  quality: number;
  budgetMet: boolean;
  targetBytes: number;
  metrics: ProcessingMetrics;
}

interface BatchResult {
  source: string;
  success: boolean;
  error?: string;
  outputPath?: string;
  errorCode?: string;
  errorCategory?: ErrorCategory;
  effectiveConcurrency?: number;
  autoConcurrency?: boolean;
}

interface BatchResultWithMetrics extends BatchResult {
  metrics?: ProcessingMetrics;
}

interface BatchMetricsSummary {
  totalItems: number;
  successfulItems: number;
  failedItems: number;
  effectiveConcurrency: number;
  autoConcurrency: boolean;
  totalBytesIn: number;
  totalBytesOut: number;
  totalDecodeMs: number;
  totalOpsMs: number;
  totalEncodeMs: number;
  totalCpuTime: number;
  totalWallMs: number;
}

interface BatchOutputWithMetrics {
  items: BatchResultWithMetrics[];
  summary: BatchMetricsSummary;
}
```

Metrics payloads are versioned. See [metrics-api.md](./metrics-api.md) and [metrics-schema.json](./metrics-schema.json).

Only the canonical fields above are part of the v1.x contract. Legacy metric
aliases removed before v1 are not returned.

---

## Quality Metrics (SSIM/PSNR)

Benchmark helpers can report SSIM/PSNR against sharp outputs for local comparison. These metrics are benchmark-only helpers; there is no public API for them yet, and public performance claims should cite the scenario-specific data in [TRUE_BENCHMARKS.md](./TRUE_BENCHMARKS.md).

---

## Error Handling

Errors use structured codes (E1xx–E9xx). All errors include `message`, and may include `errorCode` and `recoveryHint`.

| Category | Range | Description |
|----------|-------|-------------|
| **E1xx** | 100-199 | Input Errors |
| **E2xx** | 200-299 | Processing Errors |
| **E3xx** | 300-399 | Output Errors |
| **E4xx** | 400-499 | Configuration Errors |
| **E9xx** | 900-999 | Internal Errors |

**Full list**: [ERROR_CODES.md](./ERROR_CODES.md).

### Example (JavaScript/TypeScript)

```javascript
try {
  await ImageEngine.fromPath('input.jpg').resize(800).toBuffer('jpeg', 85);
} catch (error) {
  const errorCode = error.message.match(/\[E\d+\]/)?.[0];
  if (error.errorCode) { /* use error.errorCode, error.recoveryHint */ }
}
```

### Recoverable vs non-recoverable

- **Recoverable**: Invalid path, crop bounds, rotation angle, etc. — fix input and retry.
- **Non-recoverable**: Unsupported format, encode failure — do not retry with same input.

See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md#error-recovery) for common cases.
