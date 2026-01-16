# Compatibility Matrix (lazy-image vs sharp)

## Positioning

lazy-image is an **opinionated web image optimization engine**. It is **not** a
drop-in replacement for sharp. The API is intentionally smaller and focuses on:

- Smaller file sizes (especially JPEG/AVIF)
- Predictable, safe behavior (strict limits and error taxonomy)
- Low memory usage for server workloads

If you need broad image editing features or a sharp-compatible API, use sharp.

## Supported Formats

- **Input**: jpeg/jpg, png, webp
- **Output**: jpeg/jpg, png, webp, avif

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
| Streaming pipeline | ❌ | ✅ |
| Metadata | ICC only | ✅ (EXIF/XMP/etc) |
| AVIF encoding | ✅ (focus area) | ✅ |

## Non-goals

- Full sharp API parity
- High-level image editing workflows
- Animation support

## When to Choose lazy-image

- You want **smaller files** and are OK with a smaller API surface.
- You care about **AVIF speed** and **JPEG size optimization**.
- You want clear error taxonomy and strict limits for user uploads.
