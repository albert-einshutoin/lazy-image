// src/engine/io/exif_embed.rs
//
// EXIF and ICC embedding into encoded image containers (JPEG, PNG, WebP).
//
// This module owns:
//   - `ExifEmbeddable` trait and its three `impl` blocks
//   - `embed_exif_generic` (private generic driver)
//   - `sanitize_exif_bytes` (private TIFF-level EXIF sanitizer)
//   - Public embed functions: `embed_exif_jpeg`, `embed_exif_png`, `embed_exif_webp`
//   - Public ICC embed functions: `embed_icc_jpeg`, `embed_icc_png`, `embed_icc_webp`
//
// These were extracted from `encoder.rs` to keep that file within the 800-line
// ceiling. Call sites are unchanged — `encoder.rs` re-exports the ICC functions
// via `use` and `processing.rs` imports the EXIF functions from `crate::engine::io`.

use crate::engine::common::run_with_panic_policy;
use crate::error::LazyImageError;
use img_parts::{jpeg::Jpeg, png::Png, ImageEXIF, ImageICC};

// Local result alias — mirrors the one in `encoder.rs`.
type EncoderResult<T> = std::result::Result<T, LazyImageError>;

// ---------------------------------------------------------------------------
// ExifEmbeddable trait
// ---------------------------------------------------------------------------

/// Trait for image container types that support EXIF embedding.
///
/// Abstracts over `img_parts::jpeg::Jpeg`, `img_parts::png::Png`, and
/// `img_parts::webp::WebP` so that EXIF embed logic can be written once
/// in `embed_exif_generic`.
trait ExifEmbeddable: Sized + ImageEXIF {
    /// Format label used for panic-policy tracing and error messages (e.g. "jpeg").
    const FORMAT: &'static str;

    /// Label for `run_with_panic_policy` (must be `&'static str`).
    const PANIC_LABEL: &'static str;

    /// Parse raw image bytes into the container type.
    fn parse(data: img_parts::Bytes) -> std::result::Result<Self, img_parts::Error>;

    /// Serialize the (possibly modified) container back to bytes.
    ///
    /// `capacity` is a hint for the output buffer (input length + EXIF payload +
    /// overhead) so the serializer avoids repeated reallocations on large images.
    fn write_out(self, capacity: usize) -> EncoderResult<Vec<u8>>;
}

impl ExifEmbeddable for Jpeg {
    const FORMAT: &'static str = "jpeg";
    const PANIC_LABEL: &'static str = "encode:jpeg:embed_exif";

    fn parse(data: img_parts::Bytes) -> std::result::Result<Self, img_parts::Error> {
        Jpeg::from_bytes(data)
    }

    fn write_out(self, capacity: usize) -> EncoderResult<Vec<u8>> {
        let mut output = Vec::with_capacity(capacity);
        self.encoder().write_to(&mut output).map_err(|e| {
            LazyImageError::encode_failed("jpeg", format!("failed to write jpeg with EXIF: {e}"))
        })?;
        Ok(output)
    }
}

impl ExifEmbeddable for Png {
    const FORMAT: &'static str = "png";
    const PANIC_LABEL: &'static str = "encode:png:embed_exif";

    fn parse(data: img_parts::Bytes) -> std::result::Result<Self, img_parts::Error> {
        Png::from_bytes(data)
    }

    fn write_out(self, capacity: usize) -> EncoderResult<Vec<u8>> {
        let mut output = Vec::with_capacity(capacity);
        self.encoder().write_to(&mut output).map_err(|e| {
            LazyImageError::encode_failed("png", format!("failed to write png with EXIF: {e}"))
        })?;
        Ok(output)
    }
}

impl ExifEmbeddable for img_parts::webp::WebP {
    const FORMAT: &'static str = "webp";
    const PANIC_LABEL: &'static str = "encode:webp:embed_exif";

    fn parse(data: img_parts::Bytes) -> std::result::Result<Self, img_parts::Error> {
        img_parts::webp::WebP::from_bytes(data)
    }

    fn write_out(self, capacity: usize) -> EncoderResult<Vec<u8>> {
        let mut output = Vec::with_capacity(capacity);
        self.encoder().write_to(&mut output).map_err(|e| {
            LazyImageError::encode_failed("webp", format!("failed to write webp with EXIF: {e}"))
        })?;
        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// EXIF sanitizer (private)
// ---------------------------------------------------------------------------

/// Sanitize raw EXIF TIFF bytes (Zero-Copy approach):
/// - Reset Orientation tag to 1 (if reset_orientation is true)
/// - Strip GPS tags by zeroing GPS IFD pointer (if strip_gps is true)
///
/// This operates directly on the TIFF structure without creating temp files.
#[cfg_attr(not(feature = "napi"), allow(dead_code))]
fn sanitize_exif_bytes(
    exif: &[u8],
    reset_orientation: bool,
    strip_gps: bool,
) -> EncoderResult<Vec<u8>> {
    // EXIF data is TIFF format - we can modify it directly
    // Structure: [byte order (2)] [magic 0x002A (2)] [IFD0 offset (4)] [IFD data...]
    if exif.len() < 8 {
        return Ok(exif.to_vec());
    }

    // Determine byte order
    let is_little_endian = match &exif[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Ok(exif.to_vec()), // Unknown byte order, return unchanged
    };

    // Helper functions for reading/writing with correct endianness
    let read_u16 = |data: &[u8], offset: usize| -> u16 {
        if is_little_endian {
            u16::from_le_bytes([data[offset], data[offset + 1]])
        } else {
            u16::from_be_bytes([data[offset], data[offset + 1]])
        }
    };

    let read_u32 = |data: &[u8], offset: usize| -> u32 {
        if is_little_endian {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        } else {
            u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        }
    };

    // Make a mutable copy for modification
    let mut result = exif.to_vec();

    // Get IFD0 offset
    let ifd0_offset = read_u32(&result, 4) as usize;
    if ifd0_offset >= result.len() || ifd0_offset < 8 {
        return Ok(result);
    }

    // Parse IFD0 to find Orientation and GPS IFD pointer tags
    let num_entries = read_u16(&result, ifd0_offset) as usize;
    let mut offset = ifd0_offset + 2;

    const ORIENTATION_TAG: u16 = 0x0112;
    const GPS_IFD_TAG: u16 = 0x8825;

    for _ in 0..num_entries {
        if offset + 12 > result.len() {
            break;
        }

        let tag = read_u16(&result, offset);
        let tag_type = read_u16(&result, offset + 2);
        // let count = read_u32(&result, offset + 4);
        let value_offset = offset + 8;

        // Reset Orientation tag to 1
        if reset_orientation && tag == ORIENTATION_TAG && tag_type == 3 {
            // Type 3 = SHORT (2 bytes)
            if is_little_endian {
                result[value_offset] = 1;
                result[value_offset + 1] = 0;
            } else {
                result[value_offset] = 0;
                result[value_offset + 1] = 1;
            }
        }

        // Zero out GPS IFD pointer to effectively strip GPS data
        if strip_gps && tag == GPS_IFD_TAG {
            // Zero the value/offset field (4 bytes)
            result[value_offset] = 0;
            result[value_offset + 1] = 0;
            result[value_offset + 2] = 0;
            result[value_offset + 3] = 0;
        }

        offset += 12; // Each IFD entry is 12 bytes
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Generic EXIF embed driver (private)
// ---------------------------------------------------------------------------

/// Generic EXIF embed: parse → sanitize → set_exif → write.
///
/// Wrapped by the format-specific public functions below so callers
/// never need to name the trait directly.
fn embed_exif_generic<T: ExifEmbeddable>(
    image_data: Vec<u8>,
    exif: &[u8],
    reset_orientation: bool,
    strip_gps: bool,
) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy(T::PANIC_LABEL, || {
        // Capture the input length before `image_data` is moved into `Bytes`
        // so we can pre-size the output buffer (input + EXIF + overhead).
        let output_cap = image_data.len() + exif.len() + 256;
        let mut img = T::parse(img_parts::Bytes::from(image_data)).map_err(|e| {
            LazyImageError::decode_failed(format!("failed to parse {} for EXIF: {e}", T::FORMAT))
        })?;

        let sanitized_exif = sanitize_exif_bytes(exif, reset_orientation, strip_gps)?;
        img.set_exif(Some(img_parts::Bytes::from(sanitized_exif)));

        img.write_out(output_cap)
    })
}

// ---------------------------------------------------------------------------
// Public EXIF embed functions
// ---------------------------------------------------------------------------

/// Embed EXIF metadata into JPEG using img-parts
///
/// Note: This function expects raw TIFF-format EXIF data (without the "Exif\0\0" header).
/// Orientation is automatically reset to 1 if auto_orient was applied.
/// GPS tags are stripped if strip_gps is true (default for privacy).
#[cfg_attr(not(feature = "napi"), allow(dead_code))]
pub fn embed_exif_jpeg(
    jpeg_data: Vec<u8>,
    exif: &[u8],
    reset_orientation: bool,
    strip_gps: bool,
) -> EncoderResult<Vec<u8>> {
    embed_exif_generic::<Jpeg>(jpeg_data, exif, reset_orientation, strip_gps)
}

pub fn embed_exif_png(
    png_data: Vec<u8>,
    exif: &[u8],
    reset_orientation: bool,
    strip_gps: bool,
) -> EncoderResult<Vec<u8>> {
    embed_exif_generic::<Png>(png_data, exif, reset_orientation, strip_gps)
}

pub fn embed_exif_webp(
    webp_data: Vec<u8>,
    exif: &[u8],
    reset_orientation: bool,
    strip_gps: bool,
) -> EncoderResult<Vec<u8>> {
    embed_exif_generic::<img_parts::webp::WebP>(webp_data, exif, reset_orientation, strip_gps)
}

// ---------------------------------------------------------------------------
// Public ICC embed functions
// ---------------------------------------------------------------------------

/// Embed ICC profile into JPEG using img-parts
pub fn embed_icc_jpeg(jpeg_data: Vec<u8>, icc: &[u8]) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:jpeg:embed_icc", || {
        use img_parts::jpeg::{markers::APP2, JpegSegment};
        use img_parts::Bytes;

        // Capacity hint: output ≈ input + ICC payload + segment/marker overhead.
        let output_cap = jpeg_data.len() + icc.len() + 64;
        let mut jpeg = Jpeg::from_bytes(Bytes::from(jpeg_data)).map_err(|e| {
            LazyImageError::decode_failed(format!("failed to parse JPEG for ICC: {e}"))
        })?;

        let mut marker_data = Vec::with_capacity(14 + icc.len());
        marker_data.extend_from_slice(b"ICC_PROFILE\0");
        marker_data.push(1);
        marker_data.push(1);
        marker_data.extend_from_slice(icc);

        let segment = JpegSegment::new_with_contents(APP2, Bytes::from(marker_data));

        let segments = jpeg.segments_mut();
        segments.insert(0, segment);

        let mut output = Vec::with_capacity(output_cap);
        jpeg.encoder().write_to(&mut output).map_err(|e| {
            LazyImageError::encode_failed("jpeg", format!("failed to write JPEG with ICC: {e}"))
        })?;

        Ok(output)
    })
}

/// Embed ICC profile into PNG using img-parts
pub fn embed_icc_png(png_data: Vec<u8>, icc: &[u8]) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:png:embed_icc", || {
        use img_parts::Bytes;

        let output_cap = png_data.len() + icc.len() + 64;
        let mut png = Png::from_bytes(Bytes::from(png_data)).map_err(|e| {
            LazyImageError::decode_failed(format!("failed to parse PNG for ICC: {e}"))
        })?;

        // `copy_from_slice` builds the `Bytes` directly from the borrowed ICC
        // slice, avoiding the intermediate `Vec` that `Bytes::from(icc.to_vec())`
        // would allocate (ICC profiles can be hundreds of KB for wide gamut).
        png.set_icc_profile(Some(Bytes::copy_from_slice(icc)));

        let mut output = Vec::with_capacity(output_cap);
        png.encoder().write_to(&mut output).map_err(|e| {
            LazyImageError::encode_failed("png", format!("failed to write PNG with ICC: {e}"))
        })?;

        Ok(output)
    })
}

/// Embed ICC profile into WebP using img-parts
pub fn embed_icc_webp(webp_data: Vec<u8>, icc: &[u8]) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:webp:embed_icc", || {
        use img_parts::webp::WebP;
        use img_parts::Bytes;

        let output_cap = webp_data.len() + icc.len() + 64;
        let mut webp = WebP::from_bytes(Bytes::from(webp_data)).map_err(|e| {
            LazyImageError::decode_failed(format!("failed to parse WebP for ICC: {e}"))
        })?;

        // Avoid the intermediate `Vec` from `Bytes::from(icc.to_vec())`.
        webp.set_icc_profile(Some(Bytes::copy_from_slice(icc)));

        let mut output = Vec::with_capacity(output_cap);
        webp.encoder().write_to(&mut output).map_err(|e| {
            LazyImageError::encode_failed("webp", format!("failed to write WebP with ICC: {e}"))
        })?;

        Ok(output)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal little-endian TIFF/EXIF payload with two IFD0 entries:
    /// Orientation (0x0112, SHORT) = `orientation`, and the GPS IFD pointer
    /// (0x8825, LONG) = `gps_offset`. Layout (byte offsets):
    ///   0  "II"              byte order
    ///   2  0x002A            magic
    ///   4  IFD0 offset = 8
    ///   8  entry count = 2
    ///   10 entry0: Orientation  (value field at offset 18)
    ///   22 entry1: GPS pointer   (value field at offset 30)
    ///   34 next-IFD offset = 0
    fn make_tiff(orientation: u16, gps_offset: u32) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"II"); // little-endian
        b.extend_from_slice(&0x002Au16.to_le_bytes());
        b.extend_from_slice(&8u32.to_le_bytes()); // IFD0 offset
        b.extend_from_slice(&2u16.to_le_bytes()); // entry count
                                                  // entry0: Orientation, type 3 (SHORT), count 1
        b.extend_from_slice(&0x0112u16.to_le_bytes());
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&(orientation as u32).to_le_bytes());
        // entry1: GPS IFD pointer, type 4 (LONG), count 1
        b.extend_from_slice(&0x8825u16.to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&gps_offset.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes()); // next IFD
        b
    }

    const ORIENTATION_VALUE_OFFSET: usize = 18;
    const GPS_VALUE_OFFSET: usize = 30;

    #[test]
    fn strip_gps_zeroes_the_gps_ifd_pointer() {
        let exif = make_tiff(6, 0x0000_0064);
        let out = sanitize_exif_bytes(&exif, false, true).unwrap();
        assert_eq!(
            &out[GPS_VALUE_OFFSET..GPS_VALUE_OFFSET + 4],
            &[0, 0, 0, 0],
            "GPS IFD pointer must be zeroed when strip_gps is true"
        );
    }

    #[test]
    fn keep_gps_leaves_the_pointer_intact() {
        let exif = make_tiff(6, 0x0000_0064);
        let out = sanitize_exif_bytes(&exif, false, false).unwrap();
        assert_eq!(
            &out[GPS_VALUE_OFFSET..GPS_VALUE_OFFSET + 4],
            &0x0000_0064u32.to_le_bytes(),
            "GPS IFD pointer must be preserved when strip_gps is false"
        );
    }

    #[test]
    fn reset_orientation_rewrites_tag_to_one() {
        let exif = make_tiff(6, 0x0000_0064);
        let out = sanitize_exif_bytes(&exif, true, false).unwrap();
        assert_eq!(out[ORIENTATION_VALUE_OFFSET], 1);
        assert_eq!(out[ORIENTATION_VALUE_OFFSET + 1], 0);
    }

    #[test]
    fn no_flags_returns_payload_unchanged() {
        let exif = make_tiff(6, 0x0000_0064);
        let out = sanitize_exif_bytes(&exif, false, false).unwrap();
        assert_eq!(out, exif);
    }

    #[test]
    fn strip_and_reset_together() {
        let exif = make_tiff(8, 0x0000_00AB);
        let out = sanitize_exif_bytes(&exif, true, true).unwrap();
        assert_eq!(out[ORIENTATION_VALUE_OFFSET], 1);
        assert_eq!(&out[GPS_VALUE_OFFSET..GPS_VALUE_OFFSET + 4], &[0, 0, 0, 0]);
    }

    #[test]
    fn too_short_payload_is_returned_unchanged() {
        let exif = vec![0x49, 0x49, 0x2A];
        let out = sanitize_exif_bytes(&exif, true, true).unwrap();
        assert_eq!(out, exif);
    }
}
