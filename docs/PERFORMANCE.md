# Performance & When to Use

lazy-image is optimized for **JPEG output size and memory safety** rather than raw throughput. This guide helps you choose between lazy-image and sharp and use lazy-image effectively.

## Canonical Benchmark Source

The canonical benchmark dataset for public performance claims is [TRUE_BENCHMARKS.md](./TRUE_BENCHMARKS.md).

All summary claims in README and migration docs should be derived from that document's exact scenarios rather than mixing numbers from different workloads.

For the full inventory of public claims and quoting rules, see [BENCHMARK_CLAIMS.md](./BENCHMARK_CLAIMS.md).

## Benchmark Summary (Canonical Scenarios)

| Scenario | Metric | lazy-image | sharp | Interpretation |
|---------|--------|------------|-------|----------------|
| PNG → JPEG (no resize, 5000×5000) | Output size | **1,224,894 bytes** | 1,475,223 bytes | lazy-image output is **17.0% smaller** |
| PNG → JPEG (resize 800px, 5000×5000 input) | Output size | **31,518 bytes** | 39,416 bytes | lazy-image output is **20.0% smaller** |
| PNG → JPEG (resize 800px, 5000×5000 input) | Encode time | **70ms** | 79ms | lazy-image was **1.13x faster** in this run |
| PNG → AVIF (no resize, 5000×5000) | Encode time | 12,668ms | **4,729ms** | sharp is faster for this workload |
| PNG → AVIF (no resize, 5000×5000) | Output size | 1,718,430 bytes | **1,290,501 bytes** | sharp output is smaller for this workload |
| PNG → WebP (no resize, 5000×5000) | Encode time | 4,323ms | **909ms** | sharp is much faster for this workload |
| JPEG/WebP real-time resize workloads | Latency | varies by codec and settings | often faster | use sharp if sub-100ms latency is the main priority |

Full data: [TRUE_BENCHMARKS.md](./TRUE_BENCHMARKS.md).

---

## When to Use lazy-image

- **Node-runtime serverless (AWS Lambda, Google Cloud Run, Vercel Node Functions, Google Cloud Functions)** — Avoid OOM and smaller cold-start footprint. V8-isolate runtimes such as Cloudflare Workers and Vercel Edge are **not supported** by the native NAPI build (tracked under [#645](https://github.com/albert-einshutoin/lazy-image/issues/645) for future Wasm support).
- **Bandwidth-sensitive JPEG delivery** — Smaller JPEG saves CDN and transfer costs in the two simple canonical PNG → JPEG benchmarks; multi-operation pipelines need their own measurement.
- **AVIF output support** — Use when you need lazy-image's AVIF output path, ICC handling, and safety defaults; benchmark target AVIF workloads before assuming size or speed wins.
- **Memory-constrained** — 512MB containers; `fromPath()` bypasses the V8 heap (read-into-memory ≤ 256 MB, mmap with advisory locks > 256 MB).
- **Safety-first** — Rust memory safety; built-in decompression limits and Image Firewall.

## When to Use sharp

- **Heavy persistent servers** — Plenty of RAM and CPU.
- **Throughput-critical** — Thousands of JPEG/WebP resizes per second.
- **AVIF/WebP speed or size is decisive** — Current canonical AVIF/WebP benchmarks favor sharp for speed, and often for output size.

## Philosophy

lazy-image trades raw throughput for **JPEG compression efficiency**, **stable memory**, and **safe defaults**. Treat benchmark wins as **codec- and workload-specific**, not universal throughput claims.

---

## Performance Notes

### Memory efficiency

- **Prefer file-to-file**: `fromPath(...).toFile(...)` avoids loading the image into the Node.js heap.
- **Avoid**: `fs.readFileSync('huge.jpg')` then `ImageEngine.from(buffer)` — the whole file sits in V8.

```javascript
// ✅ Good: no heap usage for input
await ImageEngine.fromPath('huge.png').resize(800).toFile('out.jpg', 'jpeg', 80);

// ❌ Bad: entire file in V8 heap
const buf = fs.readFileSync('huge.png');
await ImageEngine.from(buf).resize(800).toBuffer('jpeg', 80);
```

### Use cases where lazy-image fits

- Build-time optimization (SSG, CI/CD)
- Batch thumbnails / media pipelines
- CDN / mobile backends
- AVIF and WebP encoding when feature support and safety defaults matter; benchmark when speed/size is critical
- Color-accurate workflows (ICC preservation)

### When sharp may be better

- Real-time processing with strict latency (&lt;100ms).

---

## Serverless

### Binary size

| Platform | lazy-image | sharp (total) | Savings |
|----------|------------|--------------|---------|
| macOS ARM64 | **~5.7 MB** | ~17 MB | ~66% |
| Linux x64 | **~9.1 MB** | ~21 MB | ~57% |
| Windows x64 | **~9.1 MB** | ~15 MB | ~39% |

### Zero config

- No `LD_LIBRARY_PATH` or system libvips.
- Single static binary (mozjpeg, libwebp; AVIF via `avif` feature).
- Main npm package ~29 KB tarball / ~107 KB unpacked + one platform binary (run `npm pack --dry-run --json` for the current numbers).

**Ideal for Node-runtime serverless**: AWS Lambda, Google Cloud Functions, Google Cloud Run, Vercel Node Functions.

**Not supported**: V8-isolate runtimes such as Cloudflare Workers and Vercel Edge. The published native NAPI module requires a Node.js runtime; Wasm support is tracked under [#645](https://github.com/albert-einshutoin/lazy-image/issues/645).
