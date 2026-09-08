# Compatibility Matrix (lazy-image vs sharp)

## Positioning

lazy-image is an **opinionated web image optimization engine**. It is **not** a
drop-in replacement for sharp. The API is intentionally smaller and focuses on:

- Smaller JPEG files in canonical benchmarks
- Predictable, safe behavior (strict limits and error taxonomy)
- Low memory usage for server workloads

If you need broad image editing features or a sharp-compatible API, use sharp.

## Runtime Support

The native and Wasm packages require Node.js 22 or newer. CI covers Node.js 22
and 24; Node.js 18 and 20 are unsupported because they are end-of-life.

## Supported Formats

- **Input**: jpeg/jpg, png, webp
- **Output**: jpeg/jpg, png, webp, avif (optional at build time via Cargo feature `avif`)

You can query at runtime with `supportedInputFormats()` and
`supportedOutputFormats()`.

## Feature Matrix

| Capability | lazy-image | sharp |
| :--- | :--- | :--- |
| Drop-in API compatibility | ❌ | ✅ |
| Resize / crop / rotate / flip | ✅ | ✅ |
| Grayscale / brightness / contrast | ✅ | ✅ |
| Compositing / overlays | ❌ | ✅ |
| Rich filters (blur/sharpen/tint/etc) | ❌ | ✅ |
| Animated images (GIF/WebP) | ❌ | ✅ |
| Streaming pipeline | Disk-backed bounded-memory pipeline (`createStreamingPipeline()`), not true streaming transforms | ✅ |
| Metadata | ICC + EXIF (GPS auto-strip, XMP unsupported) | ✅ (EXIF/XMP/etc) |
| AVIF encoding | ✅ (supported; benchmark size/speed per workload) | ✅ |

## Non-goals

- Full sharp API parity
- High-level image editing workflows
- Animation support

## When to Choose lazy-image

- You want **smaller JPEG files** and are OK with a smaller API surface.
- You care about **JPEG size optimization**, metadata safety, and bounded-memory file-path processing.
- You want clear error taxonomy and strict limits for user uploads.
- You are fine with a disk-backed bounded-memory pipeline instead of true chunk transform streaming.

For exact metadata behavior, see [METADATA_SUPPORT.md](./METADATA_SUPPORT.md).
