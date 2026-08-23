# lazy-image 🦀

<img width="192" height="192" alt="lazy-image" src="https://github.com/user-attachments/assets/239496c7-ad7f-4649-b130-8ed0a65481f7" />

Image optimization for Node.js, powered by Rust. lazy-image prioritizes smaller
web images, bounded memory, and safe defaults over encoding speed or a broad
editing API.

[![npm version](https://badge.fury.io/js/@alberteinshutoin%2Flazy-image.svg)](https://www.npmjs.com/package/@alberteinshutoin/lazy-image)
[![Node.js CI](https://github.com/albert-einshutoin/lazy-image/actions/workflows/CI.yml/badge.svg)](https://github.com/albert-einshutoin/lazy-image/actions/workflows/CI.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Node.js 22+ · prebuilt for macOS arm64/x64, Linux x64 GNU/musl and arm64 GNU,
and Windows x64 · JPEG/PNG/WebP input · JPEG/PNG/WebP/AVIF output

[日本語](./README.ja.md) · [API](./docs/API.md) · [Examples](./examples/) · [Performance](./docs/PERFORMANCE.md)

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

## Buffer to WebP

```javascript
import { readFile } from 'node:fs/promises';
import { ImageEngine } from '@alberteinshutoin/lazy-image';

const input = await readFile('input.jpg');
const output = await ImageEngine.from(input)
  .resize({ width: 1200 })
  .toBuffer('webp', 80);
```

## Sanitize an upload

```javascript
const output = await ImageEngine.from(uploadBuffer)
  .sanitize({ policy: 'public-upload' })
  .resize({ width: 1600, height: 1600, fit: 'inside' })
  .toBuffer('jpeg', 85);
```

The `public-upload` policy enforces resource limits and strips EXIF, GPS, and
XMP while preserving only validated ICC profiles.

## Inspect without decoding

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
```

Operations are queued and evaluated once when an output method runs. Use
`clone()` when producing multiple variants from one source. See the
[full API reference](./docs/API.md) for batch processing, presets, metrics,
streaming, metadata controls, and error codes.

## When to use it

Choose lazy-image when output size, predictable memory use, or upload safety is
more important than raw throughput. Choose sharp or ImageMagick when you need
maximum speed, drawing, compositing, animation, SVG, TIFF, or a drop-in sharp
API. See the [benchmark methodology](./docs/PERFORMANCE.md) before comparing
codecs or quality settings.

## Safety and limits

- Metadata is stripped by default. Use `keepMetadata()` only when needed.
- File inputs up to 256 MB use Rust-owned memory; larger files may use mmap.
  Do not modify or delete an mmap-backed source while processing it.
- Rotation supports 90°, 180°, and 270°.
- 16-bit images are converted to 8-bit.
- Animated GIF/APNG, drawing, and heavy filters are out of scope.

Details: [metadata](./docs/METADATA_SUPPORT.md) · [file I/O](./docs/ZERO_COPY.md) ·
[thread model](./docs/THREAD_MODEL.md) · [errors](./docs/ERROR_CODES.md)

## Browser and Edge

Use the separate Wasm package for browser and V8-isolate upload preflight:

```bash
npm install @alberteinshutoin/lazy-image-wasm
```

See the [Wasm package guide](./docs/WASM_PACKAGE_API.md). Validate bundle size
and latency in the target runtime before production use.

## Development

```bash
npm install
npm run build
npm test
```

See [CONTRIBUTING.md](./CONTRIBUTING.md) for build requirements and workflow.

## License

MIT. Built with mozjpeg, libwebp, libavif, fast_image_resize, img-parts, and
NAPI-RS.
