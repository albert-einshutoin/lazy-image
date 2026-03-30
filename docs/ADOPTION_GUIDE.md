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

## Serverless Deployment

lazy-image's single static binary (no system libvips) and bounded-memory design make it well-suited for constrained environments.

### AWS Lambda / Google Cloud Functions

```javascript
// handler.js — Lambda-friendly: file-to-file avoids heap pressure
const os = require('os');
const path = require('path');
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

exports.handler = async (event) => {
  const inputPath = '/tmp/input.jpg';   // download from S3/GCS first
  const outputPath = '/tmp/output.webp';

  await ImageEngine.fromPath(inputPath)
    .resize({ width: 1600, fit: 'inside' })
    .toFile(outputPath, 'webp', 80);

  // upload outputPath back to S3/GCS
};
```

Key points:
- Use `fromPath` / `toFile` — avoids loading images into V8 heap
- Lambda `/tmp` has 512 MB–10 GB of ephemeral storage; use it as scratch space
- Binary size is ~5–9 MB per platform (vs ~15–21 MB for sharp), reducing cold starts
- Memory auto-detection reads cgroup limits so concurrency self-tunes inside containers

### Docker / Kubernetes

```dockerfile
FROM node:20-slim
# No extra apt-get or system libraries needed
COPY package*.json ./
RUN npm ci --omit=dev
COPY . .
```

For 512 MB containers, lazy-image's memory semaphore automatically limits concurrent operations. No tuning required.

## Upload Pipeline

Combine Image Firewall with format normalization for a complete upload sanitization flow.

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

async function processUpload(inputPath, outputDir, userId) {
  const outputPath = `${outputDir}/${userId}-avatar.webp`;

  const { data, metrics } = await ImageEngine.fromPath(inputPath)
    .sanitize({ policy: 'strict' })           // reject oversized / malformed
    .resize({ width: 512, height: 512, fit: 'cover' })
    .keepMetadata({ icc: true, stripGps: true }) // preserve color, strip location
    .toFileWithMetrics(outputPath, 'webp', 80);

  return {
    path: outputPath,
    width: metrics.widthOut,
    height: metrics.heightOut,
    bytes: metrics.sizeOut,
    ms: metrics.totalMs,
  };
}
```

For batch uploads, use `processBatch()`:

```javascript
const results = await ImageEngine.processBatch(
  files.map(f => ({ input: f.path, resize: { width: 1024, fit: 'inside' } })),
  outputDir,
  { format: 'webp', quality: 80 },
);
// Each result: { success, output, error?, errorCode?, errorCategory? }
```

## Observability

### Metrics from every operation

Use `toFileWithMetrics()` or `toBufferWithMetrics()` to capture per-image telemetry:

```javascript
const { data, metrics } = await engine.toBufferWithMetrics('jpeg', 80);
// metrics: { widthIn, heightIn, widthOut, heightOut,
//            formatIn, formatOut, sizeIn, sizeOut, totalMs }
```

### Production logging pattern

```javascript
async function optimizeWithLogging(input, output, format, quality) {
  const { data, metrics } = await ImageEngine.fromPath(input)
    .resize({ width: 1600, fit: 'inside' })
    .toFileWithMetrics(output, format, quality);

  console.log(JSON.stringify({
    event: 'image_optimized',
    input: { w: metrics.widthIn, h: metrics.heightIn, bytes: metrics.sizeIn, format: metrics.formatIn },
    output: { w: metrics.widthOut, h: metrics.heightOut, bytes: metrics.sizeOut, format: metrics.formatOut },
    savings_pct: ((1 - metrics.sizeOut / metrics.sizeIn) * 100).toFixed(1),
    duration_ms: metrics.totalMs,
  }));
}
```

### Error categories for alerting

```javascript
const { getErrorCategory, ErrorCategory } = require('@alberteinshutoin/lazy-image');

try {
  await engine.toFile(output, 'jpeg', 80);
} catch (err) {
  const category = getErrorCategory(err);
  if (category === ErrorCategory.InternalBug) {
    // Page oncall — this should not happen
    alerting.critical('lazy-image internal error', { code: err.errorCode, msg: err.message });
  } else if (category === ErrorCategory.ResourceLimit) {
    // Log and skip — image too large or system under pressure
    alerting.warn('resource limit', { code: err.errorCode, hint: err.recoveryHint });
  }
  // UserError / CodecError: log and return 400 to caller
}
```

## File-to-file vs Buffer Paths

| Path | Heap usage | Best for |
|------|-----------|----------|
| `fromPath()` → `toFile()` | Minimal (mmap) | Production servers, large images, serverless |
| `fromPath()` → `toBuffer()` | Output buffer in V8 | When you need the buffer (e.g. HTTP response) |
| `from(buffer)` → `toBuffer()` | Full input + output in V8 | Small images already in memory |
| `from(buffer)` → `toFile()` | Input in V8 | Writing from an in-memory source |

**Rule of thumb:** if the image is on disk and the output goes to disk or object storage, use `fromPath()` → `toFile()`.

## Streaming Note

`createStreamingPipeline()` is available for **disk-backed bounded-memory processing**. It is useful when you need stream-shaped I/O but still want lazy-image's file-based processing model.

It is **not** equivalent to sharp's true streaming transforms:
- input is staged to disk first
- output is produced after file processing completes
- latency includes temp-file I/O

## Operational Default

If you are unsure where to start, use:

```javascript
ImageEngine.fromPath(input).resize(...).toFile(output, format, quality)
```

That path best matches the library's design goals: smaller outputs, bounded memory, and predictable server behavior.
