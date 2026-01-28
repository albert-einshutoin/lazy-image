# Migration Guide: sharp → lazy-image

This guide helps teams port common sharp workflows to **lazy-image**. The focus is on web-image optimization use cases; lazy-image is not a drop-in replacement for sharp's full surface.

## Quick Differences
- Web optimization first: smaller JPEG/AVIF files and lower memory use; narrower feature set than sharp.
- Metadata defaults: both libraries strip most metadata; lazy-image additionally auto-strips GPS. Use `.keepMetadata(...)` (lazy-image) or `.withMetadata()` (sharp) to retain data.
- Formats: lazy-image inputs jpeg/png/webp; outputs jpeg/png/webp/avif. Use sharp if you need TIFF, GIF, HEIF, or multi-page inputs.
- Streaming: lazy-image lacks sharp's true streaming transforms; `createStreamingPipeline()` stages to disk for bounded-memory processing.

## API Mapping
| sharp | lazy-image | Notes |
|-------|-----------|-------|
| `sharp(inputBuffer)` | `ImageEngine.from(buffer)` | Loads from a JS buffer (copies into V8 heap). |
| `sharp('input.jpg')` | `ImageEngine.fromPath('input.jpg')` | Zero-copy path; recommended for servers. |
| `.resize(800).jpeg({ quality: 80 }).toBuffer()` | `.resize(800).toBuffer('jpeg', 80)` | Default fit is `inside` in both. |
| `.resize(800, 600, { fit: 'cover' })` | `.resize(800, 600, 'cover').toBuffer('jpeg')` | `cover` crops to fill. |
| `.extract({ left: 10, top: 20, width: 300, height: 200 })` | `.crop(10, 20, 300, 200)` | Same origin (top-left). |
| `.rotate(90)` | `.rotate(90)` | Only 90/180/270 are allowed in lazy-image. |
| `.flip().flop()` | `.flipV().flipH()` | `flip` = vertical, `flop` = horizontal. |
| `.grayscale()` | `.grayscale()` | Both convert to grayscale. |
| `.modulate({ brightness: 1.1 })` | `.brightness(10)` | lazy-image uses -100..100; 10 ≈ +10%. |
| `.modulate({ saturation: 0.9 })` | `.contrast(-10)` | Saturation is not exposed; contrast is the closest control. |
| `.withMetadata({ icc, exif })` | `.keepMetadata({ icc: true, exif: true, stripGps?: boolean })` | Both are opt-in; lazy-image keeps stripping GPS unless `stripGps: false`. |
| `.toFile('out.webp')` | `.toFile('out.webp', 'webp', 80)` | Writes directly without buffering in JS. |
| `.toBuffer({ resolveWithObject: true })` | `.toBufferWithMetrics('jpeg', 85)` | Adds timing/size metrics for observability. |
| `pipeline.clone()` | `.clone()` | Duplicate pipeline for multi-output. |

## Unsupported or Partially Supported Features
- Compositing / overlays / tint / blur / sharpen (use sharp or ImageMagick for these).
- Animated images (GIF/WebP multi-frame) and multi-page inputs.
- Broad input formats (TIFF, HEIF, PDF, SVG, RAW) — use sharp when needed.
- True streaming transforms; lazy-image only offers disk-backed `createStreamingPipeline()` for bounded memory.

## Migration Script Example
```javascript
// Before (sharp)
const output = await sharp(input)
  .resize(800)
  .webp({ quality: 80 })
  .toBuffer();

// After (lazy-image)
const output = await ImageEngine.from(input)
  .resize(800)
  .toBuffer('webp', 80);
```

## Performance Comparison (when to switch)
- AVIF: lazy-image is ~6× faster than sharp for large PNG→AVIF conversions and yields ~38% smaller AVIF files (see README benchmarks).
- JPEG: expect 20–25% smaller files; encoding is slower because compression is prioritized over throughput.
- WebP: similar or slightly slower throughput; use if you want the safer defaults and metadata stripping.
- Latency-sensitive or filter-heavy workloads still favor sharp; build-time optimization and batch processing favor lazy-image.

## FAQ
**Q. How are ICC profiles handled?**  
lazy-image strips metadata by default for safety; call `.keepMetadata({ icc: true })` to retain profiles. AVIF ICC is preserved on v0.9.x (libavif-sys). sharp also strips metadata by default—use `.withMetadata()` to preserve ICC/EXIF during transforms.

**Q. What about EXIF/GPS and other metadata?**  
lazy-image removes EXIF by default and always strips GPS unless `stripGps: false` is set. sharp drops EXIF unless you opt into `.withMetadata()`; scrub GPS manually if you need parity with lazy-image defaults.

**Q. Does lazy-image auto-orient like sharp?**  
Yes. Both respect EXIF orientation and normalize the tag after rotation. lazy-image resets Orientation to 1 to avoid double-rotation in downstream viewers.

**Q. How should I pick quality settings?**  
Start with JPEG 85, WebP 80, AVIF 60 (lazy-image defaults). For parity with sharp defaults, pass the same numbers explicitly: `.toBuffer('jpeg', 80)` mirrors `jpeg({ quality: 80 })`.

**Q. Any migration pitfalls?**  
Watch for missing features (compositing/animation), stricter limits, and the default metadata strip. Prefer `fromPath()` to keep memory low in server pipelines.
