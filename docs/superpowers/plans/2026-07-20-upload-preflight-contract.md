# Authoritative Upload Preflight Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `inspect()` and `inspectFile()` with authoritative alpha, animation, and EXIF orientation facts for static-upload policy decisions without decoding pixels or reading an entire file into the Node.js heap.

**Architecture:** Move header inspection out of `src/lib.rs` into a focused Rust module that selects an existing format-specific header decoder for JPEG, PNG, and WebP. Successful inspection always returns definite booleans for alpha and animation; malformed or unsupported headers fail with the existing typed error taxonomy instead of returning an optimistic value. N-API remains a thin conversion layer, and buffer/path APIs share the same core parser while path orientation uses a second seekable file handle.

**Tech Stack:** Rust, `image` JPEG/PNG/WebP header decoders, `mozjpeg`, `kamadak-exif`, N-API-RS, Node.js integration tests, TypeScript, existing fuzz targets.

---

## Scope and dependency boundary

This plan implements Issue #700 and is the first dependency of the approved design in `docs/superpowers/specs/2026-07-20-public-upload-artifact-compiler-design.md`.

It does not implement the `public-upload` firewall policy (#781), artifact compilation, responsive output, LQIP, CLI changes, or any release operation. The complete compiler program remains split into separate plans so every plan produces independently testable software.

## File responsibility map

| File | Responsibility |
| --- | --- |
| `src/inspect.rs` | Format sniffing, header-only trait extraction, buffer/path parity, and Rust tests |
| `src/lib.rs` | Public Rust re-export and N-API `ImageMetadata` conversion only |
| `src/engine/decoder.rs` | Reusable EXIF orientation reader helper used by decode and inspect paths |
| `test/helpers/png-helpers.js` | Deterministic RGB/RGBA/APNG construction for JS contract tests |
| `test/integration/inspect-preflight.test.js` | Public Node.js behavior and buffer/path parity |
| `test/type-safety/typescript-typecheck.test.ts` | Compile-time field contract |
| `test/benchmarks/inspect-preflight.bench.js` | Reproducible header-inspection timing evidence; not a CI pass/fail gate |
| `docs/API.md` | Public API, meaning of alpha/animation/orientation, and failure behavior |
| `docs/METADATA_SUPPORT.md` | EXIF orientation inspection boundary |
| `FUZZING.md` | Expanded inspect attack surface |
| `CHANGELOG.md` | Additive public API record |

The public result is:

```ts
export interface ImageMetadata {
  width: number;
  height: number;
  format?: InputFormat;
  /** Authoritative for a successfully inspected JPEG, PNG, or WebP. */
  hasAlpha: boolean;
  /** True for APNG or animated WebP; JPEG is always false. */
  isAnimated: boolean;
  /** EXIF Orientation 1-8; undefined when absent or invalid. */
  orientation?: number;
}
```

A successful call never has an unknown alpha or animation state. If a supported container cannot be parsed far enough to determine those facts, inspection rejects with E131/CodecError. This avoids `unknown -> false` coercion in the future compiler.

### Task 1: Make EXIF orientation parsing reusable for seekable readers

**Files:**
- Modify: `src/engine/decoder.rs:454-468`
- Modify: `test/fixtures/create-exif-test-image.js:110-122`
- Generated: `test/fixtures/test_with_exif.jpg`
- Test: `src/engine/decoder.rs` test module

- [ ] **Step 1: Add the failing reader-parity test**

Add this test inside `src/engine/decoder.rs`'s existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_detect_exif_orientation_from_reader_matches_byte_api() {
    let jpeg = include_bytes!("../../test/fixtures/test_with_exif.jpg");
    let mut reader = std::io::BufReader::new(std::io::Cursor::new(jpeg));

    assert_eq!(detect_exif_orientation(jpeg), Some(6));
    assert_eq!(detect_exif_orientation_from_reader(&mut reader), Some(6));
}

#[test]
fn test_detect_exif_orientation_from_reader_rejects_out_of_range_value() {
    let mut invalid = include_bytes!("../../test/fixtures/test_with_exif.jpg").to_vec();
    let orientation_value = invalid
        .windows(4)
        .position(|window| window == [0x06, 0x00, 0x00, 0x00])
        .expect("fixture must contain the little-endian orientation value");
    invalid[orientation_value] = 9;
    let mut reader = std::io::BufReader::new(std::io::Cursor::new(invalid));

    assert_eq!(detect_exif_orientation_from_reader(&mut reader), None);
}
```

- [ ] **Step 2: Run the targeted test and verify the missing function fails compilation**

Run:

```bash
cargo test --no-default-features test_detect_exif_orientation_from_reader -- --nocapture
```

Expected: compilation fails because `detect_exif_orientation_from_reader` is not defined.

- [ ] **Step 3: Extract the reader-based implementation and keep the bytes API as a wrapper**

Replace the existing `detect_exif_orientation()` implementation with:

```rust
/// Extract EXIF Orientation from a seekable container without decoding pixels.
///
/// The reader form exists so `inspectFile()` can parse EXIF through a bounded
/// `BufReader<File>` instead of materializing the full file. Invalid containers,
/// absent tags, and values outside 1-8 all map to `None` because orientation is
/// optional metadata, not proof that the image pixels are decodable.
pub(crate) fn detect_exif_orientation_from_reader<R: std::io::BufRead + std::io::Seek>(
    reader: &mut R,
) -> Option<u16> {
    let exif = exif::Reader::new().read_from_container(reader).ok()?;
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    let orientation = field.value.get_uint(0)? as u16;
    (1..=8).contains(&orientation).then_some(orientation)
}

/// Extract EXIF Orientation tag (1-8). Returns None if missing or invalid.
pub fn detect_exif_orientation(bytes: &[u8]) -> Option<u16> {
    let mut cursor = Cursor::new(bytes);
    detect_exif_orientation_from_reader(&mut cursor)
}
```

- [ ] **Step 4: Run the decoder tests**

Before running the reader parity test, repair the fixture generator's GPS
rational pointer. Its payload begins after the GPS entry count, two entries,
and the next-IFD pointer; the historical `+ 50` offset makes kamadak-exif reject
the entire container as truncated.

```js
const gpsEntriesCount = 2;
// ...
createIfdEntry(
  0x0002,
  5,
  3,
  gpsIfdOffset + 2 + (gpsEntriesCount * 12) + 4,
)
```

Regenerate the committed fixture:

```bash
node test/fixtures/create-exif-test-image.js
```

Expected: a 232-byte JPEG is regenerated and a standards-compliant EXIF reader
can read Orientation `6` without a truncated-field error.

Run:

```bash
cargo test --no-default-features test_detect_exif_orientation -- --nocapture
```

Expected: both new tests and existing orientation consumers pass.

- [ ] **Step 5: Commit the focused refactor**

```bash
git add test/fixtures/create-exif-test-image.js test/fixtures/test_with_exif.jpg
git commit -m "fix(test): generate valid GPS EXIF offsets"
git add src/engine/decoder.rs
git commit -m "refactor(inspect): share seekable orientation parser"
```

### Task 2: Add format-specific, pixel-free inspection with definite traits

**Files:**
- Create: `src/inspect.rs`
- Modify: `src/engine.rs:55-70`
- Modify: `src/lib.rs:30-92`
- Test: `src/inspect.rs`

- [ ] **Step 1: Create the module and write failing format-trait tests**

Create `src/inspect.rs` with the imports, public result type, and the following tests first. The production functions named in the tests intentionally do not exist yet.

```rust
use crate::engine::{detect_exif_orientation_from_reader, run_with_panic_policy};
use crate::error::LazyImageError;
use image::{ImageDecoder, ImageFormat, ImageReader};
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Seek, SeekFrom};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectMetadata {
    pub width: u32,
    pub height: u32,
    pub format: Option<String>,
    pub has_alpha: bool,
    pub is_animated: bool,
    pub orientation: Option<u16>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
    use webp::{AnimEncoder, AnimFrame, WebPConfig};

    fn encode_image(image: DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut encoded = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut encoded), format)
            .expect("test image must encode");
        encoded
    }

    fn animated_webp() -> Vec<u8> {
        let config = WebPConfig::new().expect("libwebp config must initialize");
        let first = vec![255, 0, 0, 255];
        let second = vec![0, 255, 0, 128];
        let mut encoder = AnimEncoder::new(1, 1, &config);
        encoder.add_frame(AnimFrame::from_rgba(&first, 1, 1, 0));
        encoder.add_frame(AnimFrame::from_rgba(&second, 1, 1, 100));
        encoder.encode().to_vec()
    }

    #[test]
    fn jpeg_is_opaque_and_static() {
        let jpeg = encode_image(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(3, 2, Rgb([1, 2, 3]))),
            ImageFormat::Jpeg,
        );
        let metadata = inspect_header_from_bytes(&jpeg).expect("JPEG header must inspect");

        assert_eq!((metadata.width, metadata.height), (3, 2));
        assert_eq!(metadata.format.as_deref(), Some("jpeg"));
        assert!(!metadata.has_alpha);
        assert!(!metadata.is_animated);
    }

    #[test]
    fn rgba_png_reports_alpha_without_decoding_pixels() {
        let png = encode_image(
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 3, Rgba([1, 2, 3, 4]))),
            ImageFormat::Png,
        );
        let metadata = inspect_header_from_bytes(&png).expect("RGBA PNG header must inspect");

        assert_eq!((metadata.width, metadata.height), (2, 3));
        assert!(metadata.has_alpha);
        assert!(!metadata.is_animated);
    }

    #[test]
    fn rgb_png_reports_no_alpha() {
        let png = encode_image(
            DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 3, Rgb([1, 2, 3]))),
            ImageFormat::Png,
        );
        assert!(!inspect_header_from_bytes(&png).unwrap().has_alpha);
    }

    #[test]
    fn animated_webp_reports_animation_and_alpha() {
        let metadata = inspect_header_from_bytes(&animated_webp()).unwrap();
        assert_eq!(metadata.format.as_deref(), Some("webp"));
        assert!(metadata.has_alpha);
        assert!(metadata.is_animated);
    }

    #[test]
    fn malformed_supported_header_returns_typed_decode_failure() {
        let error = inspect_header_from_bytes(b"RIFF\x10\0\0\0WEBPVP8X").unwrap_err();
        assert!(matches!(error, LazyImageError::DecodeFailed { .. }));
    }

    #[test]
    fn orientation_is_reported_without_changing_dimensions() {
        let jpeg = include_bytes!("../test/fixtures/test_with_exif.jpg");
        let metadata = inspect_header_from_bytes(jpeg).unwrap();
        assert_eq!((metadata.width, metadata.height), (8, 8));
        assert_eq!(metadata.orientation, Some(6));
    }
}
```

In `src/lib.rs`, add this private module declaration near the other modules so
the focused implementation compiles without expanding the public module tree:

```rust
#[cfg(any(feature = "napi", feature = "fuzzing", test))]
mod inspect;
```

Do not remove the old `InspectMetadata` implementation yet; the red test only establishes the new module contract.

The new sibling module cannot reach `engine`'s private child modules directly.
After observing the missing-function RED state, add only these crate-private
coordination points in `src/engine.rs`; do not expose either internal module:

```rust
#[cfg(any(feature = "napi", feature = "fuzzing", test))]
pub(crate) use common::run_with_panic_policy;
#[cfg(any(feature = "napi", feature = "fuzzing", test))]
pub(crate) use decoder::detect_exif_orientation_from_reader;
```

- [ ] **Step 2: Run the new module tests and verify the missing API fails**

Run:

```bash
cargo test --no-default-features inspect::tests -- --nocapture
```

Expected: compilation fails because `inspect_header_from_bytes` is not defined in `src/inspect.rs`.

- [ ] **Step 3: Implement the format-specific parser**

Insert the following production code above the test module in `src/inspect.rs`:

```rust
fn decode_failed(context: &str, error: impl std::fmt::Display) -> LazyImageError {
    LazyImageError::decode_failed(format!("{context}: {error}"))
}

fn format_name(format: ImageFormat) -> String {
    format!("{format:?}").to_lowercase()
}

fn inspect_jpeg<R: BufRead>(reader: R) -> Result<(u32, u32, bool, bool), LazyImageError> {
    run_with_panic_policy("inspect:jpeg", || {
        let decoder = mozjpeg::Decompress::new_reader(reader)
            .map_err(|error| decode_failed("failed to read JPEG header", error))?;
        let (width, height) = decoder.size();
        let width = u32::try_from(width)
            .map_err(|error| decode_failed("JPEG width is out of range", error))?;
        let height = u32::try_from(height)
            .map_err(|error| decode_failed("JPEG height is out of range", error))?;
        Ok((width, height, false, false))
    })
}

fn inspect_png<R: BufRead + Seek>(
    reader: R,
) -> Result<(u32, u32, bool, bool), LazyImageError> {
    run_with_panic_policy("inspect:png", || {
        let decoder = image::codecs::png::PngDecoder::new(reader)
            .map_err(|error| decode_failed("failed to read PNG header", error))?;
        let (width, height) = decoder.dimensions();
        let has_alpha = decoder.color_type().has_alpha();
        let is_animated = decoder
            .is_apng()
            .map_err(|error| decode_failed("failed to inspect PNG animation", error))?;
        Ok((width, height, has_alpha, is_animated))
    })
}

fn inspect_webp<R: BufRead + Seek>(
    reader: R,
) -> Result<(u32, u32, bool, bool), LazyImageError> {
    run_with_panic_policy("inspect:webp", || {
        let decoder = image::codecs::webp::WebPDecoder::new(reader)
            .map_err(|error| decode_failed("failed to read WebP header", error))?;
        let (width, height) = decoder.dimensions();
        Ok((
            width,
            height,
            decoder.color_type().has_alpha(),
            decoder.has_animation(),
        ))
    })
}

fn read_inspect_metadata<R: BufRead + Seek>(
    mut reader: R,
) -> Result<InspectMetadata, LazyImageError> {
    let format = {
        let image_reader = ImageReader::new(&mut reader)
            .with_guessed_format()
            .map_err(|error| decode_failed("failed to read image signature", error))?;
        image_reader
            .format()
            .ok_or_else(|| LazyImageError::decode_failed("failed to determine image format"))?
    };

    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| decode_failed("failed to rewind image header", error))?;

    let (width, height, has_alpha, is_animated) = match format {
        ImageFormat::Jpeg => inspect_jpeg(reader)?,
        ImageFormat::Png => inspect_png(reader)?,
        ImageFormat::WebP => inspect_webp(reader)?,
        unsupported => return Err(LazyImageError::unsupported_format(format_name(unsupported))),
    };

    Ok(InspectMetadata {
        width,
        height,
        format: Some(format_name(format)),
        has_alpha,
        is_animated,
        orientation: None,
    })
}

pub fn inspect_header_from_bytes(data: &[u8]) -> Result<InspectMetadata, LazyImageError> {
    let mut metadata = read_inspect_metadata(Cursor::new(data))?;
    let mut orientation_reader = Cursor::new(data);
    metadata.orientation = detect_exif_orientation_from_reader(&mut orientation_reader);
    Ok(metadata)
}

pub fn inspect_header_from_path(path: &str) -> Result<InspectMetadata, LazyImageError> {
    let open = || {
        File::open(path).map_err(|error| LazyImageError::file_read_failed(path.to_string(), error))
    };

    let mut metadata = read_inspect_metadata(BufReader::new(open()?))?;
    let mut orientation_reader = BufReader::new(open()?);
    metadata.orientation = detect_exif_orientation_from_reader(&mut orientation_reader);
    Ok(metadata)
}
```

Why these decoders are used must remain in comments when implementation lands:

- `mozjpeg::Decompress::new_reader()` stops after the JPEG header rather than using `image::JpegDecoder`, whose constructor buffers the complete JPEG.
- `image::PngDecoder` applies palette/`tRNS` expansion when reporting `color_type()`, so indexed transparency is not mislabeled opaque.
- `PngDecoder::is_apng()` and `WebPDecoder::has_animation()` expose container animation state without decoding frames.
- every third-party codec header call remains inside the project panic policy.

- [ ] **Step 4: Replace the old `src/lib.rs` implementation with re-exports**

Delete `src/lib.rs`'s old `ImageReader`, `BufRead`, `BufReader`, `Cursor`, and `Seek` imports and the old `InspectMetadata`, `read_inspect_metadata`, `inspect_header_from_bytes`, and `inspect_header_from_path` definitions.

Add this feature-gated re-export after the module declaration so the historical
crate-root API remains available to N-API and fuzz targets while `inspect`
stays internal. The extra `test` module condition above lets
`cargo test --no-default-features inspect::tests` exercise the core without
creating dead code in a non-test, no-feature build.

```rust
#[cfg(any(feature = "napi", feature = "fuzzing", test))]
pub use inspect::{inspect_header_from_bytes, inspect_header_from_path, InspectMetadata};
```

Keep `napi::bindgen_prelude::*` and all N-API functions in `src/lib.rs`.

- [ ] **Step 5: Run Rust inspection, fuzz-regression, and clippy checks**

Run:

```bash
cargo test --no-default-features inspect::tests -- --nocapture
cargo test --no-default-features --test fuzz_regression -- --nocapture
cargo clippy --no-default-features --all-targets -- -D warnings
```

Expected: all inspection tests pass, fuzz regression returns errors without panic/OOM, and clippy reports no warnings.

- [ ] **Step 6: Commit the new inspection core**

```bash
git add src/inspect.rs src/lib.rs src/engine.rs
git commit -m "feat(inspect): report authoritative image traits"
```

### Task 3: Prove path inspection does not consume complete image payloads

**Files:**
- Modify: `src/inspect.rs`
- Test: `src/inspect.rs`

- [ ] **Step 1: Add a counted-reader regression test**

Add these test-only imports and helper inside `src/inspect.rs`'s test module:

```rust
use std::io::{Read, Seek};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct CountingReader<R> {
    inner: R,
    bytes_read: Arc<AtomicUsize>,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(output)?;
        self.bytes_read.fetch_add(read, Ordering::Relaxed);
        Ok(read)
    }
}

impl<R: std::io::BufRead> std::io::BufRead for CountingReader<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.bytes_read.fetch_add(amount, Ordering::Relaxed);
        self.inner.consume(amount);
    }
}

impl<R: Seek> Seek for CountingReader<R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(position)
    }
}
```

Add the regression test:

```rust
#[test]
fn jpeg_header_inspection_does_not_read_large_scan_payload() {
    let base = encode_image(
        DynamicImage::ImageRgb8(RgbImage::from_pixel(32, 32, Rgb([1, 2, 3]))),
        ImageFormat::Jpeg,
    );
    let mut padded = base;
    let eoi = padded.len() - 2;
    padded.splice(eoi..eoi, std::iter::repeat_n(0u8, 4 * 1024 * 1024));

    let bytes_read = Arc::new(AtomicUsize::new(0));
    let reader = CountingReader {
        inner: BufReader::with_capacity(8 * 1024, Cursor::new(padded)),
        bytes_read: Arc::clone(&bytes_read),
    };
    let metadata = read_inspect_metadata(reader).unwrap();

    assert_eq!((metadata.width, metadata.height), (32, 32));
    assert!(
        bytes_read.load(Ordering::Relaxed) < 64 * 1024,
        "JPEG inspection must stop after bounded header reads"
    );
}
```

- [ ] **Step 2: Run the counted-reader test**

Run:

```bash
cargo test --no-default-features jpeg_header_inspection_does_not_read_large_scan_payload -- --nocapture
```

Expected: the test passes and the counted consumption remains below 64 KiB. The
format-specific JPEG reader is intentionally retained because its header phase
stops before scan data; a failure here is a correctness failure for this plan,
not a threshold to relax.

- [ ] **Step 3: Add buffer/path parity to the Rust API**

Add:

```rust
#[test]
fn path_and_buffer_inspection_return_identical_metadata() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test/fixtures/test_with_exif.jpg");
    let bytes = std::fs::read(&path).unwrap();

    assert_eq!(
        inspect_header_from_path(path.to_str().unwrap()).unwrap(),
        inspect_header_from_bytes(&bytes).unwrap(),
    );
}
```

- [ ] **Step 4: Run the complete inspection module**

Run:

```bash
cargo test --no-default-features inspect::tests -- --nocapture
```

Expected: all tests pass.

- [ ] **Step 5: Commit the bounded-read proof**

```bash
git add src/inspect.rs
git commit -m "test(inspect): bound file header reads"
```

### Task 4: Expose the fields through N-API and generated TypeScript

**Files:**
- Modify: `src/lib.rs:94-115`
- Modify: `test/type-safety/typescript-typecheck.test.ts:68-73`
- Modify: `scripts/postbuild-fixup.js`
- Modify: `test/unit/generated-artifact-policy.test.js`
- Generated: `index.d.ts`

- [ ] **Step 1: Add the TypeScript usage that must fail before the N-API fields exist**

After the existing `ImageMetadata` assignment in `test/type-safety/typescript-typecheck.test.ts`, add:

```ts
    const alpha: boolean = metadata.hasAlpha;
    const animated: boolean = metadata.isAnimated;
    const orientation: number | undefined = metadata.orientation;
    console.log(`Traits: alpha=${alpha}, animated=${animated}, orientation=${orientation}`);
```

- [ ] **Step 2: Run the typecheck and verify the three missing properties fail**

Run:

```bash
npm run test:types
```

Expected: TypeScript reports that `hasAlpha`, `isAnimated`, and `orientation` do not exist on `ImageMetadata`.

- [ ] **Step 3: Extend the N-API object and conversion**

Update `ImageMetadata` and `From<InspectMetadata>` in `src/lib.rs`:

```rust
#[cfg(feature = "napi")]
/// Header-only image traits returned by inspect() and inspectFile().
#[napi(object)]
pub struct ImageMetadata {
    /// Encoded width in pixels; orientation is reported separately.
    pub width: u32,
    /// Encoded height in pixels; orientation is reported separately.
    pub height: u32,
    /// Detected supported input format.
    pub format: Option<String>,
    /// Whether the encoded image has alpha/transparency.
    pub has_alpha: bool,
    /// Whether the container declares more than one animation frame.
    pub is_animated: bool,
    /// EXIF Orientation 1-8, or undefined when absent/invalid.
    pub orientation: Option<u16>,
}

#[cfg(feature = "napi")]
impl From<InspectMetadata> for ImageMetadata {
    fn from(value: InspectMetadata) -> Self {
        Self {
            width: value.width,
            height: value.height,
            format: value.format,
            has_alpha: value.has_alpha,
            is_animated: value.is_animated,
            orientation: value.orientation,
        }
    }
}
```

Update the `inspect()` and `inspectFile()` Rustdoc comments so they say “header/container metadata without pixel decode” and do not claim a universal `<1ms` latency.

- [ ] **Step 4: Build and verify generated declarations**

Run:

```bash
npm run build
rg -n "hasAlpha|isAnimated|orientation" index.d.ts
npm run test:types
```

Expected declarations:

```ts
export interface ImageMetadata {
  width: number
  height: number
  format?: InputFormat
  hasAlpha: boolean
  isAnimated: boolean
  orientation?: number
}
```

Expected: typecheck passes and generated `index.d.ts` retains the existing
`format?: InputFormat` narrowing while adding the three new properties. Update
the postbuild transform to match the new Rustdoc and export the focused helper
behind a `require.main === module` guard:

```js
function patchImageMetadataFormat(content) {
  return replaceIfNeeded(
    content,
    '  /** Detected supported input format. */\n  format?: string',
    '  /** Detected supported input format. */\n  format?: InputFormat',
  );
}
```

In `test/unit/generated-artifact-policy.test.js`, pass a minimal generated
`ImageMetadata` fixture to this helper and assert both the `InputFormat`
narrowing and a thrown `replacement target not found` error for
`format?: unknown`. Run:

```bash
node test/unit/generated-artifact-policy.test.js
```

Expected: the fixture passes and generator drift fails closed.

- [ ] **Step 5: Verify generated artifact provenance**

Run:

```bash
git diff -- index.js index.d.ts
```

Expected before commit: only intended generated `index.d.ts` changes remain;
`index.js` is unchanged. After committing the public type surface, run
`npm run check:generated-artifacts`; it must exit 0.

- [ ] **Step 6: Commit the public type surface**

```bash
git add src/lib.rs test/type-safety/typescript-typecheck.test.ts index.d.ts index.js
git commit -m "feat(inspect): expose alpha animation and orientation"
```

Before committing, omit `index.js` from `git add` when it has no diff.

### Task 5: Add public Node.js contract tests for transparency and animation

**Files:**
- Modify: `test/helpers/png-helpers.js`
- Create: `test/integration/inspect-preflight.test.js`

- [ ] **Step 1: Add deterministic RGBA and APNG builders**

Extend `test/helpers/png-helpers.js` with:

```js
function createRgbaPng(width, height, rgba = [255, 0, 0, 128]) {
    const signature = Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    const ihdr = Buffer.alloc(13);
    ihdr.writeUInt32BE(width, 0);
    ihdr.writeUInt32BE(height, 4);
    ihdr[8] = 8;
    ihdr[9] = 6; // RGBA
    const raw = Buffer.alloc((width * 4 + 1) * height);
    for (let y = 0; y < height; y++) {
        const row = y * (width * 4 + 1);
        raw[row] = 0;
        for (let x = 0; x < width; x++) {
            Buffer.from(rgba).copy(raw, row + 1 + x * 4);
        }
    }
    return Buffer.concat([
        signature,
        pngChunk('IHDR', ihdr),
        pngChunk('IDAT', zlib.deflateSync(raw)),
        pngChunk('IEND', Buffer.alloc(0)),
    ]);
}

function createApng(width = 2, height = 2) {
    const png = createRgbaPng(width, height);
    const afterIhdr = 8 + 12 + 13;
    const animationControl = Buffer.alloc(8);
    animationControl.writeUInt32BE(1, 0); // one animation frame
    animationControl.writeUInt32BE(0, 4); // loop forever
    return Buffer.concat([
        png.subarray(0, afterIhdr),
        pngChunk('acTL', animationControl),
        png.subarray(afterIhdr),
    ]);
}
```

Replace the export line with:

```js
module.exports = {
    buildCrc32Table,
    crc32,
    pngChunk,
    createGrayscalePng,
    createRgbaPng,
    createApng,
};
```

- [ ] **Step 2: Write the public integration test**

Create `test/integration/inspect-preflight.test.js`:

```js
'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const { inspect, inspectFile } = require('../../index');
const { resolveFixture, resolveTemp } = require('../helpers/paths');
const { createApng, createGrayscalePng, createRgbaPng } = require('../helpers/png-helpers');

function assertParity(buffer, fileName) {
    const path = resolveTemp(fileName);
    fs.mkdirSync(require('node:path').dirname(path), { recursive: true });
    fs.writeFileSync(path, buffer);
    assert.deepEqual(inspectFile(path), inspect(buffer));
    fs.rmSync(path, { force: true });
}

const jpeg = fs.readFileSync(resolveFixture('test_with_exif.jpg'));
const jpegMetadata = inspect(jpeg);
assert.deepEqual(jpegMetadata, {
    width: 8,
    height: 8,
    format: 'jpeg',
    hasAlpha: false,
    isAnimated: false,
    orientation: 6,
});
assertParity(jpeg, 'inspect-preflight/oriented.jpg');

const opaquePng = createGrayscalePng(3, 2);
assert.equal(inspect(opaquePng).hasAlpha, false);
assert.equal(inspect(opaquePng).isAnimated, false);
assertParity(opaquePng, 'inspect-preflight/opaque.png');

const alphaPng = createRgbaPng(3, 2);
assert.equal(inspect(alphaPng).hasAlpha, true);
assert.equal(inspect(alphaPng).isAnimated, false);
assertParity(alphaPng, 'inspect-preflight/alpha.png');

const apng = createApng();
assert.equal(inspect(apng).hasAlpha, true);
assert.equal(inspect(apng).isAnimated, true);
assertParity(apng, 'inspect-preflight/animated.png');

assert.throws(
    () => inspect(Buffer.from('RIFF\x10\0\0\0WEBPVP8X', 'binary')),
    /E131|Failed to decode|failed to read WebP header/,
);

console.log('inspect preflight contract tests passed');
```

- [ ] **Step 3: Run the focused JS test**

Run:

```bash
node test/integration/inspect-preflight.test.js
```

Expected: `inspect preflight contract tests passed`.

- [ ] **Step 4: Run all JS integration tests to catch result-shape assumptions**

Run:

```bash
npm run test:js:specs
```

Expected: all integration test files pass. Existing tests that compare the full metadata object must be updated to include the new additive fields; dimension-only assertions need no changes.

- [ ] **Step 5: Commit the public runtime contract**

```bash
git add test/helpers/png-helpers.js test/integration/inspect-preflight.test.js
git commit -m "test(inspect): cover upload preflight traits"
```

### Task 6: Add reproducible inspection evidence without a flaky CI gate

**Files:**
- Create: `test/benchmarks/inspect-preflight.bench.js`

- [ ] **Step 1: Add the benchmark harness**

Create `test/benchmarks/inspect-preflight.bench.js`:

```js
'use strict';

const { performance } = require('node:perf_hooks');
const { inspectFile } = require('../../index');
const { resolveFixture } = require('../helpers/paths');

const cases = [
    ['jpeg-3.2mb', resolveFixture('test_3.2MB_5000x5000.jpg')],
    ['png-4.5mb', resolveFixture('test_4.5MB_5000x5000.png')],
    ['webp-1.4mb', resolveFixture('test_1.4MB_5000x5000.webp')],
];

const iterations = Number(process.env.INSPECT_BENCH_ITERATIONS || 50);
const results = [];

for (const [name, path] of cases) {
    for (let warmup = 0; warmup < 5; warmup++) inspectFile(path);
    const started = performance.now();
    let metadata;
    for (let iteration = 0; iteration < iterations; iteration++) {
        metadata = inspectFile(path);
    }
    const elapsedMs = performance.now() - started;
    results.push({
        name,
        iterations,
        meanMs: Number((elapsedMs / iterations).toFixed(3)),
        width: metadata.width,
        height: metadata.height,
        hasAlpha: metadata.hasAlpha,
        isAnimated: metadata.isAnimated,
    });
}

console.log(JSON.stringify({ schemaVersion: 1, results }, null, 2));
```

- [ ] **Step 2: Capture evidence on the implementation branch**

Run:

```bash
npm run build
INSPECT_BENCH_ITERATIONS=50 node test/benchmarks/inspect-preflight.bench.js
```

Expected: JSON containing three results with dimensions and mean milliseconds. Paste the exact output into the implementation PR. Do not add a fixed millisecond assertion because shared CI runner timing is unstable.

- [ ] **Step 3: Verify path inspection returns metadata rather than image bytes**

Run:

```bash
node -e "
const { inspectFile } = require('./index');
const file = 'test/fixtures/test_3.2MB_5000x5000.jpg';
const metadata = inspectFile(file);
if (Buffer.isBuffer(metadata)) throw new Error('inspectFile returned image bytes');
if (typeof metadata.width !== 'number' || typeof metadata.hasAlpha !== 'boolean') {
  throw new Error('inspectFile did not return authoritative metadata');
}
console.log(JSON.stringify(metadata));
"
```

Expected: exit 0 and a small metadata object. The counted-reader Rust test is
the authoritative proof that large scan payloads are not consumed; this check
only guards the public N-API return shape and avoids a flaky heap threshold.

- [ ] **Step 4: Commit the evidence harness**

```bash
git add test/benchmarks/inspect-preflight.bench.js
git commit -m "bench(inspect): measure header preflight"
```

### Task 7: Document the exact contract and security boundary

**Files:**
- Modify: `docs/API.md:74-83`
- Modify: `docs/METADATA_SUPPORT.md:40-55`
- Modify: `FUZZING.md:15-25`
- Modify: `CHANGELOG.md` under `Unreleased -> Added`

- [ ] **Step 1: Expand the API reference**

Replace the two inspect utility descriptions in `docs/API.md` with:

```markdown
| `inspect(buffer)` | Inspect encoded dimensions, format, alpha, animation, and EXIF orientation without decoding pixels. Buffer bytes already reside in V8 and cross into Rust. |
| `inspectFile(path)` | **Recommended for servers**: inspect the same traits through bounded header/container reads without returning image bytes to V8. |
```

Add this subsection after the Utilities table:

```markdown
### Upload preflight metadata

`inspect()` and `inspectFile()` return `{ width, height, format, hasAlpha,
isAnimated, orientation? }` for supported JPEG, PNG, and WebP containers.
JPEG is always opaque and static. PNG transparency includes explicit alpha and
palette/`tRNS` transparency. `isAnimated` detects APNG and animated WebP.
`orientation` is the encoded EXIF Orientation value from 1 through 8; dimensions
remain encoded dimensions and are not swapped by inspection.

A successful inspection has definite alpha and animation values. A malformed or
indeterminate supported header rejects with the normal typed decode error instead
of returning `false`. Header preflight is not a security substitute for the
authoritative decode and Image Firewall checks performed during output.
```

- [ ] **Step 2: Document orientation as inspection, not preservation**

Add to `docs/METADATA_SUPPORT.md` under Important Caveats:

```markdown
- `inspect()` / `inspectFile()` may report EXIF Orientation without preserving
  EXIF in output. Inspection is read-only; normal output still strips metadata
  unless `keepMetadata()` is explicitly requested. Auto-orientation applies the
  value to pixels during processing and resets/removes the emitted tag according
  to the output metadata policy.
```

- [ ] **Step 3: Update fuzz-surface documentation**

Update the `inspect_header` row in `FUZZING.md` so it explicitly lists JPEG, PNG, and WebP dimension, alpha, animation, and orientation parsing. Keep `inspect_header_from_bytes` in the listed entrypoints.

- [ ] **Step 4: Add the changelog entry**

Under `CHANGELOG.md`'s Unreleased `Added` section, add:

```markdown
- `inspect()` and `inspectFile()` now report authoritative `hasAlpha`,
  `isAnimated`, and optional EXIF `orientation` fields for supported upload
  preflight without decoding pixels.
```

- [ ] **Step 5: Run documentation and generated-contract tests**

Run:

```bash
npm run test:js:specs
npm run test:generated-artifact-policy
git diff --check
```

Expected: all tests pass and no whitespace errors are reported.

- [ ] **Step 6: Commit documentation**

```bash
git add docs/API.md docs/METADATA_SUPPORT.md FUZZING.md CHANGELOG.md
git commit -m "docs(inspect): define upload preflight semantics"
```

### Task 8: Complete security, fuzz, build, and package verification

**Files:**
- Verify all files changed by Tasks 1-7
- Update only failures caused by this feature; do not suppress checks

- [ ] **Step 1: Run format and static analysis**

```bash
cargo fmt --check
cargo clippy --no-default-features --all-targets -- -D warnings
```

Expected: both commands exit 0. If formatting fails, run `cargo fmt`, inspect the exact diff, and commit only formatting produced for this feature.

- [ ] **Step 2: Run Rust and fuzz regression tests**

```bash
cargo test --no-default-features
cargo test --no-default-features --features fuzzing --test fuzz_regression
```

Expected: all tests pass without panic, OOM, or ignored new trait cases.

- [ ] **Step 3: Run the required project verification**

```bash
npm run build
npm test
```

Expected: native build and all JS, Rust, type, package, example, and Wasm tests pass.

- [ ] **Step 4: Run dependency security checks**

```bash
npm audit --audit-level=high
cargo audit --locked
```

Expected: npm reports zero high/critical vulnerabilities. Cargo audit reports no unapproved vulnerability; the repository's documented unmaintained-warning exception, if still current, must be reported rather than hidden.

- [ ] **Step 5: Inspect the final diff and public surface**

```bash
git diff origin/main...HEAD --check
git diff origin/main...HEAD --stat
git diff origin/main...HEAD -- src/inspect.rs src/lib.rs src/engine/decoder.rs index.d.ts
```

Expected: no unrelated changes, no direct dependency additions, and no behavior changes to encode/output paths.

- [ ] **Step 6: Resolve verification findings in their owning task**

If verification exposes a defect, return to the task that owns the affected
file, add a failing regression test, apply the smallest fix, rerun that task's
commands, and use that task's explicit `git add` and commit command. Do not
create a catch-all verification commit or an empty commit.

- [ ] **Step 7: Push and create the implementation PR**

```bash
git push -u origin feat/700-upload-preflight
gh pr create --base main --head feat/700-upload-preflight \
  --title "feat(inspect): add authoritative upload preflight traits" \
  --body "$(cat <<'EOF'
## Why

The upload compiler must choose JPEG only when alpha is authoritatively absent.
The previous inspection contract exposed dimensions and format but could not
distinguish static/animated or opaque/alpha-bearing input, making an unsafe
unknown-to-false fallback tempting.

## What changed

- move inspection into a focused Rust module with format-specific header readers;
- add definite `hasAlpha` and `isAnimated` facts plus optional EXIF `orientation`;
- keep buffer and file inspection on the same parser contract;
- prove JPEG path inspection stops before a large scan payload;
- extend JS, TypeScript, fuzz, benchmark, and documentation coverage.

## Before / after

Before: `{ width, height, format }`

After: `{ width, height, format, hasAlpha, isAnimated, orientation? }`

The fields are additive. Malformed or unsupported headers reject through the
existing typed error taxonomy instead of returning optimistic trait values.

## Verification

- `cargo fmt --check`
- `cargo clippy --no-default-features --all-targets -- -D warnings`
- `cargo test --no-default-features`
- `cargo test --no-default-features --features fuzzing --test fuzz_regression`
- `npm run build`
- `npm test`
- `npm audit --audit-level=high`
- `cargo audit --locked`
- counted-reader regression included; benchmark JSON posted as a PR comment

Closes #700
Depends on the approved compiler design in #780.
Unblocks #768, #781, and #703.
EOF
)"
```

The PR body must include:

- why `unknown -> false` is unsafe for JPEG selection;
- why format-specific header decoders were chosen;
- before/after API examples;
- counted-reader evidence and benchmark JSON;
- exact build, test, fuzz, and audit commands;
- compatibility statement that fields are additive;
- `Closes #700` and dependency links to #768, #781, and #703.

The here-document passes the complete body through standard input expansion, so
no temporary PR file is committed. After creation, attach the exact benchmark
output without manually transcribing it:

```bash
gh pr comment feat/700-upload-preflight --body "$(INSPECT_BENCH_ITERATIONS=50 node test/benchmarks/inspect-preflight.bench.js)"
```

## Plan self-review record

- Spec coverage: orientation, authoritative alpha, animation, buffer/path parity, no pixel decode, bounded file reads, types, docs, fuzzing, security, and complete verification each map to explicit tasks.
- Placeholder scan: the plan contains no `TBD`, `TODO`, “similar to”, unspecified error handling, or unspecified test requests.
- Type consistency: Rust `has_alpha`/`is_animated`/`orientation` consistently become JS/TS `hasAlpha`/`isAnimated`/`orientation`; `InspectMetadata` is defined once in `src/inspect.rs` and re-exported at the historical crate root.
- Scope control: #781 policy semantics and artifact compiler implementation remain separate plans.
- Execution correction: the existing GPS EXIF fixture used an invalid rational
  offset and was repaired at its generator before reader parity was accepted;
  sibling-module visibility and pre-commit generated-artifact checks were also
  corrected above after their fail-closed tests exposed the original assumptions.
