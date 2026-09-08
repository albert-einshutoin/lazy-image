# True Benchmarks: Canonical Benchmark Dataset

This document is the **canonical benchmark source** for public performance claims in lazy-image documentation.

Use this file when citing benchmark numbers in:

- `README.md`
- `docs/PERFORMANCE.md`
- `docs/MIGRATION_FROM_SHARP.md`

Do not mix numbers from different workloads without clearly labeling the scenario.

This document provides comprehensive benchmark documentation showing the actual performance characteristics of lazy-image, with a focus on:

- **JPEG file size advantages** in benchmarked web-delivery scenarios
- **AVIF/WebP trade-offs** where current sharp results are often faster or smaller
- **Copy and memory semantics** that distinguish V8-heap avoidance from codec working buffers

## Test Environment

All benchmarks use images from `test/fixtures/*` directory:

- **Large test image**: `test/fixtures/test_4.5MB_5000x5000.png` (4.5MB PNG, 5000×5000 pixels)
- **Small test images**: Various sizes in `test/fixtures/` for different scenarios

### Benchmark Configuration

| Item | Version/Spec |
|------|--------------|
| **lazy-image** | v0.16.0 |
| **Node.js** | v24.2.0 |
| **sharp** | 0.34.5 |
| **Test Images** | `test/fixtures/*` (test_4.5MB_5000x5000.png: 4.5MB, 5000×5000, test_100KB_*, etc.) |
| **Output Size** | No-resize conversion and 800px width resize scenarios |
| **Quality** | JPEG: 80, WebP: 80, AVIF: 60 |
| **Platform** | macOS 26.3, Apple M4 (arm64) |
| **Test Date** | 2026-07-17 |

**How to reproduce:**
```bash
npm run test:bench
node --expose-gc test/benchmarks/convert-only.bench.js
npm run test:bench:extended
```

> **Note**: Benchmark results may vary depending on hardware, Node.js version, and sharp version. These results are for reference only.

---

## AVIF Current Trade-offs

In the current benchmarked PNG → AVIF scenarios below, sharp is faster and produces smaller AVIF output. Do not quote AVIF as a blanket speed or size win for lazy-image.

### Performance Results

| Scenario | Format | lazy-image | sharp | Outcome |
| :--- | :--- | :--- | :--- | :--- |
| **Speed (No Resize)** | **AVIF** | 13,440ms | **5,849ms** | lazy-image 0.44x speed ratio (sharp/lazy) |
| **Speed (Resize 800px)** | **AVIF** | 483ms | **168ms** | lazy-image 0.35x speed ratio (sharp/lazy) |
| **File Size (No Resize)** | **AVIF** | 1,718,430 bytes | **1,290,501 bytes** | **+33.2%** |
| **File Size (Resize 800px)** | **AVIF** | 28,678 bytes | **22,227 bytes** | **+29.0%** |

### Technical Explanation

lazy-image uses **libavif** through safe Rust wrappers. Current AVIF behavior is shaped by:

1. **Optimized Speed Settings**: Quality-based speed tuning that balances encoding speed and file size
   - Higher quality (85+): Speed 6
   - Balanced quality (70-84): Speed 7
   - Speed-leaning quality (50-69): Speed 8
   - Fastest lower-quality band (<50): Speed 9

2. **libavif RGB/YUV handoff**: The encoder converts the image to RGBA8 (`img.to_rgba8()`), then asks libavif to convert into YUV planes. This is not a no-copy codec path.

3. **Input-path memory model**: `fromPath()` avoids staging the source file in the V8 heap, but codec-level working buffers and output buffers still exist.

4. **Safety wrappers**: AVIF FFI calls are wrapped to keep unsafe blocks small and checked.

### Use Cases

- **Batch AVIF generation**: Process large image galleries with AVIF format
- **Build-time optimization**: Generate AVIF variants during static site generation
- **CDN optimization**: Serve next-generation formats when AVIF output is required, after benchmarking the target workload

### Example Usage

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

// AVIF output with lazy-image defaults
const avifBuffer = await ImageEngine.fromPath('test/fixtures/test_4.5MB_5000x5000.png')
  .resize(800, null)
  .toBuffer('avif', 60); // Quality 60

// Compare with sharp for your workload before making size/speed claims
// const sharpAvif = await sharp('test/fixtures/test_4.5MB_5000x5000.png')
//   .resize(800)
//   .avif({ quality: 60 })
//   .toBuffer();
```

---

## JPEG File Size Advantages (mozjpeg Optimization)

lazy-image produces significantly smaller JPEG files than sharp, thanks to mozjpeg's advanced optimization techniques.

### Performance Results

| Scenario | Format | lazy-image | sharp | Size Advantage |
| :--- | :--- | :--- | :--- | :--- |
| **File Size (No Resize)** | **JPEG** | **1,224,894 bytes** 📉 | 1,475,223 bytes | **-17.0%** ✅ |
| **File Size (Resize 800px)** | **JPEG** | **31,518 bytes** 📉 | 39,416 bytes | **-20.0%** ✅ |
| **Speed (No Resize)** | JPEG | **712ms** | 765ms | **1.07x speed ratio (sharp/lazy)** ⚡ |
| **Speed (Resize 800px)** | JPEG | 114ms | **78ms** | 0.68x speed ratio (sharp/lazy) |

### Technical Explanation: mozjpeg Optimization

lazy-image uses **mozjpeg** (Mozilla's JPEG encoder) with aggressive web optimization settings:

#### 1. Chroma Subsampling (4:2:0)
- Forces 4:2:0 chroma subsampling (2×2 pixel blocks for Cb and Cr channels)
- Halves chroma resolution - imperceptible for photos
- Reduces file size by 15-20% compared to 4:4:4

#### 2. Progressive Mode
- Enables progressive JPEG encoding
- Better compression ratio through optimized scan ordering
- Improves perceived loading performance (progressive rendering)

#### 3. Optimized Huffman Tables
- Custom Huffman tables per image (`set_optimize_coding(true)`)
- Automatically optimizes entropy coding for each image
- Reduces file size by 5-10% compared to standard tables

#### 4. Scan Optimization
- Optimizes scan order for progressive compression (`set_optimize_scans(true)`)
- Uses `AllComponentsTogether` mode for better compression
- Further reduces file size by 2-5%

#### 5. Trellis Quantization
- Automatically enabled via `set_optimize_coding(true)`
- Tries multiple quantization strategies and picks the best one
- Optimizes rate-distortion trade-off (file size vs quality)
- This is mozjpeg's "secret sauce" - reduces file size by 10-15%

#### 6. Adaptive Smoothing
- Quality-based smoothing factor:
  - Quality ≥ 90: No smoothing (0)
  - Quality 70-89: Minimal smoothing (5)
  - Quality 60-69: Moderate smoothing (10)
  - Quality < 60: Enhanced smoothing (18)
- Reduces high-frequency noise for better compression
- Maintains visual quality while improving compression ratio

#### 7. Quantization Table Optimization
- Automatically optimizes quantization tables when `optimize_coding` is enabled
- Custom tables per image for optimal compression

### Trade-offs

**File Size vs Speed**: lazy-image prioritizes compression ratio (smaller file sizes) over raw encoding speed for JPEG. In the two simple PNG -> JPEG cases this results in:
- **Smaller files** (17-20% reduction) to save bandwidth costs
- **Comparable or slightly longer processing times** compared to sharp (depending on scenario)
- **Intentional trade-off**: Bandwidth savings often outweigh processing time in web applications

### Source-reference quality condition

The resize-to-800px comparison uses a lossless resized PNG as the canonical quality reference. The encoder-to-encoder axis is reported separately. This prevents two similarly degraded outputs from satisfying the quality gate merely because they resemble each other.

| Format / setting | lazy-image vs source | sharp vs source | lazy-image gate floor | Size difference |
|---|---|---|---|---:|
| JPEG q80 | SSIM 0.9942 / PSNR 37.07 dB | SSIM 0.9955 / PSNR 37.65 dB | SSIM 0.9935 / PSNR 36.5 dB | **-20.0%** |
| WebP q80 | SSIM 0.9914 / PSNR 37.08 dB | SSIM 0.9910 / PSNR 37.12 dB | SSIM 0.9905 / PSNR 36.5 dB | +9.3% |
| AVIF q60 | SSIM 0.9950 / PSNR 39.40 dB | SSIM 0.9942 / PSNR 41.62 dB | SSIM 0.9940 / PSNR 38.5 dB | +29.0% |

The 17-20% JPEG statement is therefore a same-quality-setting result for the two named PNG → JPEG scenarios. It is not a claim of exact perceptual-quality matching; the resize case must also retain the source-reference measurements above and pass the documented regression floor.

### Use Cases

- **Bandwidth-sensitive applications**: Web apps, mobile apps, CDN optimization
- **Photography workflows**: Image galleries, social media platforms
- **Build-time optimization**: Static site generation, CI/CD pipelines
- **Cost optimization**: Reduce CDN bandwidth costs with smaller files

### Example Usage

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

// Optimized JPEG with mozjpeg
const jpegBuffer = await ImageEngine.fromPath('test/fixtures/test_4.5MB_5000x5000.png')
  .resize(800, null)
  .toBuffer('jpeg', 80); // Quality 80, mozjpeg optimization

// Result in simple canonical JPEG benchmarks: 17-20% smaller than sharp at the same quality setting
```

---

## Comprehensive Benchmark Results

### Format Conversion Efficiency (No Resize)

When converting formats without resizing, current results are codec-specific. CoW avoids unnecessary pipeline materialization, but encoders still allocate codec working buffers.

| Conversion | lazy-image | sharp | Speed | File Size |
|------------|------------|-------|-------|-----------|
| **PNG → AVIF** | 13,440ms | 5,849ms | 0.44x speed ratio | +33.2% |
| **PNG → JPEG** | 712ms | 765ms | 1.07x speed ratio | **-17.0%** ✅ |
| **PNG → WebP** | 4,379ms | 964ms | 0.22x speed ratio | +2.4% |

> *Pure format conversion without pixel manipulation. 4.5MB PNG (5000×5000) input from `test/fixtures/test_4.5MB_5000x5000.png`.*

> *Speed ratio is `sharp time / lazy-image time`; values below 1.0 mean sharp was faster.*

### Resize 800px Web-Delivery Scenario

`npm run test:bench:compare` measured the following representative 800px resize results:

| Scenario | lazy-image | sharp | Speed | File Size |
|----------|------------|-------|-------|-----------|
| **PNG → JPEG resize 800px** | 114ms / 31,518 bytes | 78ms / 39,416 bytes | 0.68x speed ratio | **-20.0%** ✅ |
| **PNG → WebP resize 800px** | 190ms / 32,448 bytes | 82ms / 29,686 bytes | 0.43x speed ratio | +9.3% |
| **PNG → AVIF resize 800px** | 483ms / 28,678 bytes | 168ms / 22,227 bytes | 0.35x speed ratio | +29.0% |
| **Resize + rotate + grayscale → JPEG** | 76ms / 27,129 bytes | 117ms / 24,650 bytes | 1.54x speed ratio | +10.1% |

### Copy and Allocation Scope

lazy-image's **zero-copy** claim is about the input crossing the JS boundary, not about every internal codec step:

1. **V8-heap avoidance**: `fromPath()` and `processBatch()` do not place source bytes in the Node.js heap. Files <= 256 MB are read into Rust-owned memory; files > 256 MB use mmap with advisory locks.
2. **Pipeline CoW**: A format-only path can avoid materializing operation outputs, while resize/crop/rotate/grayscale materialize transformed images.
3. **Codec buffers**: Encoders allocate output and working buffers. AVIF currently materializes an RGBA8 buffer and libavif YUV/alpha planes.
4. **Smart cloning**: `.clone()` shares pipeline state until divergent or destructive work is applied.

See [ZERO_COPY.md](./ZERO_COPY.md) for the precise scope and validation method.

---

## Key Advantages Summary

```
JPEG: 17-20% smaller files in simple PNG -> JPEG no-resize and resize-800px scenarios
AVIF: current PNG -> AVIF benchmarks are slower and larger than sharp
WebP: current PNG -> WebP benchmarks are slower and similar/larger than sharp
Memory: fromPath avoids V8-heap input copies; codec working buffers still exist
```

**Summary**: lazy-image's current public size advantage is strongest for **JPEG compression efficiency** in the benchmarked delivery-oriented scenarios. AVIF and WebP must be treated as workload-specific trade-offs, not blanket wins over sharp.

---

## Benchmark Methodology

### Test Images

All benchmarks use images from `test/fixtures/*` directory:
- `test/fixtures/test_4.5MB_5000x5000.png` - Large PNG (4.5MB, 5000×5000) for performance tests
- `test/fixtures/test_100KB_*` - Medium-sized images for various format tests (100KB_1188x1188.png, 100KB_1057x1057.jpg, 90KB_1471x1471.webp, 95KB.avif)
- Other test fixtures for specific scenarios

### Test Procedure

1. **Comparison**: Compare lazy-image vs sharp with identical format, quality, and resize parameters.
2. **Scenario separation**: Keep no-resize conversion, resize, memory, and cold-start results separate.
3. **Iteration counts**: Use each benchmark script's own warm-up and iteration settings; some scripts are representative single-run checks while extended matrix benchmarks average multiple runs.
4. **Validation**: Gate each required codec against the resized source reference. Encoder-to-encoder similarity remains a separate informational axis.
5. **Failure semantics**: `pass`, `skipped`, `unsupported`, `benchmark-failure`, and `quality-regression` are recorded separately in the JSON artifact; any non-pass required case makes the command fail after all cases finish.

### Reproducing Benchmarks

```bash
# Run comprehensive benchmark comparison
npm run test:bench:compare

# Run format conversion benchmark (no resize)
node test/benchmarks/convert-only.bench.js

# Run README verification benchmark
npm run test:bench:verify

# Run AVIF quality/speed matrix
npm run test:bench:avif

# Run memory tracking benchmark (RSS delta vs estimate)
npm run test:bench:memory

# Run cold-start benchmark (require + first toBuffer)
npm run test:bench:cold-start
```

For reproducible snapshots, record outputs in [docs/BENCHMARK_RESULTS.md](./BENCHMARK_RESULTS.md).
For ongoing CI/threshold operation rules, see [docs/BENCHMARK_OPERATIONS.md](./BENCHMARK_OPERATIONS.md).

---

## Limitations & Notes

### Performance Trade-offs

- **JPEG encoding speed**: lazy-image prioritizes compression ratio and produced 17-20% smaller JPEG outputs in the two simple current benchmarked PNG -> JPEG scenarios. Speed was comparable in these runs and should be remeasured on target hardware.
- **AVIF/WebP encoding speed and size**: current PNG -> AVIF and PNG -> WebP benchmarks favor sharp for speed, and often for output size.
- **Real-time processing**: For strict latency requirements (<100ms), benchmark the target codec and image mix before choosing lazy-image.

### Test Environment Variations

Benchmark results may vary depending on:
- Hardware (CPU architecture, clock speed, cache size)
- Node.js version
- sharp version
- System load
- Image content (photographs vs graphics vs text)

These benchmarks are for reference only and should be validated in your specific environment.

---

## Conclusion

lazy-image provides significant advantages in:

1. **JPEG file size**: 17-20% smaller files through mozjpeg optimization in the two simple canonical PNG -> JPEG scenarios
2. **Memory efficiency**: `fromPath()` avoids V8-heap input copies and keeps decoded pixels in Rust memory
3. **Operational safety**: metadata stripping, Image Firewall, structured errors, and panic guards
4. **Build-time optimization**: Ideal for static site generation and CI/CD pipelines

Choose lazy-image when:
- JPEG file size optimization is critical (bandwidth savings)
- Native AVIF/WebP/JPEG output support is needed and you have benchmarked the target workload
- Memory constraints exist (serverless, containers)
- Batch processing is acceptable (build-time optimization)

Choose sharp when:
- Real-time processing with strict latency requirements (<100ms)
- Maximum throughput is needed (high-volume processing)
- AVIF/WebP speed or size is the primary decision criterion in the current benchmarked scenarios
- Complex operations are needed (advanced filters, color space conversions)

---

*Last updated: 2026-07-17, based on lazy-image v0.16.0, Node.js v24.2.0, and sharp v0.34.5. Re-run `npm run test:bench`, `node --expose-gc test/benchmarks/convert-only.bench.js`, and `npm run test:bench:extended` when encoder dependencies or public claims change.*
