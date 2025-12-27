# lazy-image 🦀

> **Next-generation image processing engine for Node.js**
> 
> Smaller files. Better quality. Memory-efficient. Powered by Rust + mozjpeg + AVIF.

[![npm version](https://badge.fury.io/js/@alberteinshutoin%2Flazy-image.svg)](https://www.npmjs.com/package/@alberteinshutoin/lazy-image)
[![codecov](https://codecov.io/gh/albert-einshutoin/lazy-image/branch/main/graph/badge.svg)](https://codecov.io/gh/albert-einshutoin/lazy-image)

---

## 📊 Benchmark Results

**vs sharp (libvips + mozjpeg)**

### File Size Comparison

| Format | lazy-image | sharp | Difference |
|--------|-----------|-------|------------|
| **AVIF** | **77,800 bytes** | 144,700 bytes | **-46.2%** ✅ |
| **JPEG** | 91,437 bytes | 103,566 bytes | **-11.7%** ✅ |
| **WebP** | 115,782 bytes | 114,664 bytes | +1.0% ⚠️ |
| **Complex Pipeline** | 73,956 bytes | 69,786 bytes | +6.0% ⚠️ |

### Processing Speed Comparison

| Format | lazy-image | sharp | Speed Ratio |
|--------|-----------|-------|-------------|
| **AVIF** | 346ms | 381ms | **1.10x faster** ⚡ |
| **JPEG** | 325ms | 185ms | 0.57x slower 🐢 |
| **WebP** | 429ms | 171ms | 0.40x slower 🐢 |
| **Complex Pipeline** | 293ms | 176ms | 0.60x slower 🐢 |

> *Tested with 23MB PNG input, resize to 800px, quality 60-80*

<details>
<summary>📋 Benchmark Test Environment (Click to expand)</summary>

| Item | Version/Spec |
|------|--------------|
| **Node.js** | v22.x |
| **sharp** | 0.34.x |
| **Test Image** | 6000×4000 PNG (23MB) |
| **Output Size** | 800px width (auto height) |
| **Quality** | JPEG: 80, WebP: 80, AVIF: 60 |
| **Platform** | macOS (Apple Silicon) |

**How to reproduce:**
```bash
npm run test:bench:compare
```

> **Note**: Benchmark results may vary depending on the hardware, Node.js version, and sharp version. These results are for reference only.

</details>

### AVIF: The Ultimate Compression

```
AVIF vs JPEG: -14.9% smaller
AVIF vs WebP: -32.8% smaller
AVIF vs sharp AVIF: -46.2% smaller (and 1.10x faster!)
```

**Translation**: If you serve 1 billion images/month and switch to AVIF, you save **~300GB of bandwidth per month** compared to WebP. lazy-image's AVIF encoder produces **46% smaller files** than sharp's AVIF implementation while being **10% faster**.

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

## ⚠️ Limitations

Before choosing lazy-image, please understand these limitations:

| Limitation | Details |
|------------|---------|
| **16-bit images** | Converted to 8-bit during processing (by design for web optimization) |
| **AVIF ICC profiles** | Not preserved (ravif encoder limitation) - use JPEG/WebP for color-critical work |
| **Rotation angles** | Only 90°, 180°, 270° supported (no arbitrary angles) |
| **No filters** | No blur, sharpen, or artistic effects (out of scope) |
| **No animation** | GIF/APNG animation not supported |
| **Processing speed** | Slower than sharp for JPEG/WebP (prioritizes compression over speed) |

> **Note**: These limitations are intentional - lazy-image focuses on **file size optimization**, not feature completeness.
> See [docs/ROADMAP.md](./docs/ROADMAP.md) for the full project scope.

---

## 📦 Installation

```bash
npm install @alberteinshutoin/lazy-image
```

### Package Size

lazy-image uses **platform-specific packages** to minimize download size.  
Only the binary for your platform is downloaded.

| Package | Size | Description |
|---------|------|-------------|
| `@alberteinshutoin/lazy-image` | ~15 KB | Main package (JS + types) |
| `@alberteinshutoin/lazy-image-darwin-arm64` | ~5.7 MB | macOS Apple Silicon |
| `@alberteinshutoin/lazy-image-darwin-x64` | ~8.5 MB | macOS Intel |
| `@alberteinshutoin/lazy-image-win32-x64-msvc` | ~9.1 MB | Windows x64 |
| `@alberteinshutoin/lazy-image-linux-x64-gnu` | ~9.1 MB | Linux x64 (glibc) |
| `@alberteinshutoin/lazy-image-linux-x64-musl` | ~9.1 MB | Linux x64 (musl/Alpine) |

**Total download**: ~6-9 MB (one platform only)

> **Note**: Actual sizes may vary slightly. These are approximate unpacked sizes from npm registry.

### Size Comparison with sharp

> **Note**: sharp's main package (534 KB) is misleading - it requires `@img/sharp-libvips-*` (~16-20 MB) as a separate download.

| Platform | lazy-image | sharp (actual total) |
|----------|------------|---------------------|
| macOS ARM64 | **~5.7 MB** | ~17 MB (534KB + 274KB + 16.1MB) |
| macOS Intel | **~8.5 MB** | ~18 MB |
| Linux x64 | **~9.1 MB** | ~21 MB |
| Windows x64 | **~9.1 MB** | ~15 MB |

**lazy-image is 2-3x smaller** because all dependencies (mozjpeg, libwebp, ravif) are statically linked into a single binary, while sharp requires a separate libvips package.

### Installation

The main package automatically installs the correct platform-specific binary:

```bash
npm install @alberteinshutoin/lazy-image
```

**How it works:**
- The main package (`@alberteinshutoin/lazy-image`) contains only JavaScript and TypeScript definitions
- Platform-specific native binaries are published as separate packages (e.g., `@alberteinshutoin/lazy-image-darwin-arm64`)
- npm automatically installs the correct platform package via `optionalDependencies`

**Publishing:**
- Platform-specific packages are published automatically via CI/CD on tag releases (e.g., `v0.7.7`)
- CI/README changes are tested but don't trigger npm publish (only tag releases do)
- If you encounter installation issues, check [GitHub Actions](https://github.com/albert-einshutoin/lazy-image/actions) to ensure the latest release was successfully published
- All platform packages are published with proper npm token permissions for the `@alberteinshutoin` scope

### Building from Source

For development or if platform packages aren't available:

```bash
npm install
npm run build
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
// Note: Each format has optimal default quality (JPEG: 85, WebP: 80, AVIF: 60)
const [jpeg, webp, avif] = await Promise.all([
  engine.clone().toBuffer('jpeg'),      // Uses default quality 85
  engine.clone().toBuffer('webp'),      // Uses default quality 80
  engine.clone().toBuffer('avif'),      // Uses default quality 60
]);

// Or specify custom quality
const [jpeg2, webp2, avif2] = await Promise.all([
  engine.clone().toBuffer('jpeg', 90), // Custom quality
  engine.clone().toBuffer('webp', 85),
  engine.clone().toBuffer('avif', 70),
]);
```

### Quality Settings (v0.7.2+)

Each format has an optimal default quality based on its compression characteristics:

| Format | Default Quality | Recommended Range | Notes |
|--------|----------------|-------------------|-------|
| **JPEG** | 85 | 70-95 | Higher quality for better detail retention |
| **WebP** | 80 | 70-90 | Balanced quality and file size |
| **AVIF** | 60 | 50-80 | High compression efficiency means lower quality still looks great |

**Why different defaults?**
- **JPEG (85)**: JPEG benefits from higher quality to avoid compression artifacts
- **WebP (80)**: WebP's superior compression allows good quality at 80
- **AVIF (60)**: AVIF's advanced compression means 60 quality often matches JPEG 85 visually

You can always override the default by specifying quality explicitly:
```javascript
await engine.toBuffer('jpeg', 90); // Override default (85 → 90)
```

### Performance Metrics (v0.6.0+)

```javascript
// Get detailed timing information
const { data, metrics } = await ImageEngine.from(buffer)
  .resize(800)
  .toBufferWithMetrics('jpeg', 80);

console.log(metrics);
// {
//   decodeTime: 12.5,   // ms
//   processTime: 8.3,   // ms
//   encodeTime: 45.2,   // ms
//   memoryPeak: 2621440 // bytes
// }
```

### Batch Processing (v0.6.0+)

```javascript
// Process multiple images in parallel with the same operations
// Note: Create an engine just to define operations - no source image needed
const engine = ImageEngine.fromPath('dummy.jpg') // or use any existing image
  .resize(800)
  .grayscale();

// Apply the same operations to multiple files
// Default: uses all CPU cores for parallel processing
const results = await engine.processBatch(
  ['img1.jpg', 'img2.jpg', 'img3.jpg'],
  './output',
  'webp',
  80  // quality (optional, uses format default if omitted)
);

// Control concurrency (v0.7.3+)
// Limit to 4 parallel workers (useful for memory-constrained environments)
const results2 = await engine.processBatch(
  ['img1.jpg', 'img2.jpg', 'img3.jpg'],
  './output',
  'webp',
  80,
  4  // concurrency: number of parallel workers (0 = use CPU cores)
);

results.forEach(r => {
  if (r.success) {
    console.log(`✅ ${r.source} → ${r.outputPath}`);
  } else {
    console.log(`❌ ${r.source}: ${r.error}`);
  }
});
```

### Presets (v0.7.0+)

```javascript
// Use built-in presets for common use cases
const engine = ImageEngine.fromPath('photo.jpg');

// Apply preset and get recommended settings
const preset = engine.preset('thumbnail');
// preset = { format: 'webp', quality: 75, width: 150, height: 150 }

// Use the preset settings
const buffer = await engine.toBuffer(preset.format, preset.quality);

// Available presets:
// - 'thumbnail': 150x150, WebP q75 (gallery thumbnails)
// - 'avatar':    200x200, WebP q80 (profile pictures)
// - 'hero':      1920w,   JPEG q85 (hero images, banners)
// - 'social':    1200x630, JPEG q80 (OGP/Twitter cards)
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
| `.toColorspace(space)` | Convert to color space (`'srgb'`) |
| `.preset(name)` | Apply preset (`'thumbnail'`, `'avatar'`, `'hero'`, `'social'`) |

### Output

| Method | Description |
|--------|-------------|
| `.toBuffer(format, quality?)` | Encode to Buffer. Format: `'jpeg'`, `'png'`, `'webp'`, `'avif'`. Quality defaults: JPEG=85, WebP=80, AVIF=60. If quality is omitted, format-specific default is used. |
| `.toBufferWithMetrics(format, quality?)` | Encode with performance metrics. Returns `{ data: Buffer, metrics: ProcessingMetrics }`. Quality defaults: JPEG=85, WebP=80, AVIF=60 |
| `.toFile(path, format, quality?)` | **Recommended**: Write directly to file (memory-efficient). Returns bytes written. Quality defaults: JPEG=85, WebP=80, AVIF=60 |
| `.processBatch(inputs, outDir, format, quality?, concurrency?)` | Process multiple images in parallel. `inputs`: array of file paths. `concurrency`: number of workers (0 or undefined = CPU cores). Returns array of `BatchResult` |
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

interface PresetResult {
  format: string;
  quality?: number;
  width?: number;
  height?: number;
}

interface ProcessingMetrics {
  decodeTime: number;   // milliseconds
  processTime: number;  // milliseconds
  encodeTime: number;   // milliseconds
  memoryPeak: number;   // bytes
}

interface OutputWithMetrics {
  data: Buffer;
  metrics: ProcessingMetrics;
}

interface BatchResult {
  source: string;
  success: boolean;
  error?: string;
  outputPath?: string;
}
```

---

## ⚠️ Error Handling

lazy-image uses a structured error code system for type-safe error handling. All errors include detailed context and are categorized for easy programmatic handling.

### Error Code System

Errors are organized into categories:

| Category | Range | Description |
|----------|-------|-------------|
| **E1xx** | 100-199 | Input Errors - Issues with input files or data |
| **E2xx** | 200-299 | Processing Errors - Issues during image processing operations |
| **E3xx** | 300-399 | Output Errors - Issues when writing or encoding output |
| **E4xx** | 400-499 | Configuration Errors - Invalid parameters or settings |
| **E9xx** | 900-999 | Internal Errors - Unexpected internal state or bugs |

### Common Error Codes

| Code | Name | Recoverable | Description |
|------|------|-------------|-------------|
| **E100** | FileNotFound | ✅ Yes | File path does not exist |
| **E101** | FileReadFailed | ✅ Yes | I/O error reading file |
| **E111** | UnsupportedFormat | ❌ No | Image format not supported |
| **E121** | DimensionExceedsLimit | ✅ Yes | Image dimension too large |
| **E200** | InvalidCropBounds | ✅ Yes | Crop coordinates exceed image bounds |
| **E201** | InvalidRotationAngle | ✅ Yes | Rotation angle not multiple of 90° |
| **E300** | EncodeFailed | ❌ No | Failed to encode image |
| **E301** | FileWriteFailed | ✅ Yes | I/O error writing file |
| **E401** | InvalidPreset | ✅ Yes | Unknown preset name |

> 📖 **Full Reference**: See [docs/ERROR_CODES.md](./docs/ERROR_CODES.md) for complete error code documentation.

### Handling Errors

#### JavaScript/TypeScript

```javascript
try {
  const result = await ImageEngine.fromFile('input.jpg')
    .resize(800)
    .toBuffer('jpeg', 85);
} catch (error) {
  // Error message includes error code: "[E100] File not found: input.jpg"
  const errorCode = error.message.match(/\[E\d+\]/)?.[0];
  
  if (errorCode === '[E100]') {
    console.error('File not found - check the path');
  } else if (errorCode === '[E200]') {
    console.error('Invalid crop bounds - adjust coordinates');
  } else {
    console.error('Error:', error.message);
  }
}
```

#### Rust

```rust
use lazy_image::error::{ErrorCode, LazyImageError};

match result {
    Ok(data) => println!("Success!"),
    Err(err) => {
        match err.code() {
            ErrorCode::FileNotFound => {
                eprintln!("File not found: {}", err);
            }
            ErrorCode::InvalidCropBounds => {
                eprintln!("Invalid crop bounds: {}", err);
            }
            _ => {
                eprintln!("Error {}: {}", err.code(), err);
            }
        }
        
        // Check if error is recoverable
        if err.code().is_recoverable() {
            // User can fix this - retry with corrected input
        } else {
            // Non-recoverable - log and report
        }
    }
}
```

### Error Recovery

Errors marked as **Recoverable** can be handled programmatically:

- ✅ **Recoverable errors**: User can fix (invalid parameters, file paths, etc.)
- ❌ **Non-recoverable errors**: Indicate corrupted data, bugs, or unsupported formats

Use `ErrorCode::is_recoverable()` to check if an error can be handled programmatically.

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

ICC color profiles are automatically extracted and embedded during processing.

| Format | ICC Profile Support | Notes |
|--------|---------------------|-------|
| JPEG   | ✅ Full support | Extracted and embedded |
| PNG    | ✅ Full support | Via iCCP chunk |
| WebP   | ✅ Full support | Via ICCP chunk |
| AVIF   | ⚠️ **Not supported** | **See warning below** |

> ⚠️ **Important: AVIF Color Space Limitation**
> 
> **AVIF format does NOT preserve ICC color profiles** due to a limitation in the ravif encoder.
> 
> **Impact:**
> - Images with Display P3, Adobe RGB, or other wide-gamut profiles will be converted to sRGB
> - Color accuracy may be affected for professional photography workflows
> 
> **Recommendation:**
> - Use **JPEG or WebP** for color-critical applications
> - AVIF is safe for images already in sRGB color space
> - For maximum compatibility, convert to sRGB before AVIF encoding
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
npm run test:js       # JavaScript specs (custom runner)
npm run test:types    # TypeScript 型チェック
npm run test:bench    # ベンチマークスクリプト
npm run test:rust     # Cargo tests
npm test              # JS + Rust をまとめて実行
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
| v0.7.7 | CI/CD improvements: skip napi prepublish auto-publish, use manual package generation |
| v0.7.6 | Fixed napi prepublish: create skeleton package.json for each platform before running prepublish |
| v0.7.5 | Fixed platform-specific package publishing (robust CI/CD workflow) |
| v0.7.4 | Fixed platform-specific package publishing (CI/CD improvements) |
| v0.7.3 | Batch processing concurrency control (limit parallel workers) |
| v0.7.2 | Format-specific default quality (JPEG: 85, WebP: 80, AVIF: 60) |
| v0.7.1 | Platform-specific packages (reduced download from 42MB to ~6-9MB) |
| v0.7.0 | Built-in presets (`thumbnail`, `avatar`, `hero`, `social`) |
| v0.6.0 | Performance metrics (`toBufferWithMetrics`), batch processing (`processBatch`), color space API, adaptive encoder settings |
| v0.5.0 | Memory-efficient file I/O (`fromPath`, `toFile`, `inspectFile`) |
| v0.4.0 | ICC color profile preservation |
| v0.3.1 | Fast metadata (`inspect`) |
| v0.3.0 | AVIF support |
| v0.2.0 | Cross-platform CI/CD |
| v0.1.0 | Initial release |

---

## 🧭 Project Direction & Roadmap

lazy-image has a focused scope: web image optimization for backends, CDNs, and build pipelines.

See **[docs/ROADMAP.md](./docs/ROADMAP.md)** for:

- Vision & positioning
- In-scope vs out-of-scope features
- High-level version roadmap
- Contribution guidelines (what we accept / reject)

---

**Ship it.** 🚀
