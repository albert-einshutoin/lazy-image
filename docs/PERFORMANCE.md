# Performance & When to Use

lazy-image is optimized for **smaller output and memory safety** rather than raw throughput. This guide helps you choose between lazy-image and sharp and use lazy-image effectively.

## Benchmark Summary (Large Image 5000×5000px)

| Feature | Metric | lazy-image | sharp | Verdict |
|---------|--------|------------|-------|---------|
| **AVIF Generation** | Time | **13.3s** | 47.4s | lazy-image ~3.5× faster |
| **JPEG Compression** | File Size | **1.1 MB** | 1.5 MB | lazy-image ~26% smaller |
| **JPEG Encoding** | Speed | 1.2s | **0.3s** | sharp ~4× faster |
| **Memory (format conv.)** | Peak RSS | **713 MB** | 2,416 MB | lazy-image ~70% less |
| **WebP Resize** | Speed | 320ms | **134ms** | sharp ~2.5× faster |

Full data: [TRUE_BENCHMARKS.md](./TRUE_BENCHMARKS.md).

---

## When to Use lazy-image

- **Serverless (Lambda, Cloud Run, Vercel, etc.)** — Avoid OOM and smaller cold-start footprint.
- **Bandwidth-sensitive** — Smaller JPEG/AVIF saves CDN and transfer costs.
- **AVIF generation** — Production-ready AVIF without timeouts on large files.
- **Memory-constrained** — 512MB containers; zero-copy path keeps heap low.
- **Safety-first** — Rust memory safety; built-in decompression limits and Image Firewall.

## When to Use sharp

- **Heavy persistent servers** — Plenty of RAM and CPU.
- **Throughput-critical** — Thousands of JPEG/WebP resizes per second.
- **No AVIF** — Legacy formats only and maximum resize speed.

## Philosophy

lazy-image trades raw JPEG/WebP encoding speed for **smaller files** and **stable memory**. “Slow to encode, fast to load.”

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
- Single static binary (mozjpeg, libwebp, ravif linked in).
- Main npm package ~15 KB + one platform binary.

**Ideal for**: AWS Lambda, Vercel Edge, Cloudflare Workers, Google Cloud Functions.
