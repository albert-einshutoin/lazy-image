# API Reference

Full API reference for lazy-image. For a quick start, see [README.md](../README.md#-basic-usage).

## Constructors

| Method | Description |
|--------|-------------|
| `ImageEngine.from(buffer)` | Create engine from a Buffer (loads into V8 heap) |
| `ImageEngine.fromPath(path)` | **Recommended**: Create engine from file path (bypasses V8 heap). Uses memory mapping for zero-copy access. **Note**: On Windows, memory-mapped files cannot be deleted while mapped. See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md#windows-file-locking). |

## Pipeline Operations (chainable)

| Method | Description |
|--------|-------------|
| `.resize(width?, height?, fit?)` | Resize image (`fit`: `'inside'` default, `'cover'` to crop + fill, `'fill'` to ignore aspect ratio) |
| `.crop(x, y, width, height)` | Crop a region |
| `.rotate(degrees)` | Rotate (90, 180, 270) |
| `.flipH()` | Flip horizontally |
| `.flipV()` | Flip vertically |
| `.grayscale()` | Convert to grayscale |
| `.keepMetadata(options?)` | Preserve ICC and EXIF metadata. GPS stripped by default for privacy. See [ARCHITECTURE.md](./ARCHITECTURE.md#metadata-handling). |
| `.brightness(value)` | Adjust brightness (-100 to 100) |
| `.contrast(value)` | Adjust contrast (-100 to 100) |
| `.normalizePixelFormat()` | Normalize pixel format to RGB/RGBA without color space conversion. |
| `.toColorspace(space)` | ⚠️ **DEPRECATED** - Use `.normalizePixelFormat()` instead. |
| `.preset(name)` | Apply preset (`'thumbnail'`, `'avatar'`, `'hero'`, `'social'`) |
| `.sanitize({ policy?, ... })` | Image Firewall: apply strict/lenient limits. See [ARCHITECTURE.md](./ARCHITECTURE.md#image-firewall-mode-strict--lenient). |
| `.limits({ maxPixels?, maxBytes?, timeoutMs? })` | Override firewall limits. |

## Output

| Method | Description |
|--------|-------------|
| `.toBuffer(format, quality?)` | Encode to Buffer. Format: `'jpeg'`, `'png'`, `'webp'`, `'avif'`. Default quality: JPEG=85, WebP=80, AVIF=60. |
| `.toBufferWithMetrics(format, quality?)` | Encode with performance metrics. Returns `{ data: Buffer, metrics: ProcessingMetrics }`. |
| `.toFile(path, format, quality?)` | **Recommended**: Write directly to file (memory-efficient). Returns bytes written. |
| `.processBatch(inputs, outDir, { format, quality?, fastMode?, concurrency? })` | Process multiple images in parallel. Returns array of `BatchResult`. `concurrency`: workers (0 = CPU cores). |
| `.clone()` | Clone the engine for multi-output (e.g. same pipeline to JPEG + WebP + AVIF). |

## Utilities

| Method | Description |
|--------|-------------|
| `inspect(buffer)` | Get metadata from Buffer without decoding pixels |
| `inspectFile(path)` | **Recommended**: Get metadata from file without loading into memory |
| `.dimensions()` | Get `{ width, height }` (requires decode) |
| `.hasIccProfile()` | Returns ICC profile size in bytes, or null if none |
| `createStreamingPipeline({ format, quality, ops })` | Disk-backed bounded-memory pipeline. See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md#streaming). |

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

interface BatchResult {
  source: string;
  success: boolean;
  error?: string;
  outputPath?: string;
}
```

Metrics payloads are versioned. See [metrics-api.md](./metrics-api.md) and [metrics-schema.json](./metrics-schema.json).

**Deprecation**: Legacy metric field names (`decodeTime`, `processTime`, etc.) will be removed in v2.0.0. Use `decodeMs`, `opsMs`, `peakRss`, `bytesIn`, `bytesOut`.

---

## Quality Metrics (SSIM/PSNR)

lazy-image enforces quality parity with sharp in CI: SSIM ≥ 0.995, PSNR ≥ 40 dB. These are used in benchmarks only; no public API for them yet.

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
