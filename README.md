# lazy-image 🦀

> **Next-generation image processing engine for Node.js**
> 
> Smaller files. Better quality. Memory-efficient. Powered by Rust + mozjpeg + AVIF.

[![npm version](https://badge.fury.io/js/@alberteinshutoin%2Flazy-image.svg)](https://www.npmjs.com/package/@alberteinshutoin/lazy-image)

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
- 🎨 **ICC color profiles** - Preserves color accuracy (P3, Adobe RGB)
- 💾 **Memory-efficient** - Direct file I/O bypasses Node.js heap
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

### JavaScript

```javascript
const { ImageEngine, inspect, inspectFile } = require('@alberteinshutoin/lazy-image');
const fs = require('fs');

// === Basic Usage ===
const buffer = fs.readFileSync('input.png');

const result = await ImageEngine.from(buffer)
  .resize(800, null)     // Width 800, auto height
  .rotate(90)            // Rotate 90°
  .grayscale()           // Convert to grayscale
  .toBuffer('avif', 60); // AVIF quality 60 (smallest!)

fs.writeFileSync('output.avif', result);

// === Memory-Efficient: File-to-File (Recommended for servers) ===
// Bypasses Node.js heap entirely - no OOM on large images
const bytesWritten = await ImageEngine.fromPath('input.png')
  .resize(800)
  .toFile('output.jpg', 'jpeg', 80);

console.log(`Wrote ${bytesWritten} bytes`);

// === Fast Metadata (no decoding) ===
const meta = inspectFile('input.png');
console.log(meta); // { width: 6000, height: 4000, format: 'png' }

// === ICC Color Profile Check ===
const engine = ImageEngine.from(buffer);
const iccSize = engine.hasIccProfile();
console.log(`ICC profile: ${iccSize ? iccSize + ' bytes' : 'none'}`);
```

### TypeScript

```typescript
import { 
  ImageEngine, 
  inspect, 
  inspectFile,
  ImageMetadata 
} from '@alberteinshutoin/lazy-image';
import { readFileSync, writeFileSync } from 'fs';

// === Basic Usage ===
const buffer: Buffer = readFileSync('input.png');

const result: Buffer = await ImageEngine.from(buffer)
  .resize(800, null)
  .rotate(90)
  .grayscale()
  .toBuffer('avif', 60);

writeFileSync('output.avif', result);

// === Memory-Efficient: File-to-File ===
const bytesWritten: number = await ImageEngine.fromPath('input.png')
  .resize(800)
  .toFile('output.jpg', 'jpeg', 80);

// === Fast Metadata ===
const meta: ImageMetadata = inspectFile('input.png');
// { width: number, height: number, format: string | null }

// === Type-safe Pipeline ===
async function optimizeImage(
  inputPath: string,
  outputPath: string,
  options: { width?: number; quality?: number; format?: 'jpeg' | 'webp' | 'avif' | 'png' }
): Promise<number> {
  const { width = 800, quality = 80, format = 'jpeg' } = options;
  
  return ImageEngine.fromPath(inputPath)
    .resize(width, null)
    .toFile(outputPath, format, quality);
}

await optimizeImage('photo.jpg', 'thumb.webp', { 
  width: 400, 
  quality: 75, 
  format: 'webp' 
});
```

### Multi-format Output

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

### Constructors

| Method | Description |
|--------|-------------|
| `ImageEngine.from(buffer)` | Create engine from a Buffer (loads into V8 heap) |
| `ImageEngine.fromPath(path)` | **Recommended**: Create engine from file path (bypasses V8 heap) |

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
| `.toBuffer(format, quality?)` | Encode to Buffer. Format: `'jpeg'`, `'png'`, `'webp'`, `'avif'` |
| `.toFile(path, format, quality?)` | **Recommended**: Write directly to file (memory-efficient) |
| `.clone()` | Clone the engine for multi-output |

### Utilities

| Method | Description |
|--------|-------------|
| `inspect(buffer)` | Get metadata from Buffer without decoding pixels |
| `inspectFile(path)` | **Recommended**: Get metadata from file without loading into memory |
| `.dimensions()` | Get `{ width, height }` (requires decode) |
| `.hasIccProfile()` | Returns ICC profile size in bytes, or null if none |

### Return Types

```typescript
interface ImageMetadata {
  width: number;
  height: number;
  format: string | null;
}

interface Dimensions {
  width: number;
  height: number;
}
```

---

## 🏎️ Performance Notes

### Memory Efficiency

```javascript
// ❌ BAD: Loads entire file into Node.js heap
const buffer = fs.readFileSync('huge-image.tiff'); // 100MB in V8 heap!
const result = await ImageEngine.from(buffer).resize(800).toBuffer('jpeg', 80);

// ✅ GOOD: Rust reads directly from filesystem
const result = await ImageEngine.fromPath('huge-image.tiff')
  .resize(800)
  .toFile('output.jpg', 'jpeg', 80); // 0 bytes in V8 heap!
```

### When to use lazy-image

- ✅ **Build-time optimization** (static site generation, CI/CD)
- ✅ **Batch processing** (thumbnail generation, media pipelines)
- ✅ **Bandwidth-sensitive applications** (CDN, mobile apps)
- ✅ **AVIF generation** (lazy-image has native AVIF support)
- ✅ **Memory-constrained environments** (512MB containers)
- ✅ **Color-accurate workflows** (ICC profile preservation)

### When to use sharp instead

- ⚠️ **Real-time processing** with strict latency requirements (<100ms)

---

## 🔬 Technical Details

### Why smaller files?

1. **mozjpeg** - Progressive mode, optimized Huffman tables, scan optimization, trellis quantization
2. **libwebp** - Method 6 (max compression), multi-pass encoding, preprocessing
3. **ravif** - Pure Rust AVIF encoder, AV1-based compression
4. **Chroma subsampling** (4:2:0) forced for web-optimal output
5. **Smoothing/preprocessing** applied before encoding

### Color Management

ICC color profiles are automatically:
- **Extracted** from input images (JPEG, PNG, WebP)
- **Preserved** through the processing pipeline
- **Embedded** in output images

This ensures photos from iPhones (P3 color space) or professional cameras (Adobe RGB) maintain their intended colors.

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
│  ┌──────────┐  ┌──────────┐                                 │
│  │img-parts │  │  flate2  │  ← ICC profile handling         │
│  │ (ICC)    │  │ (zlib)   │                                 │
│  └──────────┘  └──────────┘                                 │
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
- [ravif](https://github.com/kornelski/ravif) - Pure Rust AVIF encoder
- [fast_image_resize](https://github.com/Cykooz/fast_image_resize) - SIMD-accelerated resizer
- [img-parts](https://github.com/paolobarbolini/img-parts) - Image container manipulation
- [napi-rs](https://napi.rs/) - Rust bindings for Node.js

---

## 📈 Version History

| Version | Features |
|---------|----------|
| v0.5.0 | Memory-efficient file I/O (`fromPath`, `toFile`, `inspectFile`) |
| v0.4.0 | ICC color profile preservation |
| v0.3.1 | Fast metadata (`inspect`) |
| v0.3.0 | AVIF support |
| v0.2.0 | Cross-platform CI/CD |
| v0.1.0 | Initial release |

---

**Ship it.** 🚀
