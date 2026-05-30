# Adoption Guide

lazy-image is strongest when used as a **focused web optimization engine**, not as a general-purpose image editing toolkit.

## Recommended Paths

### 1. File-to-file optimization (default recommendation)

Use this when you want the lowest Node.js heap usage and the clearest production path.

```javascript
await ImageEngine.fromPath('input.jpg')
  .resize({ width: 1600, fit: 'inside' })
  .toFile('output.webp', 'webp', 80);
```

Best for:
- CDN/mobile delivery
- Serverless jobs
- Batch pipelines
- Large inputs

### 2. Upload sanitization for untrusted images

Use strict firewall mode for user-uploaded avatars, social images, or form uploads.

```javascript
await ImageEngine.fromPath('upload.jpg')
  .sanitize({ policy: 'strict' })
  .resize({ width: 1024, fit: 'inside' })
  .toFile('safe-output.jpg', 'jpeg', 85);
```

Best for:
- Avatar uploads
- Community/social platforms
- Admin dashboards handling user content

### 3. Build-time static asset optimization

Use batch processing or cloning when the same source needs multiple outputs.

```javascript
const engine = ImageEngine.fromPath('hero.png').resize({ width: 1600 });

await Promise.all([
  engine.clone().toFile('hero.jpg', 'jpeg', 85),
  engine.clone().toFile('hero.webp', 'webp', 80),
  engine.clone().toFile('hero.avif', 'avif', 60),
]);
```

Best for:
- Static site generation
- CMS export pipelines
- Media preprocessing jobs

### 4. Final optimization after editing elsewhere

Use another tool for rich editing, then run lazy-image as the final delivery optimizer.

Recommended split:
- `sharp` / ImageMagick: compositing, overlays, blur, animation, SVG/PDF/TIFF workflows
- `lazy-image`: final JPEG/WebP/AVIF optimization, metadata stripping, bounded-memory output path

This is often the easiest adoption path because it does not require replacing the existing editing stack.

## When Not to Lead With lazy-image

Use another tool first when you need:

- Compositing or overlays
- Rich filters such as blur/sharpen/tint
- Animated images
- Broad input format coverage such as TIFF, HEIF, PDF, SVG, RAW
- True chunk-by-chunk streaming transforms

---

## File-to-file vs Buffer Paths

### Decision matrix

| Path | Heap usage | Best for |
|------|-----------|----------|
| `fromPath()` -> `toFile()` | Minimal (mmap for > 256 MB; read for smaller) | Production servers, large images, serverless |
| `fromPath()` -> `toBuffer()` | Output buffer in V8 | When you need the buffer (e.g. HTTP response body) |
| `from(buffer)` -> `toBuffer()` | Full input + output in V8 | Small images already in memory |
| `from(buffer)` -> `toFile()` | Input in V8 | Writing from an in-memory source |

### When to use each path

**`fromPath()` -> `toFile()`** -- default choice for production workloads.

Use this when the image lives on disk (or can be staged to disk from S3/GCS) and the output goes back to disk or object storage. This path keeps pixel data entirely outside the V8 heap, which is critical in serverless and memory-constrained containers.

```javascript
// CDN optimization pipeline: download -> optimize -> upload
const tmp = '/tmp';
await downloadFromS3(key, `${tmp}/input.jpg`);

await ImageEngine.fromPath(`${tmp}/input.jpg`)
  .resize({ width: 1600, fit: 'inside' })
  .toFile(`${tmp}/output.webp`, 'webp', 80);

await uploadToS3(`${tmp}/output.webp`, outputKey);
```

**`fromPath()` -> `toBuffer()`** -- when you need to return the image inline.

Use this when the caller expects a Buffer (e.g. an HTTP response, a message queue payload, or a database BLOB). The input still avoids the heap, but the output buffer lives in V8.

```javascript
// Express handler returning optimized image
app.get('/image/:id', async (req, res) => {
  const buf = await ImageEngine.fromPath(localPath)
    .resize({ width: 800, fit: 'inside' })
    .toBuffer('webp', 80);
  res.type('image/webp').send(buf);
});
```

**`from(buffer)` -> `toBuffer()`** -- for small in-memory images only.

Use this when the image is already a Buffer (e.g. from a multipart upload body) and is small enough that keeping both input and output in V8 heap is acceptable (rule of thumb: < 5 MB input).

```javascript
// Small avatar already in memory from multipart upload
const optimized = await ImageEngine.from(uploadBuffer)
  .sanitize({ policy: 'strict' })
  .resize({ width: 200, height: 200, fit: 'cover' })
  .toBuffer('webp', 80);
```

**`from(buffer)` -> `toFile()`** -- stage output to disk from an in-memory source.

Use this when the input arrives as a Buffer but the output should go to disk (e.g. writing processed uploads to local storage or a mounted volume).

```javascript
// Write processed upload to permanent storage
await ImageEngine.from(uploadBuffer)
  .sanitize({ policy: 'strict' })
  .resize({ width: 1024, fit: 'inside' })
  .toFile('/data/uploads/processed.webp', 'webp', 80);
```

**Rule of thumb:** if the image is on disk and the output goes to disk or object storage, use `fromPath()` -> `toFile()`. If the image is larger than ~5 MB and arrives as a Buffer, write it to a temp file first and use `fromPath()`.

---

## Serverless Deployment

lazy-image's single static binary (no system libvips) and bounded-memory design make it well-suited for constrained environments.

### AWS Lambda

```javascript
const os = require('os');
const path = require('path');
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

exports.handler = async (event) => {
  const inputPath = path.join(os.tmpdir(), 'input.jpg');
  const outputPath = path.join(os.tmpdir(), 'output.webp');

  // 1. Download from S3
  await downloadFromS3(event.bucket, event.key, inputPath);

  // 2. Optimize -- file-to-file keeps pixel data off V8 heap
  const { bytesWritten, metrics } = await ImageEngine.fromPath(inputPath)
    .resize({ width: 1600, fit: 'inside' })
    .toFileWithMetrics(outputPath, 'webp', 80);

  // 3. Upload result
  await uploadToS3(outputPath, event.outputKey);

  return {
    statusCode: 200,
    body: JSON.stringify({
      bytes: metrics.bytesOut,
      savings: `${((1 - metrics.bytesOut / metrics.bytesIn) * 100).toFixed(1)}%`,
      ms: metrics.totalMs,
    }),
  };
};
```

Key sizing guidance:

| Setting | Recommendation |
|---------|---------------|
| Memory | 512 MB minimum; 1024 MB for images > 5 MP |
| Timeout | 30 s for single images; 120 s for batch |
| Ephemeral storage | Default 512 MB is fine for single images; increase for batch |
| Architecture | `arm64` (Graviton) recommended -- lower cost, lazy-image ships `linux-arm64-gnu` |

### Google Cloud Functions / Cloud Run

Same pattern as Lambda. Use `/tmp` for scratch files. For Cloud Run, set `--memory 512Mi` and `--concurrency 1` per container instance if processing large images. lazy-image's internal memory semaphore prevents concurrent operations from exceeding cgroup limits.

### Vercel / Netlify Edge Functions and Cloudflare Workers

These environments run in V8 isolates and **do not support the native NAPI binary** that lazy-image ships. The published `@alberteinshutoin/lazy-image` package requires a Node.js runtime (Vercel Node Functions, Netlify Node Functions, AWS Lambda, Cloud Run, etc.).

For Edge / Workers, process images in a background Node-runtime function and serve the results from a CDN. Wasm support for V8-isolate runtimes is tracked under [#87](https://github.com/albert-einshutoin/lazy-image/issues/87).

### Cold start optimization

- Binary size is ~5-9 MB per platform (vs ~15-21 MB for sharp), reducing cold start time
- No native system dependencies to install -- `npm ci` is sufficient
- Memory auto-detection reads cgroup limits so concurrency self-tunes inside containers
- Keep `node_modules` lean: `npm ci --omit=dev` in your Docker/Lambda layer

### Docker / Kubernetes

```dockerfile
FROM node:20-slim
# No extra apt-get or system libraries needed
WORKDIR /app
COPY package*.json ./
RUN npm ci --omit=dev
COPY . .
CMD ["node", "server.js"]
```

For memory-constrained pods (256-512 MB), lazy-image's memory semaphore automatically limits concurrent operations. No tuning required.

For high-throughput deployments:

```yaml
# k8s resource recommendation
resources:
  requests:
    memory: "512Mi"
    cpu: "500m"
  limits:
    memory: "1Gi"
    cpu: "2000m"
```

---

## Upload Sanitization Pipeline

### Basic upload flow

Combine Image Firewall with format normalization for a complete upload sanitization flow.

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

async function processUpload(inputPath, outputDir, userId) {
  const outputPath = `${outputDir}/${userId}-avatar.webp`;

  const { bytesWritten, metrics } = await ImageEngine.fromPath(inputPath)
    .sanitize({ policy: 'strict' })           // reject oversized / malformed
    .resize({ width: 512, height: 512, fit: 'cover' })
    .keepMetadata({ icc: true, stripGps: true }) // preserve color, strip location
    .toFileWithMetrics(outputPath, 'webp', 80);

  return {
    path: outputPath,
    bytes: metrics.bytesOut,
    compressionRatio: metrics.compressionRatio,
    ms: metrics.totalMs,
  };
}
```

### Multi-tier upload pipeline

For platforms accepting different image types (avatars, cover photos, gallery uploads), define per-tier processing:

```javascript
const UPLOAD_TIERS = {
  avatar: {
    maxWidth: 512,
    maxHeight: 512,
    fit: 'cover',
    format: 'webp',
    quality: 80,
    policy: 'strict',       // aggressive limits for untrusted input
  },
  cover: {
    maxWidth: 1920,
    maxHeight: 1080,
    fit: 'inside',
    format: 'jpeg',
    quality: 85,
    policy: 'strict',
  },
  gallery: {
    maxWidth: 2400,
    fit: 'inside',
    format: 'webp',
    quality: 80,
    policy: 'lenient',      // more permissive for large uploads
  },
};

async function processUploadByTier(inputPath, outputDir, tier, fileId) {
  const config = UPLOAD_TIERS[tier];
  if (!config) throw new Error(`Unknown upload tier: ${tier}`);

  const outputPath = `${outputDir}/${fileId}.${config.format === 'jpeg' ? 'jpg' : config.format}`;

  const resizeOpts = { width: config.maxWidth, fit: config.fit };
  if (config.maxHeight) resizeOpts.height = config.maxHeight;

  const { bytesWritten, metrics } = await ImageEngine.fromPath(inputPath)
    .sanitize({ policy: config.policy })
    .resize(resizeOpts)
    .keepMetadata({ icc: true, stripGps: true })
    .toFileWithMetrics(outputPath, config.format, config.quality);

  return { outputPath, metrics };
}
```

### Handling upload errors

Upload pipelines must distinguish user errors (bad input) from system errors:

```javascript
const { getErrorCategory, ErrorCategory } = require('@alberteinshutoin/lazy-image');

async function safeProcessUpload(inputPath, outputDir, userId) {
  try {
    return await processUpload(inputPath, outputDir, userId);
  } catch (err) {
    const category = getErrorCategory(err);

    if (category === ErrorCategory.UserError || category === ErrorCategory.CodecError) {
      // Bad input -- return 400 to the client
      return { error: 'invalid_image', message: err.message, code: err.errorCode };
    }

    if (category === ErrorCategory.ResourceLimit) {
      // Image too large or system under pressure -- return 413 or retry later
      return { error: 'image_too_large', hint: err.recoveryHint };
    }

    // InternalBug or unknown -- log and return 500
    console.error('Unexpected image processing error', { code: err.errorCode, msg: err.message });
    throw err;
  }
}
```

### Buffer-based uploads (Express / Fastify multipart)

When the upload body is already in memory (e.g. from `multer` or `@fastify/multipart`), write to a temp file first for large images:

```javascript
const fs = require('fs/promises');
const os = require('os');
const path = require('path');

async function processMultipartUpload(buffer, outputDir, userId) {
  // For images > 5 MB, stage to disk to avoid V8 heap pressure
  if (buffer.length > 5 * 1024 * 1024) {
    const tmpPath = path.join(os.tmpdir(), `upload-${Date.now()}.tmp`);
    try {
      await fs.writeFile(tmpPath, buffer);
      return await processUpload(tmpPath, outputDir, userId);
    } finally {
      await fs.unlink(tmpPath).catch(() => {});
    }
  }

  // Small images: process directly from buffer
  const outputPath = `${outputDir}/${userId}-avatar.webp`;
  const { bytesWritten, metrics } = await ImageEngine.from(buffer)
    .sanitize({ policy: 'strict' })
    .resize({ width: 512, height: 512, fit: 'cover' })
    .keepMetadata({ icc: true, stripGps: true })
    .toFileWithMetrics(outputPath, 'webp', 80);

  return { path: outputPath, bytes: metrics.bytesOut, ms: metrics.totalMs };
}
```

### Batch uploads

For batch uploads, use `processBatch()`:

```javascript
const results = await ImageEngine.processBatch(
  files.map(f => f.path),   // Array<string> of input file paths
  outputDir,
  { format: 'webp', quality: 80 },
);
// Each result: { success, source, outputPath?, error?, errorCode?, errorCategory? }
```

---

## Observability

### Metrics from every operation

Use `toFileWithMetrics()` or `toBufferWithMetrics()` to capture per-image telemetry:

```javascript
const { data, metrics } = await engine.toBufferWithMetrics('jpeg', 80);
// metrics: { version, decodeMs, opsMs, encodeMs, totalMs,
//            peakRss, cpuTime, processingTime,
//            bytesIn, bytesOut, compressionRatio,
//            formatIn, formatOut, iccPreserved,
//            metadataStripped, policyViolations }
```

### Structured logging pattern

Emit structured JSON logs that integrate with your log aggregator (Datadog, CloudWatch, Loki, etc.):

```javascript
const { ImageEngine, getErrorCategory } = require('@alberteinshutoin/lazy-image');

async function optimizeWithLogging(input, output, format, quality) {
  const start = Date.now();
  try {
    const { bytesWritten, metrics } = await ImageEngine.fromPath(input)
      .resize({ width: 1600, fit: 'inside' })
      .toFileWithMetrics(output, format, quality);

    console.log(JSON.stringify({
      event: 'image_optimized',
      level: 'info',
      format_in: metrics.formatIn,
      format_out: metrics.formatOut,
      bytes_in: metrics.bytesIn,
      bytes_out: metrics.bytesOut,
      savings_pct: ((1 - metrics.bytesOut / metrics.bytesIn) * 100).toFixed(1),
      compression_ratio: metrics.compressionRatio,
      duration_ms: metrics.totalMs,
      decode_ms: metrics.decodeMs,
      encode_ms: metrics.encodeMs,
      ops_ms: metrics.opsMs,
      icc_preserved: metrics.iccPreserved,
      metadata_stripped: metrics.metadataStripped,
    }));

    return { bytesWritten, metrics };
  } catch (err) {
    console.log(JSON.stringify({
      event: 'image_error',
      level: 'error',
      error_code: err.errorCode,
      error_category: getErrorCategory(err),
      message: err.message,
      recovery_hint: err.recoveryHint,
      duration_ms: Date.now() - start,
    }));
    throw err;
  }
}
```

### Key metrics to track

| Metric | Source | Alert threshold |
|--------|--------|----------------|
| `totalMs` | `ProcessingMetrics` | P99 > 10 s |
| `bytesOut / bytesIn` | `compressionRatio` | > 1.0 (output larger than input) |
| Error rate by category | `getErrorCategory()` | `InternalBug` > 0 |
| `decodeMs` | `ProcessingMetrics` | P99 > 5 s (may indicate corrupted input) |
| `encodeMs` | `ProcessingMetrics` | P99 > 8 s (AVIF can be slow for large images) |
| `policyViolations` | `ProcessingMetrics` | Non-empty array (firewall triggered) |

### Error categories for alerting

```javascript
const { getErrorCategory, ErrorCategory } = require('@alberteinshutoin/lazy-image');

try {
  await engine.toFile(output, 'jpeg', 80);
} catch (err) {
  const category = getErrorCategory(err);
  if (category === ErrorCategory.InternalBug) {
    // Page oncall -- this should not happen
    alerting.critical('lazy-image internal error', { code: err.errorCode, msg: err.message });
  } else if (category === ErrorCategory.ResourceLimit) {
    // Log and skip -- image too large or system under pressure
    alerting.warn('resource limit', { code: err.errorCode, hint: err.recoveryHint });
  }
  // UserError / CodecError: log and return 400 to caller
}
```

### Batch processing metrics

`processBatchWithMetrics()` returns a summary alongside per-item metrics:

```javascript
const { items, summary } = await ImageEngine.processBatchWithMetrics(
  inputs, outputDir, { format: 'webp', quality: 80 },
);

console.log(JSON.stringify({
  event: 'batch_complete',
  total: summary.totalItems,
  success: summary.successfulItems,
  failed: summary.failedItems,
  concurrency: summary.effectiveConcurrency,
  auto_concurrency: summary.autoConcurrency,
  total_bytes_in: summary.totalBytesIn,
  total_bytes_out: summary.totalBytesOut,
  wall_ms: summary.totalWallMs,
  cpu_time_s: summary.totalCpuTime,
}));
```

### Dashboard recommendations

If you use Prometheus/Grafana, Datadog, or similar:

1. **Histogram**: `lazy_image_duration_ms` (labels: `format_out`, `operation`)
2. **Counter**: `lazy_image_errors_total` (labels: `error_category`, `error_code`)
3. **Gauge**: `lazy_image_compression_ratio` (labels: `format_out`)
4. **Counter**: `lazy_image_bytes_saved_total` (computed: `bytesIn - bytesOut`)
5. **Histogram**: `lazy_image_output_bytes` (labels: `format_out`)

---

## Streaming Note

`createStreamingPipeline()` is available for **disk-backed bounded-memory processing**. It is useful when you need stream-shaped I/O but still want lazy-image's file-based processing model.

It is **not** equivalent to sharp's true streaming transforms:
- input is staged to disk first
- output is produced after file processing completes
- latency includes temp-file I/O

Use `createStreamingPipeline()` when integrating with stream-oriented frameworks (e.g. piping from an HTTP request to a response) but understand that it does not provide true chunked processing.

## Operational Default

If you are unsure where to start, use:

```javascript
ImageEngine.fromPath(input).resize(...).toFile(output, format, quality)
```

That path best matches the library's design goals: smaller JPEG outputs in simple canonical benchmarks, bounded memory, and predictable server behavior.
