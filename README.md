# lazy-image 🦀

<img width="256" height="256" alt="image" src="https://github.com/user-attachments/assets/239496c7-ad7f-4649-b130-8ed0a65481f7" />

> **Web image optimization engine for Node.js.** Rust core, smaller JPEG outputs, bounded memory.

At the same encoder quality setting in current canonical benchmarks, lazy-image produces **17-20% smaller JPEG outputs** than sharp for two PNG → JPEG cases: no-resize conversion and resize-to-800px. The resize case passes a source-reference quality floor but is not an exact perceptual-quality match. AVIF/WebP and multi-operation pipelines are workload-specific.

- **Not** a drop-in replacement for sharp — use sharp if you need its full API or maximum throughput.
- **Security-first**: Metadata is stripped by default; `keepMetadata()` preserves
  ICC and EXIF (not XMP in this runtime). GPS is stripped unless explicitly
  disabled. File-path inputs (`fromPath()`/`processBatch()` → `toFile()`) bypass
  the V8 heap. Small/medium files (≤ 256 MB) are read into Rust-owned memory
  for SIGBUS safety, and files larger than 256 MB use mmap with advisory locks.
  For mmap paths, avoid modifying, truncating, or deleting source files during
  processing; use a copy or `from(Buffer)` for mutable inputs.
  See [docs/ZERO_COPY.md](./docs/ZERO_COPY.md).
- Japanese: [README.ja.md](./README.ja.md). **mmap (files > 256 MB)**: do not modify or delete source files while processing; use a copy or `from(Buffer)` for mutable inputs.
- Lazy contract: [docs/LAZY_SEMANTICS.md](./docs/LAZY_SEMANTICS.md) explains what is deferred vs eager.
- Metadata matrix: [docs/METADATA_SUPPORT.md](./docs/METADATA_SUPPORT.md).
- Philosophy and non-goals: [docs/PROJECT_PHILOSOPHY.md](./docs/PROJECT_PHILOSOPHY.md).

[![npm version](https://badge.fury.io/js/@alberteinshutoin%2Flazy-image.svg)](https://www.npmjs.com/package/@alberteinshutoin/lazy-image)
[![npm downloads](https://img.shields.io/npm/dm/@alberteinshutoin/lazy-image)](https://www.npmjs.com/package/@alberteinshutoin/lazy-image)
[![Node.js CI](https://github.com/albert-einshutoin/lazy-image/actions/workflows/CI.yml/badge.svg)](https://github.com/albert-einshutoin/lazy-image/actions/workflows/CI.yml)
[![codecov](https://codecov.io/gh/albert-einshutoin/lazy-image/branch/main/graph/badge.svg)](https://codecov.io/gh/albert-einshutoin/lazy-image)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.88+-orange.svg)](https://www.rust-lang.org/)

---

## Quick Start (5 lines)

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

const bytesWritten = await ImageEngine.fromPath('input.png')
  .resize(800)
  .toFile('output.jpg', 'jpeg', 80);

console.log(`Wrote ${bytesWritten} bytes`);
```

## Choose lazy-image if / Choose sharp if

| **Choose lazy-image if** | **Choose sharp if** |
|---|---|
| You pay for bandwidth (CDN, S3, CloudFront) | You need maximum encoding throughput |
| You run in serverless / memory-constrained envs | You need a broad image editing API |
| You want AVIF with safe defaults | You need drop-in API compatibility |
| You want smaller JPEG outputs over broader codec throughput | You need GIF, SVG, or TIFF support |

**Key differentiators:**

1. **JPEG file size optimization** — mozjpeg produces 17-20% smaller JPEG outputs in the canonical simple PNG → JPEG no-resize and resize benchmarks
2. **Memory efficiency** — file-path inputs are not copied into the V8 heap; decoded pixels stay in Rust memory
3. **Security-first** — GPS auto-strip, Image Firewall, Rust memory safety
4. **AVIF output support** — libavif encoder with quality-tuned defaults and ICC support; benchmark AVIF workloads for size/speed

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
- average JPEG-oriented reduction with lazy-image: **18%** (example only; not universal across all codecs)
- CDN transfer rate: **$0.085/GB** (example: CloudFront pay-as-you-go, US/EU next 9TB tier)

| Scenario | Transfer with sharp | Transfer with lazy-image | Monthly savings |
|---|---:|---:|---:|
| 1M image deliveries / month | 976.6 GB | 800.8 GB | **$14.94** |
| 10M image deliveries / month | 9.54 TB | 7.82 TB | **$149.41** |
| 100M image deliveries / month | 95.37 TB | 78.20 TB | **$1,494.14** |

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

| Distribution path | Notes |
|---|---|
| npm optional binaries | macOS arm64/x64, Linux x64/arm64 GNU, Linux x64 musl, Windows x64 |
| Source build | Node.js 18+, Rust 1.88+, and the native codec toolchain documented in [CONTRIBUTING.md](./CONTRIBUTING.md) |

---

## Architecture Overview

```
src/                        # selected high-level Rust core modules
├── engine/
│   ├── api/                # ImageEngine public API + NAPI-facing operations
│   ├── pipeline/           # Operation application, color state, capabilities, optimization
│   ├── tasks/              # NAPI async task contexts and encode/batch/write execution
│   ├── memory/             # Cgroup detection, memory estimates, weighted semaphore
│   ├── io/                 # File sources, mmap, ICC/EXIF extraction and embedding
│   ├── resize.rs           # Dimension calc, fast_image_resize dispatch
│   ├── encoder.rs          # JPEG/PNG/WebP/AVIF encoding + ICC/EXIF embedding
│   ├── decoder.rs          # Format detection + decoding
│   ├── firewall.rs         # Input sanitization policies
│   ├── metadata.rs         # Metadata policy and preservation state
│   └── validation.rs       # Input, operation, and output validation helpers
├── ops.rs                  # Operation enum, presets, output formats
├── error.rs                # 4-tier error taxonomy (E1xx–E9xx)
└── codecs/avif_safe.rs     # Safe libavif FFI wrappers

lib/helpers.js              # Encoding profiles, target-bytes binary search
streaming/pipeline.js       # Disk-backed bounded-memory streaming
```

**Key design decisions:** lazy execution (ops queue until output), file-path inputs bypass the V8 heap (read-into-memory for ≤ 256 MB, mmap with advisory locks for > 256 MB), memory-bounded concurrency via weighted semaphore, panic guards on all codec entry points. See [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) for details.

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
| **TypeScript guide** | [docs/TYPESCRIPT.md](./docs/TYPESCRIPT.md) |
| **Full API reference** | [docs/API.md](./docs/API.md) |
| **Migration from sharp** | [docs/MIGRATION_FROM_SHARP.md](./docs/MIGRATION_FROM_SHARP.md) |
| **Performance & when to use** | [docs/PERFORMANCE.md](./docs/PERFORMANCE.md) |
| **Architecture & security** | [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) |
| **Troubleshooting** | [docs/TROUBLESHOOTING.md](./docs/TROUBLESHOOTING.md) |
| **Security policy & reporting** | [SECURITY.md](./SECURITY.md) |
| **Roadmap & scope** | [docs/ROADMAP.md](./docs/ROADMAP.md) |
| **Adoption guide / recommended workflows** | [docs/ADOPTION_GUIDE.md](./docs/ADOPTION_GUIDE.md) |
| **Wasm / browser / Edge strategy** | [docs/WASM_STRATEGY.md](./docs/WASM_STRATEGY.md) |
| **Wasm package and policy API** | [docs/WASM_PACKAGE_API.md](./docs/WASM_PACKAGE_API.md) |
| **Wasm upload benchmark guidance** | [docs/WASM_BENCHMARKING.md](./docs/WASM_BENCHMARKING.md) |
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

Smaller JPEG (mozjpeg) · WebP (libwebp) · AVIF (libavif) · ICC profiles · EXIF auto-orient · Bypass-V8-heap file I/O (read-into-memory ≤ 256 MB, mmap > 256 MB) · Disk-backed bounded-memory pipeline (`createStreamingPipeline()`, not true chunked transform streaming) · Image Firewall · GPS auto-strip · Fluent API · Rust core (NAPI-RS) · Cross-platform (macOS, Windows, Linux). Design choices and limits: [docs/ROADMAP.md](./docs/ROADMAP.md) and [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md).

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

[mozjpeg](https://github.com/mozilla/mozjpeg) · [libwebp](https://chromium.googlesource.com/webm/libwebp) · [libavif](https://github.com/AOMediaCodec/libavif) · [fast_image_resize](https://github.com/Cykooz/fast_image_resize) · [img-parts](https://github.com/paolobarbolini/img-parts) · [napi-rs](https://napi.rs/)

---

**Ship it.** 🚀
