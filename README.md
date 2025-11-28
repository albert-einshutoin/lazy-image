# lazy-image 🦀

> **Next-generation image processing engine for Node.js**
> 
> Smaller files. Better quality. Powered by Rust + mozjpeg + AVIF.

[![npm version](https://badge.fury.io/js/@alberteinshutoin/lazy-image.svg)](https://www.npmjs.com/package/@alberteinshutoin/lazy-image)

---

## 📊 Benchmark Results

**vs sharp (libvips + mozjpeg)**

| Format | lazy-image | sharp | Difference |
|--------|-----------|-------|------------|
| **AVIF** | **77,800 bytes** | N/A | 🏆 **Next-gen** |
| **JPEG** | 86,761 bytes | 88,171 bytes | **-1.6%** ✅ |
| **WebP** | 111,334 bytes | 114,664 bytes | **-2.9%** ✅ |
| **Complex Pipeline** | 38,939 bytes | 44,516 bytes | **-12.5%** ✅ |

> *Tested with 23MB PNG input, resize to 800px, quality 60-80*

### AVIF: The Ultimate Compression

```
AVIF vs JPEG: -10.3% smaller
AVIF vs WebP: -30.1% smaller
```

**Translation**: If you serve 1 billion images/month and switch to AVIF, you save **~300GB of bandwidth per month** compared to WebP.

---

## ⚡ Features

- 🏆 **AVIF support** - Next-gen format, 30% smaller than WebP
- 🚀 **Smaller files** than sharp (mozjpeg + libwebp + ravif)
- 🔗 **Fluent API** with method chaining
- 📦 **Lazy pipeline** - operations are queued and executed in a single pass
- 🔄 **Async/Promise-based** - doesn't block the event loop
- 🦀 **Pure Rust core** via NAPI-RS
- 🌍 **Cross-platform** - macOS, Windows, Linux

---

## 📦 Installation

```bash
npm install @alberteinshutoin/lazy-image
```

---

## 🔧 Usage

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');
const fs = require('fs');

// Load image
const buffer = fs.readFileSync('input.png');

// Process with fluent API
const result = await ImageEngine.from(buffer)
  .resize(800, null)     // Width 800, auto height
  .rotate(90)            // Rotate 90°
  .grayscale()           // Convert to grayscale
  .toBuffer('avif', 60); // AVIF quality 60 (smallest!)

fs.writeFileSync('output.avif', result);
```

### Multi-format output

```javascript
const engine = ImageEngine.from(buffer).resize(600, null);

// Generate all formats in parallel
const [jpeg, webp, avif] = await Promise.all([
  engine.clone().toBuffer('jpeg', 80),
  engine.clone().toBuffer('webp', 80),
  engine.clone().toBuffer('avif', 60),
]);
```

---

## 📚 API

### `ImageEngine.from(buffer: Buffer): ImageEngine`

Create a new engine from an image buffer.

### Pipeline Operations (chainable)

| Method | Description |
|--------|-------------|
| `.resize(width?, height?)` | Resize image (maintains aspect ratio if one is null) |
| `.crop(x, y, width, height)` | Crop a region |
| `.rotate(degrees)` | Rotate (90, 180, 270) |
| `.flipH()` | Flip horizontally |
| `.flipV()` | Flip vertically |
| `.grayscale()` | Convert to grayscale |
| `.brightness(value)` | Adjust brightness (-100 to 100) |
| `.contrast(value)` | Adjust contrast (-100 to 100) |

### Output

| Method | Description |
|--------|-------------|
| `.toBuffer(format, quality?)` | Encode to buffer. Format: `'jpeg'`, `'png'`, `'webp'`, `'avif'` |
| `.clone()` | Clone the engine for multi-output |
| `.dimensions()` | Get `{ width, height }` |

---

## 🏎️ Performance Notes

### When to use lazy-image

- ✅ **Build-time optimization** (static site generation, CI/CD)
- ✅ **Batch processing** (thumbnail generation, media pipelines)
- ✅ **Bandwidth-sensitive applications** (CDN, mobile apps)
- ✅ **AVIF generation** (lazy-image has native AVIF support)

### When to use sharp instead

- ⚠️ **Real-time processing** with strict latency requirements (<100ms)

---

## 🔬 Technical Details

### Why smaller files?

1. **mozjpeg** - Progressive mode, optimized Huffman tables, scan optimization
2. **libwebp** - Method 6 (max compression), multi-pass encoding
3. **ravif** - Pure Rust AVIF encoder, AV1-based compression
4. **Chroma subsampling** (4:2:0) forced for web-optimal output
5. **Smoothing/preprocessing** applied before encoding

### Supported Platforms

| Platform | Architecture | Status |
|----------|-------------|--------|
| macOS | x64 (Intel) | ✅ |
| macOS | arm64 (Apple Silicon) | ✅ |
| Windows | x64 | ✅ |
| Linux | x64 (glibc) | ✅ |
| Linux | x64 (musl/Alpine) | ✅ |

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Node.js (JavaScript)                   │
├─────────────────────────────────────────────────────────────┤
│                     NAPI-RS Bridge                          │
├─────────────────────────────────────────────────────────────┤
│                         Rust Core                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────────┐  │
│  │ mozjpeg  │  │ libwebp  │  │  ravif   │  │ fast_image  │  │
│  │ (JPEG)   │  │ (WebP)   │  │ (AVIF)   │  │ _resize     │  │
│  └──────────┘  └──────────┘  └──────────┘  └─────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## 🛠️ Development

```bash
# Install dependencies
npm install

# Build native module
npm run build

# Run tests
npm test
```

### Requirements

- Node.js 18+
- Rust 1.70+ (for building from source)
- nasm (for mozjpeg SIMD)

---

## 📄 License

MIT

---

## 🙏 Credits

Built on the shoulders of giants:

- [mozjpeg](https://github.com/mozilla/mozjpeg) - Mozilla's JPEG encoder
- [libwebp](https://chromium.googlesource.com/webm/libwebp) - Google's WebP codec
- [ravif](https://github.com/nicoptere/ravif) - Pure Rust AVIF encoder
- [fast_image_resize](https://github.com/Cykooz/fast_image_resize) - SIMD-accelerated resizer
- [napi-rs](https://napi.rs/) - Rust bindings for Node.js

---

**Ship it.** 🚀
