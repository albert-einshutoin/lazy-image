# lazy-image 🦀

<img width="192" height="192" alt="lazy-image" src="https://github.com/user-attachments/assets/239496c7-ad7f-4649-b130-8ed0a65481f7" />

Build verified web-image artifacts in your Node.js upload or build pipeline.
lazy-image combines policy-driven responsive output, metadata checks and a
manifest, then commits the verified set to a local directory. Powered by Rust,
it also provides a lazy API for individual image optimization.

[![npm version](https://badge.fury.io/js/@alberteinshutoin%2Flazy-image.svg)](https://www.npmjs.com/package/@alberteinshutoin/lazy-image)
[![Node.js CI](https://github.com/albert-einshutoin/lazy-image/actions/workflows/CI.yml/badge.svg)](https://github.com/albert-einshutoin/lazy-image/actions/workflows/CI.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Node.js 22+ · prebuilt for macOS arm64/x64, Linux x64 GNU/musl and arm64 GNU,
and Windows x64 · JPEG/PNG/WebP input · JPEG/PNG/WebP/AVIF output

[日本語](./README.ja.md) · [Documentation](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/README.md) · [API](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/API.md) · [Examples](https://github.com/albert-einshutoin/lazy-image/tree/main/examples/) · [Performance](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/PERFORMANCE.md)

## Choose your workflow

| You need | Start with |
|---|---|
| A verified public image set from one local upload | [`compileImage()` or CLI](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/ADOPTION_GUIDE.md#start-with-a-public-artifact-set) |
| One optimized file or Buffer | [Quick start](#quick-start) and [`ImageEngine`](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/API.md) |
| Browser/Edge upload preflight | [Separate Wasm package](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/WASM_PACKAGE_API.md) |
| Product fit and alternatives | [Product direction](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/PROJECT_PHILOSOPHY.md) · [Competitor analysis](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/COMPETITIVE_ANALYSIS.md) |

You own upload admission, job execution, storage and delivery. The compiler's
commit is a local filesystem operation; it does not upload to a CDN or object store.

## Install

```bash
npm install @alberteinshutoin/lazy-image
```

Prebuilt native binaries are installed automatically on supported platforms.

## Quick Start

```javascript
import { ImageEngine } from '@alberteinshutoin/lazy-image';

const image = await ImageEngine.fromPathAsync('input.png');
const bytesWritten = await image
  .resize({ width: 800, fit: 'inside' })
  .toFile('output.jpg', 'jpeg', 80);

console.log(`Wrote ${bytesWritten} bytes`);
```

Use `fromPathAsync()` in servers so file setup does not block the Node.js event
loop. CommonJS is also supported:

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');
```

## Common tasks

### Buffer to WebP

```javascript
import { readFile } from 'node:fs/promises';
import { ImageEngine } from '@alberteinshutoin/lazy-image';

const input = await readFile('input.jpg');
const output = await ImageEngine.from(input)
  .resize({ width: 1200 })
  .toBuffer('webp', 80);
```

### Sanitize an upload

```javascript
const output = await ImageEngine.from(uploadBuffer)
  .sanitize({ policy: 'public-upload' })
  .resize({ width: 1600, height: 1600, fit: 'inside' })
  .toBuffer('jpeg', 85);
```

The `public-upload` policy enforces resource limits and strips EXIF, GPS, and
XMP while preserving only validated ICC profiles.

For a complete, fail-closed artifact set, use the transactional compiler. It
creates library-owned responsive filenames and `manifest.json` in private
staging, then publishes the directory only after verification:

```javascript
const { compileImage } = require('@alberteinshutoin/lazy-image');

const manifest = await compileImage({
  inputPath: '/srv/uploads/image.bin',
  outputDir: '/srv/public/images/v1',
  policy: { widths: [320, 640], formats: ['webp'], placeholder: true },
});
```

Existing output directories are rejected and full artifact bytes are never
returned as Node.js buffers. The output parent must be trusted, without concurrent
replacement, and staging must share its filesystem. See the
[compiler contract](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/API.md#transactional-public-upload-compilation).

### CLI artifact compilation

Create a policy JSON file, then compile one input into a new output directory:

```json
{"widths":[320,640],"formats":["webp"],"placeholder":true}
```

```bash
npx @alberteinshutoin/lazy-image compile input.jpg \
  --out-dir public/images/v1 \
  --policy policy.json
```

On success, stdout contains only the generated manifest JSON. Diagnostics go to
stderr; exit 0 means success, exit 2 means invalid CLI arguments or policy JSON,
and exit 1 means compiler failure. Failed compilations do not publish the set;
see [failure handling](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/ADOPTION_GUIDE.md#start-with-a-public-artifact-set) for cleanup diagnostics.

### Inspect without decoding

```javascript
import { inspectFile } from '@alberteinshutoin/lazy-image';

const { width, height, format } = inspectFile('input.jpg');
```

## Main API

```text
ImageEngine.from(buffer)
await ImageEngine.fromPathAsync(path)

.resize({ width?, height?, fit? })
.crop(x, y, width, height)
.rotate(90 | 180 | 270)
.flipH() / .flipV() / .grayscale()
.sanitize({ policy })

await .toFile(path, format, quality?)
await .toBuffer(format, quality?)
await .encode({ format, quality?, preset?, metrics? })
await .toResponsiveSet({ widths, format, quality? })
await .toFilesResponsive('path-{width}.webp', { widths, format, quality? }, srcsetPattern?)
await .toPlaceholder({ size?, format?, quality? })
```

Operations are queued and evaluated once when an output method runs. Use
`clone()` when producing multiple variants from one source. See the
[full API reference](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/API.md) for batch processing, presets, metrics,
streaming, metadata controls, responsive output helpers, and error codes.

## When to use it

Choose lazy-image when you want policy-driven artifacts and a manifest in an
existing upload/build job, or its file-oriented optimization API. Metadata
stripping and resource limits are important defaults, not exclusive advantages.
Use sharp for broader editing/format needs; consider an HTTP image server or
managed image service when on-demand transformation and delivery are the main job.
See the [competitor analysis](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/COMPETITIVE_ANALYSIS.md) for the distinctions
and [performance evidence](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/PERFORMANCE.md) for scoped codec comparisons.

## Safety and limits

- Metadata is stripped by default. Use `keepMetadata()` only when needed.
- File inputs up to 256 MB use Rust-owned memory; larger files may use mmap.
  Do not modify or delete an mmap-backed source while processing it.
- Rotation supports 90°, 180°, and 270°.
- 16-bit images are converted to 8-bit.
- Animated GIF/APNG, drawing, and heavy filters are out of scope.

Details: [metadata](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/METADATA_SUPPORT.md) · [file I/O](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/ZERO_COPY.md) ·
[thread model](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/THREAD_MODEL.md) · [errors](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/ERROR_CODES.md)

## Browser and Edge

Use the separate Wasm package for browser and V8-isolate upload preflight:

```bash
npm install @alberteinshutoin/lazy-image-wasm
```

See the [Wasm package guide](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/WASM_PACKAGE_API.md). Validate bundle size
and latency in the target runtime before production use.

## Development

```bash
npm install
npm run build
npm test
```

See [contributor documentation](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/DEVELOPMENT.md) for build requirements, contracts and verification.

## License

MIT. Built with mozjpeg, libwebp, libavif, fast_image_resize, img-parts, and
NAPI-RS.
