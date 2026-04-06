# lazy-image 🦀

<img width="256" height="256" alt="image" src="https://github.com/user-attachments/assets/239496c7-ad7f-4649-b130-8ed0a65481f7" />

> **Web image optimization engine for Node.js.** Rust core, smaller outputs than sharp.

lazy-image reduces image file sizes by 20-30% compared to sharp in its target web-delivery scenarios, trading encoding speed for smaller outputs. Ideal for CDN-heavy workloads where bandwidth costs matter more than CPU costs.

- **Not** a drop-in replacement for sharp — use sharp if you need its full API or maximum throughput.
- **Security-first**: Metadata stripped by default; `keepMetadata()` to preserve. Zero-copy path: `fromPath()`/`processBatch()` → `toFile()`.
- Japanese: [README.ja.md](./README.ja.md). **mmap**: Do not modify/delete source files while processing; use a copy or `from(Buffer)` for mutable inputs.
- Lazy contract: [docs/LAZY_SEMANTICS.md](./docs/LAZY_SEMANTICS.md) explains what is deferred vs eager.
- Metadata matrix: [docs/METADATA_SUPPORT.md](./docs/METADATA_SUPPORT.md).
- Philosophy and non-goals: [docs/PROJECT_PHILOSOPHY.md](./docs/PROJECT_PHILOSOPHY.md).

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

## Choose lazy-image if / Choose sharp if

| **Choose lazy-image if** | **Choose sharp if** |
|---|---|
| You pay for bandwidth (CDN, S3, CloudFront) | You need maximum encoding throughput |
| You run in serverless / memory-constrained envs | You need a broad image editing API |
| You want AVIF with good defaults | You need drop-in API compatibility |
| You want smaller outputs over faster encodes | You need GIF, SVG, or TIFF support |

**Key differentiators:**

1. **File size optimization** — mozjpeg + libwebp + ravif produce 20-30% smaller outputs than sharp's defaults
2. **Memory efficiency** — zero-copy mmap architecture; no pixel data copied to the JS heap
3. **Security-first** — GPS auto-strip, Image Firewall, Rust memory safety
4. **AVIF excellence** — ravif encoder with quality-optimized defaults

**What lazy-image does NOT compete on:** raw encoding speed (sharp/libvips is faster), feature breadth (no drawing, compositing, or GIF), API compatibility with sharp.

**Project philosophy:** optimize for smaller web outputs, bounded memory, and safe defaults first. Any speed claims must be codec- and workload-specific, not a blanket "faster than sharp" promise.

Benchmarks and details: [docs/PERFORMANCE.md](./docs/PERFORMANCE.md). Full compatibility matrix: [docs/COMPATIBILITY.md](./docs/COMPATIBILITY.md).

---

## Recommended Paths

Start with one of these workflows:

- **Web delivery optimization**: `fromPath() -> resize()/crop() -> toFile()` for the lowest heap usage and the clearest operational path
- **Upload sanitization**: `fromPath() -> sanitize({ policy: 'strict' }) -> toFile()/toBuffer()` for untrusted user uploads
- **Build-time / batch generation**: `processBatch()` or `clone()` for static sites, media pipelines, and multi-output generation
- **Final optimization after editing**: use sharp/ImageMagick for compositing, filters, or animation, then pass the result through lazy-image for final JPEG/WebP/AVIF optimization

Scenario guide: [docs/ADOPTION_GUIDE.md](./docs/ADOPTION_GUIDE.md).

---

## Cost Savings Example (ROI)

Assume:
- average image size before optimization: **1.0 MB**
- average reduction with lazy-image: **25%**
- CDN transfer rate: **$0.085/GB** (example: CloudFront pay-as-you-go, US/EU next 9TB tier)

| Scenario | Transfer with sharp | Transfer with lazy-image | Monthly savings |
|---|---:|---:|---:|
| 1M image deliveries / month | 976.6 GB | 732.4 GB | **$20.75** |
| 10M image deliveries / month | 9.54 TB | 7.15 TB | **$207.52** |
| 100M image deliveries / month | 95.37 TB | 71.53 TB | **$2,075.20** |

Break-even intuition:
- Encoding overhead is paid **once per generated image**
- Bandwidth savings are realized **every time the image is delivered**
- If each generated image is viewed multiple times, lazy-image generally wins quickly

Use the interactive calculator:
- [docs/roi-calculator.html](./docs/roi-calculator.html)
- Methodology and formulas: [docs/ROI_CALCULATOR.md](./docs/ROI_CALCULATOR.md)

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
  .resize({ width: 800, fit: 'inside' }) // positional args also supported
  .toFile('thumb.jpg', 'jpeg', 85);
```

**Format conversion (e.g. PNG → WebP/AVIF)**

```javascript
const buffer = await ImageEngine.fromPath('input.png')
  .resize({ width: 600 })
  .toBuffer('webp', 80);
```

**Metadata without decoding**

```javascript
const { inspectFile } = require('@alberteinshutoin/lazy-image');
const meta = inspectFile('input.jpg'); // { width, height, format }
```

`lazy-image` defers full pixel decode, transforms, and encoding until `toBuffer()` / `toFile()`. Constructors still do source setup and metadata extraction; see [docs/LAZY_SEMANTICS.md](./docs/LAZY_SEMANTICS.md).

More: batch processing, presets, metrics, streaming — [docs/API.md](./docs/API.md).

---

## Documentation

| Topic | Link |
|-------|------|
| **Examples** | [examples/](./examples/) |
| **Full API reference** | [docs/API.md](./docs/API.md) |
| **Migration from sharp** | [docs/MIGRATION_FROM_SHARP.md](./docs/MIGRATION_FROM_SHARP.md) |
| **Performance & when to use** | [docs/PERFORMANCE.md](./docs/PERFORMANCE.md) |
| **Architecture & security** | [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) |
| **Troubleshooting** | [docs/TROUBLESHOOTING.md](./docs/TROUBLESHOOTING.md) |
| **Security policy & reporting** | [SECURITY.md](./SECURITY.md) |
| **Roadmap & scope** | [docs/ROADMAP.md](./docs/ROADMAP.md) |
| **Adoption guide / recommended workflows** | [docs/ADOPTION_GUIDE.md](./docs/ADOPTION_GUIDE.md) |
| **Metadata support matrix** | [docs/METADATA_SUPPORT.md](./docs/METADATA_SUPPORT.md) |
| **ROI calculator methodology** | [docs/ROI_CALCULATOR.md](./docs/ROI_CALCULATOR.md) |
| **Version history** | [docs/VERSION_HISTORY.md](./docs/VERSION_HISTORY.md) |
| **Specification (spec/)** | [spec/pipeline.md](./spec/pipeline.md), [spec/resize.md](./spec/resize.md), [spec/errors.md](./spec/errors.md), [spec/limits.md](./spec/limits.md), [spec/quality.md](./spec/quality.md), [spec/metadata.md](./spec/metadata.md) |
| **Error codes** | [docs/ERROR_CODES.md](./docs/ERROR_CODES.md) |
| **Benchmarks (raw data)** | [docs/TRUE_BENCHMARKS.md](./docs/TRUE_BENCHMARKS.md) |
| **Binary size comparison (AVIF on/off)** | [docs/BINARY_SIZE.md](./docs/BINARY_SIZE.md) |
| **Benchmark snapshots log** | [docs/BENCHMARK_RESULTS.md](./docs/BENCHMARK_RESULTS.md) |
| **Benchmark operations** | [docs/BENCHMARK_OPERATIONS.md](./docs/BENCHMARK_OPERATIONS.md) |

---

## Features (summary)

Smaller JPEG (mozjpeg) · Smaller WebP (libwebp) · AVIF (ravif) · ICC profiles (AVIF in v0.9.x) · EXIF auto-orient · Zero-copy file I/O · Disk-backed bounded-memory pipeline (`createStreamingPipeline()`, not true chunked transform streaming) · Image Firewall · GPS auto-strip · Fluent API · Rust core (NAPI-RS) · Cross-platform (macOS, Windows, Linux). Design choices and limits: [docs/ROADMAP.md](./docs/ROADMAP.md) and [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md).

---

## Development

```bash
npm install && npm run build
npm test
```

See [CONTRIBUTING.md](./CONTRIBUTING.md) for workflow, test commands, and contribution guidelines. Benchmark testing: [lazy-image-test](https://github.com/albert-einshutoin/lazy-image-test) Docker environment. Fuzzing: [FUZZING.md](./FUZZING.md).

---

## License

MIT

---

## Credits

[mozjpeg](https://github.com/mozilla/mozjpeg) · [libwebp](https://chromium.googlesource.com/webm/libwebp) · [ravif](https://github.com/kornelski/ravif) · [fast_image_resize](https://github.com/Cykooz/fast_image_resize) · [img-parts](https://github.com/paolobarbolini/img-parts) · [napi-rs](https://napi.rs/)

---

**Ship it.** 🚀
