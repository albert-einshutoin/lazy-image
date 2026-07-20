// src/engine/decoder.rs
//
// Decoder operations: JPEG (mozjpeg), PNG, WebP, etc.

use crate::engine::common::run_with_panic_policy;
use crate::error::LazyImageError;
use exif;
use image::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, ImageFormat, ImageReader, RgbImage,
    RgbaImage,
};
use mozjpeg::Decompress;
use std::io::Cursor;
#[cfg(any(feature = "napi", feature = "fuzzing", test))]
use std::io::{self, BufRead, Read, Seek, SeekFrom};
use webp::{BitstreamFeatures, Decoder as WebPDecoder};
use zune_png::zune_core::bytestream::ZCursor;
use zune_png::zune_core::colorspace::ColorSpace;
use zune_png::zune_core::options::DecoderOptions;
use zune_png::zune_core::result::DecodingResult;
use zune_png::PngDecoder;

// Type alias for Result - always use LazyImageError to preserve error taxonomy
// This ensures that decode errors are properly classified (CodecError, ResourceLimit, etc.)
// rather than being converted to generic InternalBug errors.
type DecoderResult<T> = std::result::Result<T, LazyImageError>;

// decode() function removed - it was unused.
// tasks.rs::EncodeTask::decode() and stress.rs::run_stress_iteration() have their own implementations.

/// Decode JPEG using mozjpeg (backed by libjpeg-turbo)
/// This is SIGNIFICANTLY faster than image crate's pure Rust decoder
pub fn decode_jpeg_mozjpeg(data: &[u8]) -> DecoderResult<DynamicImage> {
    run_with_panic_policy("decode:mozjpeg", || {
        // EOI marker (0xFF 0xD9) is at the end of valid JPEGs.
        // Only check the last 256 bytes for O(1) performance instead of O(n).
        // See: https://github.com/albert-einshutoin/lazy-image/issues/332
        const EOI_CHECK_LEN: usize = 256;
        let check_start = data.len().saturating_sub(EOI_CHECK_LEN);
        let tail = &data[check_start..];
        if !tail.windows(2).any(|pair| pair == [0xFF, 0xD9]) {
            return Err(LazyImageError::decode_failed(
                "mozjpeg: missing JPEG EOI marker",
            ));
        }

        validate_jpeg_structure(data)?;

        let decompress = Decompress::new_mem(data).map_err(|e| {
            LazyImageError::decode_failed(format!("mozjpeg decompress init failed: {e:?}"))
        })?;

        // Get image info
        let mut decompress = decompress.rgb().map_err(|e| {
            LazyImageError::decode_failed(format!("mozjpeg rgb conversion failed: {e:?}"))
        })?;

        let width = decompress.width();
        let height = decompress.height();

        let width_u32 = width as u32;
        let height_u32 = height as u32;
        check_dimensions(width_u32, height_u32)?;

        // Read all scanlines
        let pixels: Vec<[u8; 3]> = decompress.read_scanlines().map_err(|e| {
            LazyImageError::decode_failed(format!("mozjpeg: failed to read scanlines: {e:?}"))
        })?;

        // Zero-copy reinterpretation of Vec<[u8; 3]> as Vec<u8> — avoids the
        // intermediate iterator + reallocation that `flatten().collect()` performs.
        let flat_pixels: Vec<u8> = pixels.into_flattened();

        // Create DynamicImage from raw RGB data
        let rgb_image =
            RgbImage::from_raw(width_u32, height_u32, flat_pixels).ok_or_else(|| {
                LazyImageError::decode_failed("mozjpeg: failed to create image from raw data")
            })?;

        Ok(DynamicImage::ImageRgb8(rgb_image))
    })
}

/// Minimal JPEG structure validation to avoid handing obviously malformed buffers to libjpeg.
/// Walks markers until SOS, ensuring declared segment lengths stay within the buffer.
///
/// Also used by `io.rs` to gate ICC extraction from JPEG APP2 segments.
pub(crate) fn validate_jpeg_structure(data: &[u8]) -> DecoderResult<()> {
    const SOI: u8 = 0xD8;
    const EOI: u8 = 0xD9;
    const SOS: u8 = 0xDA;

    if data.len() < 4 || data[0] != 0xFF || data[1] != SOI {
        return Err(LazyImageError::decode_failed("mozjpeg: missing SOI marker"));
    }

    let mut i = 2; // position after SOI
    while i + 1 < data.len() {
        if data[i] != 0xFF {
            return Err(LazyImageError::decode_failed(
                "mozjpeg: expected marker prefix 0xFF",
            ));
        }
        // Skip padding FF bytes
        while i < data.len() && data[i] == 0xFF {
            i += 1;
        }
        if i >= data.len() {
            return Err(LazyImageError::decode_failed(
                "mozjpeg: truncated marker after 0xFF padding",
            ));
        }
        let marker = data[i];
        i += 1;

        // Standalone markers without length
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 || marker == EOI {
            if marker == EOI {
                return Ok(());
            }
            continue;
        }

        if i + 1 >= data.len() {
            return Err(LazyImageError::decode_failed(
                "mozjpeg: truncated segment length",
            ));
        }
        let seg_len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if seg_len < 2 {
            return Err(LazyImageError::decode_failed(
                "mozjpeg: invalid segment length (<2)",
            ));
        }
        if i + seg_len > data.len() {
            return Err(LazyImageError::decode_failed(
                "mozjpeg: segment length exceeds buffer",
            ));
        }

        i += seg_len;

        if marker == SOS {
            // After SOS, compressed scan follows until EOI; require at least 1 byte of scan data.
            if i >= data.len() {
                return Err(LazyImageError::decode_failed(
                    "mozjpeg: SOS without scan data",
                ));
            }
            return Ok(()); // stop validation; hand off to decoder
        }
    }

    Err(LazyImageError::decode_failed(
        "mozjpeg: no EOI or SOS found while validating",
    ))
}

/// Build image crate `Limits` from our global dimension/pixel constants.
/// This prevents the image crate from allocating unbounded memory even when
/// `ensure_dimensions_safe()` cannot parse the header (unknown format, truncated header, etc.).
fn image_crate_limits() -> image::Limits {
    #[cfg(feature = "fuzzing")]
    let (max_dim, max_pix) = (
        crate::engine::FUZZ_MAX_DIMENSION,
        crate::engine::FUZZ_MAX_PIXELS,
    );
    #[cfg(not(feature = "fuzzing"))]
    let (max_dim, max_pix) = (crate::engine::MAX_DIMENSION, crate::engine::MAX_PIXELS);

    let mut limits = image::Limits::default();
    limits.max_image_width = Some(max_dim);
    limits.max_image_height = Some(max_dim);
    // max_alloc caps total decoded buffer size (pixels × bytes-per-pixel).
    // Use 4 bytes/pixel (RGBA) as worst case.
    limits.max_alloc = Some(max_pix * 4);
    limits
}

/// Decode non-JPEG formats using the image crate under the global panic policy.
pub fn decode_with_image_crate(data: &[u8]) -> DecoderResult<DynamicImage> {
    run_with_panic_policy("decode:image", || {
        // Avoid routing JPEG data to the image crate: its JPEG backend (zune-jpeg)
        // can panic in parallel worker threads under malformed inputs when built
        // with `panic = "abort"` (cargo-fuzz). JPEGs should be handled by
        // decode_jpeg_mozjpeg, so bail out early to keep fuzzing stable.
        if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
            return Err(LazyImageError::decode_failed(
                "jpeg: use mozjpeg decoder path",
            ));
        }

        // WebP: route to libwebp to avoid image crate OOM on malformed inputs.
        if is_webp_riff(data) {
            return decode_webp_libwebp(data);
        }

        // Pre-check dimensions to prevent OOM from decompression bombs (e.g., WebP with huge dimensions).
        // This reads only headers and is much cheaper than a full decode attempt.
        ensure_dimensions_safe(data)?;

        // Use ImageReader with explicit allocation limits as a safety net.
        // Even if ensure_dimensions_safe() returned Ok (e.g., unknown format, truncated
        // header), the image crate will enforce dimension and allocation caps during
        // decode, preventing OOM from crafted inputs with huge declared dimensions.
        let cursor = Cursor::new(data);
        let reader = ImageReader::new(cursor)
            .with_guessed_format()
            .map_err(|e| LazyImageError::decode_failed(format!("decode failed: {e}")))?;
        let mut reader = reader;
        reader.limits(image_crate_limits());
        reader
            .decode()
            .map_err(|e| LazyImageError::decode_failed(format!("decode failed: {e}")))
    })
}

/// zune-png's internal dimension limit (16,384px per axis).
///
/// This is a library-imposed constraint that cannot be changed from outside.
/// Images exceeding this limit but still within [`MAX_DIMENSION`](crate::engine::MAX_DIMENSION)
/// are decoded via the image crate fallback path, which applies `check_dimensions()` after decode.
///
/// **Safety invariant**: `ZUNE_PNG_MAX_DIM` ≤ `MAX_DIMENSION`. If zune-png rejects an image,
/// the fallback path is guaranteed to accept it (up to `MAX_DIMENSION`). A compile-time
/// assertion below enforces this.
const ZUNE_PNG_MAX_DIM: u32 = 16384;

// Compile-time assertion: zune-png's limit must not exceed the global MAX_DIMENSION.
// If it did, zune-png could accept images that the global check_dimensions() would reject,
// creating a decompression bomb bypass in the zune-png path.
const _: () = assert!(
    ZUNE_PNG_MAX_DIM <= crate::engine::MAX_DIMENSION,
    "ZUNE_PNG_MAX_DIM must be <= MAX_DIMENSION to ensure consistent dimension enforcement"
);

/// Decode PNG using zune-png (SIMD-optimized decoder). 16-bit input is downsampled to 8-bit.
/// Falls back to image crate if:
/// - Image dimensions exceed zune-png's limit (16,384px)
/// - Unsupported colorspace (e.g., YCbCr)
/// - Decode fails for any reason
pub fn decode_png_zune(data: &[u8]) -> DecoderResult<DynamicImage> {
    run_with_panic_policy("decode:png", || {
        // Try to read dimensions from PNG header without full decode
        if let Some((width, height)) = read_png_dimensions(data) {
            // Global safety check (avoid decompression bombs)
            check_dimensions(width, height)?;

            if width > ZUNE_PNG_MAX_DIM || height > ZUNE_PNG_MAX_DIM {
                // Exceeds zune-png limit but still within MAX_DIMENSION - fallback
                return Err(LazyImageError::decode_failed(
                    "png: dimensions exceed zune-png limit, fallback to image crate",
                ));
            }
        }

        let options = DecoderOptions::default().png_set_strip_to_8bit(true);
        let mut decoder = PngDecoder::new_with_options(ZCursor::new(data), options);
        let pixels = decoder
            .decode()
            .map_err(|e| LazyImageError::decode_failed(format!("png: decode failed: {e}")))?;

        let (width_usize, height_usize) = decoder
            .dimensions()
            .ok_or_else(|| LazyImageError::decode_failed("png: missing header info"))?;
        let width = width_usize as u32;
        let height = height_usize as u32;
        check_dimensions(width, height)?;

        let buf = match pixels {
            DecodingResult::U8(v) => v,
            _ => {
                return Err(LazyImageError::decode_failed(
                    "png: unexpected non-U8 pixel buffer",
                ))
            }
        };

        let colorspace = decoder
            .colorspace()
            .ok_or_else(|| LazyImageError::decode_failed("png: missing colorspace"))?;

        let img = match colorspace {
            ColorSpace::RGB => RgbImage::from_raw(width, height, buf)
                .map(DynamicImage::ImageRgb8)
                .ok_or_else(|| LazyImageError::decode_failed("png: failed to build RGB image"))?,
            ColorSpace::RGBA => RgbaImage::from_raw(width, height, buf)
                .map(DynamicImage::ImageRgba8)
                .ok_or_else(|| LazyImageError::decode_failed("png: failed to build RGBA image"))?,
            ColorSpace::Luma => GrayImage::from_raw(width, height, buf)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| LazyImageError::decode_failed("png: failed to build Luma image"))?,
            ColorSpace::LumaA => GrayAlphaImage::from_raw(width, height, buf)
                .map(DynamicImage::ImageLumaA8)
                .ok_or_else(|| LazyImageError::decode_failed("png: failed to build LumaA image"))?,
            // Fallback to image crate for unsupported ordering/space
            ColorSpace::BGRA | ColorSpace::ARGB => {
                return Err(LazyImageError::decode_failed(
                    "png: BGRA/ARGB not supported by zune-png, fallback to image crate",
                ));
            }
            // YCbCr is 3-channel, not compatible with RGBA (4-channel)
            // Fallback to image crate for proper handling
            ColorSpace::YCbCr | ColorSpace::YCCK => {
                return Err(LazyImageError::decode_failed(
                    "png: YCbCr colorspace not supported by zune-png, fallback to image crate",
                ));
            }
            other => {
                return Err(LazyImageError::decode_failed(format!(
                    "png: unsupported colorspace {other:?}, fallback to image crate"
                )))
            }
        };

        Ok(img)
    })
}

/// Read PNG dimensions from header without full decode.
/// Returns (width, height) if the header is valid.
fn read_png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    // PNG signature: 89 50 4E 47 0D 0A 1A 0A
    if data.len() < 24 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    // First chunk (IHDR) starts at offset 8
    // Width: bytes 16-19, Height: bytes 20-23 (big-endian)
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Some((width, height))
}

/// Decode WebP using libwebp (via webp crate). Falls back to image crate for animated WebP.
pub fn decode_webp_libwebp(data: &[u8]) -> DecoderResult<DynamicImage> {
    run_with_panic_policy("decode:webp", || {
        // Parse header first to avoid allocating huge buffers on malformed files
        let features = BitstreamFeatures::new(data).ok_or_else(|| {
            LazyImageError::decode_failed("webp: failed to read bitstream features")
        })?;

        if features.has_animation() {
            // libwebp simple decoder in this crate does not support animation; keep compatibility via fallback
            return image::load_from_memory(data).map_err(|e| {
                LazyImageError::decode_failed(format!("webp (animated) decode failed: {e}"))
            });
        }

        let width = features.width();
        let height = features.height();
        check_dimensions(width, height)?;

        let decoder = WebPDecoder::new(data);
        let decoded = decoder
            .decode()
            .ok_or_else(|| LazyImageError::decode_failed("webp: decode failed"))?;

        // Defensive: ensure actual decoded size is also within limits
        check_dimensions(decoded.width(), decoded.height())?;

        Ok(decoded.to_image())
    })
}

/// Detect input format using magic bytes. Returns None if unknown.
pub fn detect_format(bytes: &[u8]) -> Option<ImageFormat> {
    image::guess_format(bytes).ok()
}

/// Unified decode entrypoint:
/// - Detect format once (magic bytes)
/// - Route JPEG to mozjpeg, others to image crate
/// - Return decoded image and detected format
pub fn decode_image(bytes: &[u8]) -> DecoderResult<(DynamicImage, Option<ImageFormat>)> {
    let detected = detect_format(bytes);
    let img = match detected {
        Some(ImageFormat::Jpeg) => decode_jpeg_mozjpeg(bytes)?,
        Some(ImageFormat::Png) => {
            // Try zune-png first, fallback to image crate on failure
            let decoded = match decode_png_zune(bytes) {
                Ok(img) => img,
                Err(_) => {
                    // Fallback to image crate for compatibility (e.g., >16k dimensions, YCbCr)
                    decode_with_image_crate(bytes)?
                }
            };
            // Enforce global dimension limits even after fallback
            let (w, h) = decoded.dimensions();
            check_dimensions(w, h)?;
            decoded
        }
        Some(ImageFormat::WebP) => decode_webp_libwebp(bytes)?,
        _ => decode_with_image_crate(bytes)?,
    };
    check_dimensions(img.width(), img.height())?;
    Ok((img, detected))
}

/// Check if image dimensions are within safe limits.
/// Returns an error if the image is too large (potential decompression bomb).
/// In fuzz builds, uses stricter limits (FUZZ_MAX_DIMENSION / FUZZ_MAX_PIXELS) so that
/// decode never exceeds a small memory budget and CI stays under 2GB RSS.
pub fn check_dimensions(width: u32, height: u32) -> DecoderResult<()> {
    #[cfg(feature = "fuzzing")]
    let (max_dim, max_pix) = (
        crate::engine::FUZZ_MAX_DIMENSION,
        crate::engine::FUZZ_MAX_PIXELS,
    );
    #[cfg(not(feature = "fuzzing"))]
    let (max_dim, max_pix) = (crate::engine::MAX_DIMENSION, crate::engine::MAX_PIXELS);

    if width > max_dim || height > max_dim {
        return Err(LazyImageError::dimension_exceeds_limit(
            width.max(height),
            max_dim,
        ));
    }
    let pixels = width as u64 * height as u64;
    if pixels > max_pix {
        return Err(LazyImageError::pixel_count_exceeds_limit(pixels, max_pix));
    }
    Ok(())
}

/// Inspect encoded bytes and ensure the image dimensions are safe before decoding.
pub fn ensure_dimensions_safe(bytes: &[u8]) -> DecoderResult<()> {
    // WebP: parse bitstream features directly to avoid image crate OOM on malformed headers.
    if is_webp_riff(bytes) {
        let features = BitstreamFeatures::new(bytes).ok_or_else(|| {
            LazyImageError::decode_failed("webp: failed to read bitstream features")
        })?;
        return check_dimensions(features.width(), features.height());
    }

    // PNG: read header directly when available to avoid full decode.
    if let Some((width, height)) = read_png_dimensions(bytes) {
        return check_dimensions(width, height);
    }

    let cursor = Cursor::new(bytes);
    if let Ok(reader) = ImageReader::new(cursor).with_guessed_format() {
        return reader
            .into_dimensions()
            .map_err(|_| {
                LazyImageError::decode_failed("decode failed: could not read image dimensions")
            })
            .and_then(|(width, height)| check_dimensions(width, height));
    }
    Ok(())
}

fn is_webp_riff(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

#[cfg(any(feature = "napi", feature = "fuzzing", test))]
const MAX_ORIENTATION_SCAN_BYTES: u64 = 64 * 1024;

/// Presents a seekable reader as a smaller virtual stream.
///
/// `kamadak-exif` scans container chunks until it finds EXIF and otherwise may
/// consume image payloads. Inspection only needs a best-effort Orientation tag,
/// so cap that scan at the documented 64 KiB preflight budget. The wrapper also
/// constrains seeks, preventing a container parser from bypassing the budget.
#[cfg(any(feature = "napi", feature = "fuzzing", test))]
struct LimitedSeekReader<R> {
    inner: R,
    start: u64,
    position: u64,
    limit: u64,
}

#[cfg(any(feature = "napi", feature = "fuzzing", test))]
impl<R: Seek> LimitedSeekReader<R> {
    fn new(mut inner: R, limit: u64) -> io::Result<Self> {
        let start = inner.stream_position()?;
        Ok(Self {
            inner,
            start,
            position: 0,
            limit,
        })
    }

    fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.position)
    }
}

#[cfg(any(feature = "napi", feature = "fuzzing", test))]
impl<R: Read + Seek> Read for LimitedSeekReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let allowed = usize::try_from(self.remaining().min(output.len() as u64))
            .expect("allowed read length is bounded by the output slice");
        if allowed == 0 {
            return Ok(0);
        }
        let read = self.inner.read(&mut output[..allowed])?;
        self.position += read as u64;
        Ok(read)
    }
}

#[cfg(any(feature = "napi", feature = "fuzzing", test))]
impl<R: BufRead + Seek> BufRead for LimitedSeekReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let allowed = usize::try_from(self.remaining()).unwrap_or(usize::MAX);
        let buffer = self.inner.fill_buf()?;
        Ok(&buffer[..buffer.len().min(allowed)])
    }

    fn consume(&mut self, amount: usize) {
        let consumed = usize::try_from(self.remaining().min(amount as u64))
            .expect("consumed length is bounded by the requested amount");
        self.inner.consume(consumed);
        self.position += consumed as u64;
    }
}

#[cfg(any(feature = "napi", feature = "fuzzing", test))]
impl<R: Seek> Seek for LimitedSeekReader<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let target = match position {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.limit) + i128::from(offset),
        };
        if target < 0 || target > i128::from(self.limit) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek exceeds orientation scan budget",
            ));
        }
        let target = u64::try_from(target).expect("validated non-negative seek target");
        let absolute = self.start.checked_add(target).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "orientation seek overflow")
        })?;
        self.inner.seek(SeekFrom::Start(absolute))?;
        self.position = target;
        Ok(target)
    }
}

/// Extract EXIF Orientation from a seekable container without decoding pixels.
///
/// This bounded reader form exists so `inspectFile()` can stream EXIF through a
/// `BufReader<File>` instead of materializing the full file. The scan is capped
/// at 64 KiB so missing metadata cannot consume an image payload. Invalid
/// containers, absent tags, and values outside 1-8 all map to `None` because
/// orientation is optional metadata, not proof that pixels are decodable.
#[cfg(any(feature = "napi", feature = "fuzzing", test))]
pub(crate) fn detect_exif_orientation_bounded_from_reader<R: BufRead + Seek>(
    reader: &mut R,
) -> Option<u16> {
    let mut limited = LimitedSeekReader::new(reader, MAX_ORIENTATION_SCAN_BYTES).ok()?;
    let exif = exif::Reader::new().read_from_container(&mut limited).ok()?;
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    let orientation = field.value.get_uint(0)? as u16;
    (1..=8).contains(&orientation).then_some(orientation)
}

/// Extract EXIF Orientation tag (1-8). Returns None if missing or invalid.
///
/// Processing intentionally scans the complete in-memory source. Its firewall
/// checks already run before this call, and preserving auto-orientation for
/// valid EXIF after large APP/ICC/XMP segments is part of the existing API.
pub fn detect_exif_orientation(bytes: &[u8]) -> Option<u16> {
    let mut cursor = Cursor::new(bytes);
    let exif = exif::Reader::new().read_from_container(&mut cursor).ok()?;
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    let orientation = field.value.get_uint(0)? as u16;
    (1..=8).contains(&orientation).then_some(orientation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};

    fn exif_after_large_app_segment() -> Vec<u8> {
        let jpeg = include_bytes!("../../test/fixtures/test_with_exif.jpg");
        let mut padded = Vec::with_capacity(jpeg.len() + 65_537);
        padded.extend_from_slice(&jpeg[..2]);
        padded.extend_from_slice(&[0xff, 0xe2, 0xff, 0xff]);
        padded.extend(std::iter::repeat_n(0u8, 65_533));
        padded.extend_from_slice(&jpeg[2..]);
        padded
    }

    #[test]
    fn bounded_orientation_reader_matches_byte_api_within_budget() {
        let jpeg = include_bytes!("../../test/fixtures/test_with_exif.jpg");
        let mut reader = std::io::BufReader::new(std::io::Cursor::new(jpeg));

        assert_eq!(detect_exif_orientation(jpeg), Some(6));
        assert_eq!(
            detect_exif_orientation_bounded_from_reader(&mut reader),
            Some(6)
        );
    }

    #[test]
    fn processing_orientation_lookup_scans_beyond_preflight_budget() {
        let jpeg = exif_after_large_app_segment();
        let mut preflight_reader = std::io::BufReader::new(std::io::Cursor::new(&jpeg));

        assert_eq!(detect_exif_orientation(&jpeg), Some(6));
        assert_eq!(
            detect_exif_orientation_bounded_from_reader(&mut preflight_reader),
            None
        );
    }

    #[test]
    fn bounded_orientation_reader_rejects_out_of_range_value() {
        let mut invalid = include_bytes!("../../test/fixtures/test_with_exif.jpg").to_vec();
        let orientation_value = invalid
            .windows(4)
            .position(|window| window == [0x06, 0x00, 0x00, 0x00])
            .expect("fixture must contain the little-endian orientation value");
        invalid[orientation_value] = 9;
        let mut reader = std::io::BufReader::new(std::io::Cursor::new(invalid));

        assert_eq!(
            detect_exif_orientation_bounded_from_reader(&mut reader),
            None
        );
    }

    #[test]
    fn test_orientation_scan_reader_caps_reads_and_seeks() {
        let source = vec![0u8; (MAX_ORIENTATION_SCAN_BYTES as usize) * 2];
        let mut limited =
            LimitedSeekReader::new(std::io::Cursor::new(source), MAX_ORIENTATION_SCAN_BYTES)
                .unwrap();
        let mut visible = Vec::new();

        limited.read_to_end(&mut visible).unwrap();
        assert_eq!(visible.len(), MAX_ORIENTATION_SCAN_BYTES as usize);
        assert_eq!(
            limited
                .seek(SeekFrom::Start(MAX_ORIENTATION_SCAN_BYTES))
                .unwrap(),
            MAX_ORIENTATION_SCAN_BYTES
        );
        assert!(limited
            .seek(SeekFrom::Start(MAX_ORIENTATION_SCAN_BYTES + 1))
            .is_err());
    }

    fn encode_webp(width: u32, height: u32) -> Vec<u8> {
        let rgb: Vec<u8> = std::iter::repeat_n([10u8, 20u8, 30u8], (width * height) as usize)
            .flatten()
            .collect();
        let encoder = webp::Encoder::from_rgb(&rgb, width, height);
        encoder.encode_lossless().to_vec()
    }

    fn encode_png(width: u32, height: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(width, height, |_, _| Rgb([0, 0, 0]));
        let mut buffer = Vec::new();
        DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
            .unwrap();
        buffer
    }

    #[test]
    fn test_ensure_dimensions_safe_allows_small_image() {
        let data = encode_png(64, 64);
        assert!(ensure_dimensions_safe(&data).is_ok());
    }

    #[test]
    fn test_ensure_dimensions_safe_allows_small_webp() {
        let data = encode_webp(16, 16);
        assert!(ensure_dimensions_safe(&data).is_ok());
    }

    #[test]
    fn test_ensure_dimensions_safe_rejects_invalid_webp_header() {
        let data = b"RIFF\0\0\0\0WEBP".to_vec();
        let err = ensure_dimensions_safe(&data).unwrap_err();
        assert!(matches!(err, LazyImageError::DecodeFailed { .. }));
    }

    #[test]
    fn test_ensure_dimensions_safe_rejects_large_image() {
        let width = crate::engine::MAX_DIMENSION + 1;
        let data = encode_png(width, 1);
        let err = ensure_dimensions_safe(&data).unwrap_err();
        assert!(matches!(err, LazyImageError::DimensionExceedsLimit { .. }));
    }

    #[test]
    fn test_detect_format_jpeg_and_png() {
        let png = encode_png(2, 2);
        let jpeg = {
            let mut buf = Vec::new();
            DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 2, Rgb([1, 2, 3])))
                .write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)
                .unwrap();
            buf
        };
        assert_eq!(detect_format(&png), Some(ImageFormat::Png));
        assert_eq!(detect_format(&jpeg), Some(ImageFormat::Jpeg));
    }

    #[test]
    fn test_decode_image_routes_by_format() {
        let png = encode_png(2, 2);
        let (img_png, fmt_png) = decode_image(&png).unwrap();
        assert_eq!(fmt_png, Some(ImageFormat::Png));
        assert_eq!(img_png.dimensions(), (2, 2));
    }

    #[test]
    fn test_decode_image_routes_png_to_zune() {
        let png = encode_png(3, 1);
        let (img, fmt) = decode_image(&png).unwrap();
        assert_eq!(fmt, Some(ImageFormat::Png));
        let rgb = img.to_rgb8();
        assert_eq!(rgb.get_pixel(0, 0).0, [0, 0, 0]);
    }

    #[test]
    fn test_decode_image_routes_jpeg_to_mozjpeg() {
        // Create a tiny JPEG via image crate; detect_format should see JPEG and route to mozjpeg
        let jpeg = {
            let mut buf = Vec::new();
            DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 2, Rgb([9, 8, 7])))
                .write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)
                .unwrap();
            buf
        };
        let (img, fmt) = decode_image(&jpeg).unwrap();
        assert_eq!(fmt, Some(ImageFormat::Jpeg));
        assert_eq!(img.dimensions(), (2, 2));
    }

    #[test]
    fn test_decode_image_routes_webp_to_libwebp() {
        let webp = encode_webp(3, 2);
        let (img, fmt) = decode_image(&webp).unwrap();
        assert_eq!(fmt, Some(ImageFormat::WebP));
        assert_eq!(img.dimensions(), (3, 2));
        let rgb = img.to_rgb8();
        let pixel = rgb.get_pixel(0, 0);
        assert_eq!(pixel.0, [10, 20, 30]);
    }

    #[test]
    #[cfg(not(feature = "fuzzing"))]
    fn test_decode_png_fallback_to_image_crate_for_large_dimensions() {
        // PNG with dimensions > ZUNE_PNG_MAX_DIM (16,384) but < MAX_DIMENSION (32,768)
        // Should fallback to image crate. Skipped in fuzz build (FUZZ_MAX_DIMENSION=2048).
        let large_png = encode_png(super::ZUNE_PNG_MAX_DIM + 100, 100);
        let (img, fmt) = decode_image(&large_png).unwrap();
        assert_eq!(fmt, Some(ImageFormat::Png));
        // Should decode successfully via image crate fallback
        assert_eq!(img.dimensions(), (super::ZUNE_PNG_MAX_DIM + 100, 100));
    }

    #[test]
    #[cfg(not(feature = "fuzzing"))]
    fn test_decode_png_rejects_beyond_max_dimension() {
        // PNG with dimensions > MAX_DIMENSION should be rejected even via fallback
        let over_max = crate::engine::MAX_DIMENSION + 1;
        let big_png = encode_png(over_max, 1);
        let err = decode_image(&big_png).unwrap_err();
        assert!(matches!(err, LazyImageError::DimensionExceedsLimit { .. }));
    }

    #[test]
    fn test_zune_png_limit_lte_max_dimension() {
        // Compile-time check that ZUNE_PNG_MAX_DIM never exceeds MAX_DIMENSION.
        // A const assertion is preferred over `assert!` here because both operands
        // are constants and clippy would flag the runtime form.
        const _: () = assert!(super::ZUNE_PNG_MAX_DIM <= crate::engine::MAX_DIMENSION);
    }

    /// Crafted PNG with huge declared dimensions in the IHDR header but minimal pixel data.
    /// This reproduces the OOM crash from fuzz artifact oom-c28eb638... where a malformed
    /// file triggers unbounded allocation before dimension checks run.
    #[test]
    fn test_decode_rejects_crafted_png_with_huge_dimensions() {
        // Build a minimal PNG with a 60000×60000 IHDR but only 1 pixel of data.
        // Without the dimension guard this would try to allocate ~14 GB.
        let mut buf: Vec<u8> = Vec::new();
        // PNG signature
        buf.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        // IHDR chunk (13 bytes data)
        let ihdr_data = {
            let mut d = Vec::new();
            d.extend_from_slice(&60000u32.to_be_bytes()); // width
            d.extend_from_slice(&60000u32.to_be_bytes()); // height
            d.push(8); // bit depth
            d.push(2); // color type (RGB)
            d.push(0); // compression
            d.push(0); // filter
            d.push(0); // interlace
            d
        };
        let ihdr_len = (ihdr_data.len() as u32).to_be_bytes();
        buf.extend_from_slice(&ihdr_len);
        buf.extend_from_slice(b"IHDR");
        buf.extend_from_slice(&ihdr_data);
        // CRC (not validated by our dimension check, use dummy)
        buf.extend_from_slice(&[0; 4]);
        // IEND chunk
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(b"IEND");
        buf.extend_from_slice(&[0; 4]);

        // ensure_dimensions_safe should reject before decode
        assert!(ensure_dimensions_safe(&buf).is_err());
        // decode_image should also reject
        assert!(decode_image(&buf).is_err());
    }

    /// Verify that decode_with_image_crate enforces dimension limits via the image crate
    /// Limits API, even for formats where ensure_dimensions_safe cannot parse the header.
    #[test]
    fn test_decode_with_image_crate_enforces_limits_on_unknown_header() {
        // Garbage data that ensure_dimensions_safe returns Ok(()) for
        // (not a recognized format). decode_with_image_crate should still not OOM.
        let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];
        // Should return an error (decode failed), not OOM
        assert!(decode_with_image_crate(&garbage).is_err());
    }
}
