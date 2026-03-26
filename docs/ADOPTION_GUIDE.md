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
