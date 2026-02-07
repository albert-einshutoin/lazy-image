# lazy-image 🦀

<img width="256" height="256" alt="image" src="https://github.com/user-attachments/assets/239496c7-ad7f-4649-b130-8ed0a65481f7" />

> **Web-optimized image processing engine for Node.js.** Smaller files than sharp. Memory-safe Rust core.
>
> - **Not** a drop-in replacement for sharp — use sharp if you need its full API.
> - **Security-first**: Metadata stripped by default; `keepMetadata()` to preserve. Zero-copy path: `fromPath()`/`processBatch()` → `toFile()`.
> - Japanese: [README.ja.md](./README.ja.md). **mmap**: Do not modify/delete source files while processing; use a copy or `from(Buffer)` for mutable inputs.

[![npm version](https://badge.fury.io/js/@alberteinshutoin%2Flazy-image.svg)](https://www.npmjs.com/package/@alberteinshutoin/lazy-image)
[![npm downloads](https://img.shields.io/npm/dm/@alberteinshutoin/lazy-image)](https://www.npmjs.com/package/@alberteinshutoin/lazy-image)
[![Node.js CI](https://github.com/albert-einshutoin/lazy-image/actions/workflows/CI.yml/badge.svg)](https://github.com/albert-einshutoin/lazy-image/actions/workflows/CI.yml)
[![codecov](https://codecov.io/gh/albert-einshutoin/lazy-image/branch/main/graph/badge.svg)](https://codecov.io/gh/albert-einshutoin/lazy-image)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)

---

## Quick Start (5 lines)

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

const bytesWritten = await ImageEngine.fromPath('input.png')
  .resize(800)
  .toFile('output.jpg', 'jpeg', 80);

console.log(`Wrote ${bytesWritten} bytes`);
```

---

## When to Use (vs sharp)

| Your priority | Use |
|---------------|-----|
| Smaller files, less memory, serverless, AVIF | **lazy-image** |
| Max throughput, broad formats, drop-in API | **sharp** |

Benchmarks and details: [docs/PERFORMANCE.md](./docs/PERFORMANCE.md). Full compatibility matrix: [docs/COMPATIBILITY.md](./docs/COMPATIBILITY.md).

---

## Installation

```bash
npm install @alberteinshutoin/lazy-image
```

Platform-specific binaries (~6–9 MB per platform) are installed automatically. Build from source: `npm run build`. See [docs/PERFORMANCE.md](./docs/PERFORMANCE.md#serverless) for package size comparison with sharp.

---

## Basic Usage

**Resize and save (recommended: file-to-file)**

```javascript
await ImageEngine.fromPath('photo.jpg')
  .resize(800, null)
  .toFile('thumb.jpg', 'jpeg', 85);
```

**Format conversion (e.g. PNG → WebP/AVIF)**

```javascript
const buffer = await ImageEngine.fromPath('input.png')
  .resize(600, null)
  .toBuffer('webp', 80);
```

**Metadata without decoding**

```javascript
const { inspectFile } = require('@alberteinshutoin/lazy-image');
const meta = inspectFile('input.jpg'); // { width, height, format }
```

More: batch processing, presets, metrics, streaming — [docs/API.md](./docs/API.md).

---

## Documentation

| Topic | Link |
|-------|------|
| **Full API reference** | [docs/API.md](./docs/API.md) |
| **Migration from sharp** | [docs/MIGRATION_FROM_SHARP.md](./docs/MIGRATION_FROM_SHARP.md) |
| **Performance & when to use** | [docs/PERFORMANCE.md](./docs/PERFORMANCE.md) |
| **Architecture & security** | [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) |
| **Troubleshooting** | [docs/TROUBLESHOOTING.md](./docs/TROUBLESHOOTING.md) |
| **Security policy & reporting** | [SECURITY.md](./SECURITY.md) |
| **Roadmap & scope** | [docs/ROADMAP.md](./docs/ROADMAP.md) |
| **Version history** | [docs/VERSION_HISTORY.md](./docs/VERSION_HISTORY.md) |
| **Specification (spec/)** | [spec/pipeline.md](./spec/pipeline.md), [spec/resize.md](./spec/resize.md), [spec/errors.md](./spec/errors.md), [spec/limits.md](./spec/limits.md), [spec/quality.md](./spec/quality.md), [spec/metadata.md](./spec/metadata.md) |
| **Error codes** | [docs/ERROR_CODES.md](./docs/ERROR_CODES.md) |
| **Benchmarks (raw data)** | [docs/TRUE_BENCHMARKS.md](./docs/TRUE_BENCHMARKS.md) |

---

## Features (summary)

AVIF · Smaller JPEG/WebP (mozjpeg, libwebp) · ICC profiles (AVIF in v0.9.x) · EXIF auto-orient · Zero-copy file I/O · Bounded-memory streaming · Fluent API · Rust core (NAPI-RS) · Cross-platform (macOS, Windows, Linux). Design choices and limits: [docs/ROADMAP.md](./docs/ROADMAP.md) and [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md).

---

## Development

```bash
npm install && npm run build
npm test
```

See [CLAUDE.md](./CLAUDE.md) for workflow, test commands, and CI. Benchmark testing: [lazy-image-test](https://github.com/albert-einshutoin/lazy-image-test) Docker environment. Fuzzing: [FUZZING.md](./FUZZING.md).

---

## License

MIT

---

## Credits

[mozjpeg](https://github.com/mozilla/mozjpeg) · [libwebp](https://chromium.googlesource.com/webm/libwebp) · [ravif](https://github.com/kornelski/ravif) · [fast_image_resize](https://github.com/Cykooz/fast_image_resize) · [img-parts](https://github.com/paolobarbolini/img-parts) · [napi-rs](https://napi.rs/)

---

**Ship it.** 🚀
