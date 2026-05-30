# API Reference

Full API reference for lazy-image. For a quick start, see [README.md](../README.md#-basic-usage).

## Constructors

| Method | Description |
|--------|-------------|
| `ImageEngine.from(buffer)` | Create engine from a Buffer (loads into V8 heap). Full pixel decode is deferred, but constructor-time metadata extraction still runs. |
| `ImageEngine.fromPath(path)` | **Recommended**: Create engine from file path (bypasses V8 heap). Full pixel decode is deferred, but constructor-time file setup and metadata extraction still run. Size-tiered: files ≤ 256 MB are read once into a Rust-owned buffer; files > 256 MB are accessed via `mmap` with an advisory lock and a retained fd. **Note**: On Windows, memory-mapped files (the > 256 MB tier) cannot be deleted while mapped. See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md#windows-file-locking) and [ZERO_COPY.md](./ZERO_COPY.md). |

See [LAZY_SEMANTICS.md](./LAZY_SEMANTICS.md) for the exact deferred vs eager contract.

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
| `.toColorspace(space)` | ⚠️ **DEPRECATED** - Use `.normalizePixelFormat()` instead. |
| `.preset(name)` | ⚠️ **DEPRECATED** - Applies a preset (`'thumbnail'`, `'avatar'`, `'hero'`, `'social'`) by mutating the pipeline and returning a separate `PresetResult`. Prefer `encode({ preset: name })`, `.toBufferWithPreset(name)`, or `.toFileWithPreset(path, name)` for self-contained preset output. |
| `.sanitize({ policy?, ... })` | Image Firewall: apply strict/lenient limits. See [ARCHITECTURE.md](./ARCHITECTURE.md#image-firewall-mode-strict--lenient). |
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
| `.toFileTargetBytes(path, format, options)` | File variant of byte-budget encoding. Returns `{ bytesWritten, quality, budgetMet, targetBytes, metrics }`. |
| `.toFileProfile(path, format, profile, quality?)` | Profile-based file encode. |
| `.processBatch(inputs, outDir, { format, quality?, fastMode?, concurrency? })` | Process multiple images in parallel. Returns array of `BatchResult`. `concurrency`: workers (0 = CPU cores). |
| `.processBatchWithMetrics(inputs, outDir, { format, quality?, fastMode?, concurrency? })` | Batch processing with per-item metrics and a summary. |
| `.clone()` | Clone the engine for multi-output (e.g. same pipeline to JPEG + WebP + AVIF). |

## Utilities

| Method | Description |
|--------|-------------|
| `inspect(buffer)` | Get metadata from Buffer without decoding pixels |
| `inspectFile(path)` | **Recommended**: Get metadata from file without loading into memory |
| `.dimensions()` | Get `{ width, height }` (requires decode) |
| `.hasIccProfile()` | Returns ICC profile size in bytes, or null if none |
| `resolveEncodeProfile(format, profile, quality?)` | Resolve profile into concrete `{ format, quality, fastMode }` options. |
| `createStreamingPipeline({ format, quality, ops, onMetrics? })` | Disk-backed bounded-memory pipeline. Not a true chunk-by-chunk transform stream. Optional `onMetrics` callback receives `ProcessingMetrics` after file output completes. |

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
  format: string | null;
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

type PresetName = 'thumbnail' | 'avatar' | 'hero' | 'social';

interface EncodeOptions {
  format?: 'jpeg' | 'jpg' | 'png' | 'webp' | 'avif';
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
  metadataStripped: boolean;
  policyViolations: string[];
  /** @deprecated use decodeMs */
  decodeTime: number;
  /** @deprecated use opsMs */
  processTime: number;
  /** @deprecated use encodeMs */
  encodeTime: number;
  /** @deprecated use peakRss */
  memoryPeak: number;
  /** @deprecated use bytesIn */
  inputSize: number;
  /** @deprecated use bytesOut */
  outputSize: number;
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
  targetBytes?: number;
  maxBytes?: number;
  minQuality?: number;
  maxQuality?: number;
  fastMode?: boolean;
  qualityFloorPolicy?: 'best-effort' | 'strict';
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

**Deprecation**: Legacy metric field names (`decodeTime`, `processTime`, etc.) will be removed in v2.0.0. Use `decodeMs`, `opsMs`, `peakRss`, `bytesIn`, `bytesOut`.

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
