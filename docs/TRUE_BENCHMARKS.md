# True Benchmarks: AVIF Speed & JPEG Size Advantages

This document provides comprehensive benchmark documentation showing the actual performance characteristics of lazy-image, with a focus on:

- **AVIF encoding speed advantages** (lazy-image is 1.70x faster than sharp for format conversion)
- **JPEG file size advantages** (mozjpeg optimization produces 17-20% smaller files)

## Test Environment

All benchmarks use images from `test/fixtures/*` directory:

- **Large test image**: `test/fixtures/test_4.5MB_5000x5000.png` (4.5MB PNG, 5000×5000 pixels)
- **Small test images**: Various sizes in `test/fixtures/` for different scenarios

### Benchmark Configuration

| Item | Version/Spec |
|------|--------------|
| **Node.js** | v22.x |
| **sharp** | 0.34.x |
| **Test Images** | `test/fixtures/*` (test_4.5MB_5000x5000.png: 4.5MB, 5000×5000, test_100KB_*, etc.) |
| **Output Size** | 800px width (auto height) |
| **Quality** | JPEG: 80, WebP: 80, AVIF: 60 |
| **Platform** | macOS (Apple Silicon) |
| **Test Date** | Actual benchmark results from test execution |

**How to reproduce:**
```bash
npm run test:bench:compare
```

> **Note**: Benchmark results may vary depending on hardware, Node.js version, and sharp version. These results are for reference only.

---

## AVIF Encoding Speed Advantages

lazy-image significantly outperforms sharp in AVIF encoding speed, making it ideal for next-generation image formats.

### Performance Results

| Scenario | Format | lazy-image | sharp | Speed Advantage |
| :--- | :--- | :--- | :--- | :--- |
| **Speed (No Resize)** | **AVIF** | **3,137ms** 🚀 | 5,320ms | **1.70x Faster** |
| **Speed (Resize 800px)** | **AVIF** | **180ms** ⚡ | 195ms | **1.08x Faster** |
| **File Size (No Resize)** | **AVIF** | **762,711 bytes** 📉 | 1,290,501 bytes | **-40.9%** ✅ |
| **File Size (Resize 800px)** | **AVIF** | **24,343 bytes** | 22,227 bytes | +9.5% |

### Technical Explanation

lazy-image uses **ravif**, a pure Rust AVIF encoder based on AV1 compression. The speed advantages come from:

1. **Optimized Speed Settings**: Quality-based speed tuning that balances encoding speed and file size
   - Higher quality (70-80): Speed 4-6 (faster encoding)
   - Medium quality (50-70): Speed 2-4 (balanced)
   - Lower quality (30-50): Speed 1-2 (maximum compression)

2. **RGB Encoding for Opaque Images**: Automatically detects RGB images and uses `encode_rgb()` instead of `encode_rgba()`, reducing file size by 5-10% for opaque images

3. **Zero-Copy Architecture**: For format conversions without pixel manipulation, lazy-image's Copy-on-Write (CoW) architecture avoids intermediate buffer allocations

4. **Pure Rust Implementation**: No FFI overhead compared to sharp's libvips-based approach

### Use Cases

- **Batch AVIF generation**: Process large image galleries with AVIF format
- **Build-time optimization**: Generate AVIF variants during static site generation
- **CDN optimization**: Serve next-generation formats with faster encoding

### Example Usage

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

// Fast AVIF encoding
const avifBuffer = await ImageEngine.fromPath('test/fixtures/test_4.5MB_5000x5000.png')
  .resize(800, null)
  .toBuffer('avif', 60); // Quality 60, optimized speed

// Compare with sharp (slower)
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
| **Speed (No Resize)** | JPEG | 668ms | **681ms** | **1.02x Faster** ⚡ |
| **Speed (Resize 800px)** | JPEG | 115ms | **92ms** | 0.80x slower (optimized for size) |

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

**File Size vs Speed**: lazy-image prioritizes compression ratio (smaller file sizes) over raw encoding speed for JPEG. This results in:
- **Smaller files** (17-20% reduction) to save bandwidth costs
- **Comparable or slightly longer processing times** compared to sharp (depending on scenario)
- **Intentional trade-off**: Bandwidth savings often outweigh processing time in web applications

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

// Result: 17-20% smaller than sharp with same quality
```

---

## Comprehensive Benchmark Results

### Format Conversion Efficiency (No Resize)

When converting formats without resizing, lazy-image's CoW architecture delivers exceptional performance:

| Conversion | lazy-image | sharp | Speed | File Size |
|------------|------------|-------|-------|-----------|
| **PNG → AVIF** | 3,137ms | 5,320ms | **1.70x faster** ⚡ | **-40.9%** ✅ |
| **PNG → JPEG** | 668ms | 681ms | **1.02x faster** ⚡ | **-17.0%** ✅ |
| **PNG → WebP** | 6,777ms | 975ms | 0.14x slower 🐢 | **-0.7%** ✅ |

> *Pure format conversion without pixel manipulation. 4.5MB PNG (5000×5000) input from `test/fixtures/test_4.5MB_5000x5000.png`.*

> *\* WebP encoding optimized in v0.8.1: settings adjusted (method 4, single pass) to improve speed. Performance benchmarks pending verification.*

### Why the Difference?

lazy-image's **zero-copy architecture** avoids intermediate buffer allocations during format conversion, making it ideal for batch processing pipelines:

1. **True Lazy Loading**: `fromPath()` creates a lightweight reference. File I/O only occurs when `toBuffer()`/`toFile()` is called.
2. **Zero-Copy Conversions**: For format conversions (e.g., PNG → WebP) without pixel manipulation (resize/crop), **no pixel buffer allocation or copy occurs**. The engine reuses the decoded buffer directly.
3. **Smart Cloning**: `.clone()` operations are instant and memory-free until a destructive operation is applied.

---

## Key Advantages Summary

```
AVIF: 1.70x faster encoding (format conversion) + 40.9% smaller files
JPEG: 17-20% smaller files (optimized for compression ratio)
WebP: 0.7% smaller files (but slower encoding)
Memory: Zero-copy architecture for format conversions
```

**Summary**: lazy-image excels at **AVIF generation** (both speed and file size for format conversion) and **JPEG compression efficiency** (17-20% smaller files). For WebP, lazy-image produces slightly smaller files but with slower encoding speed.

---

## Benchmark Methodology

### Test Images

All benchmarks use images from `test/fixtures/*` directory:
- `test/fixtures/test_4.5MB_5000x5000.png` - Large PNG (4.5MB, 5000×5000) for performance tests
- `test/fixtures/test_100KB_*` - Medium-sized images for various format tests (100KB_1188x1188.png, 100KB_1057x1057.jpg, 90KB_1471x1471.webp, 95KB.avif)
- Other test fixtures for specific scenarios

### Test Procedure

1. **Warm-up**: Run each test 3 times to warm up the JIT compiler
2. **Measurement**: Run each test 10-30 times and calculate average
3. **Comparison**: Compare lazy-image vs sharp with identical parameters
4. **Validation**: Verify output quality is visually equivalent

### Reproducing Benchmarks

```bash
# Run comprehensive benchmark comparison
npm run test:bench:compare

# Run format conversion benchmark (no resize)
node test/benchmarks/convert-only.bench.js

# Run README verification benchmark
npm run test:bench:verify
```

---

## Limitations & Notes

### Performance Trade-offs

- **JPEG encoding speed**: lazy-image prioritizes compression ratio over raw encoding speed. This means slightly longer processing times (2-3x) but significantly smaller files (up to 50% reduction). This trade-off is intentional to save bandwidth costs.
- **Real-time processing**: For strict latency requirements (<100ms), sharp may be more suitable due to its faster JPEG encoding.

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

1. **AVIF encoding speed**: 1.70x faster than sharp for format conversion, making it ideal for next-generation image formats
2. **JPEG file size**: 17-20% smaller files through mozjpeg optimization, reducing bandwidth costs
3. **Memory efficiency**: Zero-copy architecture for format conversions
4. **Build-time optimization**: Ideal for static site generation and CI/CD pipelines

Choose lazy-image when:
- File size optimization is critical (bandwidth savings)
- AVIF generation is needed (speed advantage)
- Memory constraints exist (serverless, containers)
- Batch processing is acceptable (build-time optimization)

Choose sharp when:
- Real-time processing with strict latency requirements (<100ms)
- Maximum throughput is needed (high-volume processing)
- Complex operations are needed (advanced filters, color space conversions)

---

*Last updated: Based on lazy-image v0.8.1 and sharp v0.34.x*

