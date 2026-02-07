# Architecture & Technical Details

## Why Smaller Files?

1. **mozjpeg** — Progressive mode, optimized Huffman tables, scan optimization, trellis quantization
2. **libwebp** — Method 4 (balanced), single-pass encoding (v0.8.1+ optimized for speed)
3. **ravif** — Pure Rust AVIF encoder, AV1-based compression
4. **Chroma subsampling** (4:2:0) for web-optimal output
5. **Adaptive preprocessing** — JPEG: compression optimization; WebP (v0.8.1+): speed optimization

## Memory Management (Zero-Copy Architecture)

lazy-image uses a **Copy-on-Write (CoW)** design:

1. **Lazy loading** — `fromPath()` only creates a reference; I/O happens at `toBuffer()`/`toFile()`.
2. **Zero-copy mmap** — `fromPath()` and `processBatch()` use memory mapping; no copy into the Node.js heap.
3. **Zero-copy conversions** — Format conversion without resize/crop reuses the decoded buffer (no extra pixel buffer).
4. **Clone** — `.clone()` is cheap and does not allocate until a destructive op runs.
5. **Verification** — See [ZERO_COPY.md](./ZERO_COPY.md). Run `node --expose-gc docs/scripts/measure-zero-copy.js` to check heap/RSS.
6. **Contract** — Do not modify or delete the source file while it is memory-mapped. On Windows, mapped files cannot be deleted until the engine is dropped.

## Color Management

**AVIF**: ICC preserved in v0.9.0+ (libavif-sys). On &lt;0.9.0 or ravif-only builds, ICC is dropped — convert to sRGB before encoding if needed.

| Format | ICC |
|--------|-----|
| JPEG   | ✅ Extracted and embedded |
| PNG    | ✅ iCCP chunk |
| WebP   | ✅ ICCP chunk |
| AVIF   | ✅ v0.9.0+ libavif-sys; dropped on ravif-only |

## Platform Notes

### Windows file locking

On Windows, memory-mapped files **cannot be deleted** while mapped. Keep the `ImageEngine` in a block and delete the file only after the block ends, or use `ImageEngine.from(buffer)` (heap path) if you must delete immediately. See [TROUBLESHOOTING.md](./TROUBLESHOOTING.md#windows-file-locking). Linux and macOS are not affected.

### Supported platforms

| Platform | Architecture | Status |
|----------|-------------|--------|
| macOS | x64, arm64 | ✅ |
| Windows | x64 | ✅ |
| Linux | x64 (glibc), arm64 (glibc), x64 (musl/Alpine) | ✅ |

## Architecture Diagram

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

## Security

For vulnerability reporting and supported versions, see [SECURITY.md](../SECURITY.md).

### Rust memory safety

- No buffer overflows; no use-after-free; no data races (enforced by the type system).
- Image libraries in C/C++ have had many CVEs; Rust removes whole classes of bugs by design.

### Decompression bomb protection

- Max dimension 32768×32768 by default.
- Progressive decode aborts on invalid data.
- Allocation bounded by the Rust runtime.

### Metadata handling

Defaults are security-first and go beyond typical image libs:

- **Default**: All metadata (EXIF/XMP/ICC) stripped. Use `.keepMetadata({ icc: true, exif: true })` to preserve.
- **GPS**: Stripped by default for privacy; set `stripGps: false` to keep.
- **Orientation**: Auto-reset to 1 to avoid double-rotation.

```typescript
// Default: strip all
await ImageEngine.from(buffer).toBuffer('jpeg');

// Preserve ICC + EXIF, strip GPS
await ImageEngine.from(buffer)
  .keepMetadata({ icc: true, exif: true })
  .toBuffer('jpeg');
```

### Image Firewall mode (strict / lenient)

Input sanitization for untrusted images (decompression bombs, slowloris-style inputs, oversized metadata). Use `.sanitize({ policy: 'strict' | 'lenient' })`.

| Limit       | Strict   | Lenient  | No firewall   |
|------------|----------|----------|----------------|
| Max pixels | 40 MP    | 75 MP    | 1 GP           |
| Max bytes  | 32 MB    | 48 MB    | Unlimited      |
| Timeout    | 5 s      | 30 s     | Unlimited      |
| ICC        | Blocked  | 512 KB   | Allowed        |

Override with `.limits({ maxPixels, maxBytes, timeoutMs })`. Violations throw with clear messages and recovery hints.

**When to use**: User avatars / social uploads → `strict`; admin or internal → `lenient` or none; slow AVIF → `lenient` + custom timeout.
