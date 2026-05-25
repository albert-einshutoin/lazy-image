# Performance & When to Use

lazy-image is optimized for **smaller output and memory safety** rather than raw throughput. This guide helps you choose between lazy-image and sharp and use lazy-image effectively.

## Canonical Benchmark Source

The canonical benchmark dataset for public performance claims is [TRUE_BENCHMARKS.md](./TRUE_BENCHMARKS.md).

All summary claims in README and migration docs should be derived from that document's exact scenarios rather than mixing numbers from different workloads.

For the full inventory of public claims and quoting rules, see [BENCHMARK_CLAIMS.md](./BENCHMARK_CLAIMS.md).

## Benchmark Summary (Canonical Scenarios)

| Scenario | Metric | lazy-image | sharp | Interpretation |
|---------|--------|------------|-------|----------------|
| PNG → AVIF (no resize, 5000×5000) | Encode time | **3,137ms** | 5,320ms | lazy-image is **1.70x faster** in this specific format-conversion workload |
| PNG → AVIF (no resize, 5000×5000) | Output size | **762,711 bytes** | 1,290,501 bytes | lazy-image output is **40.9% smaller** in this workload |
| PNG → JPEG (no resize, 5000×5000) | Output size | **1,224,894 bytes** | 1,475,223 bytes | lazy-image output is **17.0% smaller** |
| PNG → WebP (no resize, 5000×5000) | Encode time | 6,777ms | **975ms** | sharp is much faster for this workload |
| JPEG/WebP real-time resize workloads | Latency | varies by codec and settings | often faster | use sharp if sub-100ms latency is the main priority |

Full data: [TRUE_BENCHMARKS.md](./TRUE_BENCHMARKS.md).

---

## When to Use lazy-image

- **Node-runtime serverless (AWS Lambda, Google Cloud Run, Vercel Node Functions, Google Cloud Functions)** — Avoid OOM and smaller cold-start footprint. V8-isolate runtimes such as Cloudflare Workers and Vercel Edge are **not supported** by the native NAPI build (tracked under [#87](https://github.com/albert-einshutoin/lazy-image/issues/87) for future Wasm support).
- **Bandwidth-sensitive** — Smaller JPEG/AVIF saves CDN and transfer costs.
- **AVIF generation** — Strong results in the benchmarked PNG → AVIF format-conversion workloads.
- **Memory-constrained** — 512MB containers; `fromPath()` bypasses the V8 heap (read-into-memory ≤ 256 MB, mmap with advisory locks > 256 MB).
- **Safety-first** — Rust memory safety; built-in decompression limits and Image Firewall.

## When to Use sharp

- **Heavy persistent servers** — Plenty of RAM and CPU.
- **Throughput-critical** — Thousands of JPEG/WebP resizes per second.
- **No AVIF** — Legacy formats only and maximum resize speed.

## Philosophy

lazy-image trades raw JPEG/WebP encoding speed for **smaller files** and **stable memory**. Treat benchmark wins as **codec- and workload-specific**, not universal throughput claims.

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
- AVIF and WebP encoding (v0.8.1+ WebP tuned for speed)
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

**Not supported**: V8-isolate runtimes such as Cloudflare Workers and Vercel Edge. The published native NAPI module requires a Node.js runtime; Wasm support is tracked under [#87](https://github.com/albert-einshutoin/lazy-image/issues/87).
