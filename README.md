# lazy-image 🦀

> **Next-generation image processing engine for Node.js**
> 
> Smaller files. Better quality. Powered by Rust + mozjpeg.

[![npm version](https://badge.fury.io/js/lazy-image.svg)](https://www.npmjs.com/package/lazy-image)

---

## 📊 Benchmark Results

**vs sharp (libvips + mozjpeg)**

| Format | lazy-image | sharp | Difference |
|--------|-----------|-------|------------|
| **JPEG** | 86,761 bytes | 88,171 bytes | **-1.6%** ✅ |
| **WebP** | 111,334 bytes | 114,664 bytes | **-2.9%** ✅ |
| **Complex Pipeline** | 38,939 bytes | 44,516 bytes | **-12.5%** ✅ |

> *Tested with 23MB PNG input, resize to 800px, quality 75-80*

**Translation**: If you serve 1 billion images/month, lazy-image saves you **~125GB of bandwidth per month** on complex pipelines alone.

---

## ⚡ Features

- 🚀 **Smaller files** than sharp (mozjpeg + libwebp with aggressive optimization)
- 🔗 **Fluent API** with method chaining
- 📦 **Lazy pipeline** - operations are queued and executed in a single pass
- 🔄 **Async/Promise-based** - doesn't block the event loop
- 🦀 **Pure Rust core** via NAPI-RS (no runtime dependencies)

---

## 📦 Installation

```bash
npm install lazy-image
```

---

## 🔧 Usage

```javascript
const { ImageEngine } = require('lazy-image');
const fs = require('fs');

// Load image
const buffer = fs.readFileSync('input.png');

// Process with fluent API
const result = await ImageEngine.from(buffer)
  .resize(800, null)     // Width 800, auto height
  .rotate(90)            // Rotate 90°
  .grayscale()           // Convert to grayscale
  .toBuffer('jpeg', 75); // JPEG quality 75

fs.writeFileSync('output.jpg', result);
```

### Multi-output (clone for different formats)

```javascript
const engine = ImageEngine.from(buffer).resize(600, null);

// Clone for parallel encoding
const [jpeg, webp] = await Promise.all([
  engine.clone().toBuffer('jpeg', 80),
  engine.clone().toBuffer('webp', 80),
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
| `.toBuffer(format, quality?)` | Encode to buffer. Format: `'jpeg'`, `'png'`, `'webp'` |
| `.clone()` | Clone the engine for multi-output |
| `.dimensions()` | Get `{ width, height }` |

---

## 🏎️ Performance Notes

### When to use lazy-image

- ✅ **Build-time optimization** (static site generation, CI/CD)
- ✅ **Batch processing** (thumbnail generation, media pipelines)
- ✅ **Bandwidth-sensitive applications** (CDN, mobile apps)

### When to use sharp instead

- ⚠️ **Real-time processing** with strict latency requirements
- ⚠️ **JPEG input with no resize** (sharp's libjpeg-turbo decode is faster)

---

## 🔬 Technical Details

### Why smaller files?

1. **mozjpeg** with progressive mode, optimized Huffman tables, and scan optimization
2. **Chroma subsampling** (4:2:0) forced for web-optimal output
3. **Smoothing factor** applied before JPEG encoding to reduce noise
4. **libwebp** with method=6 (max compression) and multi-pass encoding

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Node.js (JavaScript)                   │
├─────────────────────────────────────────────────────────────┤
│                     NAPI-RS Bridge                          │
├─────────────────────────────────────────────────────────────┤
│                         Rust Core                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ mozjpeg     │  │ libwebp     │  │ fast_image_resize   │  │
│  │ (JPEG enc)  │  │ (WebP enc)  │  │ (SIMD resize)       │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
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
- [fast_image_resize](https://github.com/Cykooz/fast_image_resize) - SIMD-accelerated resizer
- [napi-rs](https://napi.rs/) - Rust bindings for Node.js

---

**Ship it.** 🚀
