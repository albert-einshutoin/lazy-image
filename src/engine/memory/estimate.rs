// src/engine/memory/estimate.rs
//
// Memory footprint estimation for the image pipeline.
//
// Combines lightweight header parsing (to recover decoded BPP without a full
// decode) with operation-aware projection to compute a peak memory estimate
// per job. Used to weight `WeightedSemaphore` acquisitions.

use crate::engine::resize::{calc_cover_resize_dimensions, calc_resize_dimensions};
use crate::ops::{Operation, OutputFormat, ResizeFit, RotationAngle};
use image::ImageFormat;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::OnceLock;

/// Lower bound for any estimate to avoid zero-ish weights.
pub(super) const MIN_ESTIMATE_BYTES: u64 = 24 * 1024 * 1024; // 24MB

/// Overhead for decode/temporary buffers (heuristic).
const DECODE_OVERHEAD_BYTES: u64 = 8 * 1024 * 1024;
const FILTER_OVERHEAD_BYTES: u64 = 4 * 1024 * 1024;

/// Default bytes-per-pixel assumptions per format (decoded).
const BPP_JPEG: u64 = 3; // YCbCr → RGB
const BPP_PNG: u64 = 4; // favor safety (alpha)
const BPP_WEBP: u64 = 4;
const BPP_AVIF: u64 = 4;
const BPP_UNKNOWN: u64 = 4;

/// Maximum fallback estimate to avoid absurd weights.
const MAX_FALLBACK_ESTIMATE: u64 = 512 * 1024 * 1024; // 512 MB

const ESTIMATE_CACHE_MAX_ENTRIES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HeaderEstimate {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) format: Option<ImageFormat>,
    /// Precise bytes-per-pixel parsed from the image header.
    /// `None` means the color type could not be determined; fall back to format heuristic.
    pub(super) decoded_bpp: Option<u64>,
}

fn bytes_for_image(width: u32, height: u32, bytes_per_pixel: u64) -> u64 {
    let pixels = width as u64 * height as u64;
    pixels.saturating_mul(bytes_per_pixel)
}

fn default_bpp(format: Option<ImageFormat>) -> u64 {
    match format {
        Some(ImageFormat::Jpeg) => BPP_JPEG,
        Some(ImageFormat::Png) => BPP_PNG,
        Some(ImageFormat::WebP) => BPP_WEBP,
        Some(ImageFormat::Avif) => BPP_AVIF,
        _ => BPP_UNKNOWN,
    }
}

/// Resolve the effective BPP: prefer header-parsed value, fall back to format heuristic.
#[allow(dead_code)] // Used via tasks.rs when napi feature is enabled
fn effective_bpp(header: &HeaderEstimate) -> u64 {
    header
        .decoded_bpp
        .unwrap_or_else(|| default_bpp(header.format))
}

// ---------------------------------------------------------------------------
// Format-specific header parsers for precise decoded BPP
// ---------------------------------------------------------------------------

/// Parse JPEG SOF marker to determine number of color components.
/// SOF0 (0xFFC0) and SOF2 (0xFFC2) contain: precision(1) + height(2) + width(2) + num_components(1)
fn parse_jpeg_bpp(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 4 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let mut marker_idx = i + 1;
        while marker_idx < bytes.len() && bytes[marker_idx] == 0xFF {
            marker_idx += 1;
        }
        if marker_idx >= bytes.len() {
            break;
        }
        let marker = bytes[marker_idx];
        if marker == 0x00 {
            i = marker_idx + 1;
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            i = marker_idx + 1;
            continue;
        }
        // SOF0 (baseline), SOF1 (extended), SOF2 (progressive), SOF3 (lossless)
        if matches!(marker, 0xC0..=0xC3) {
            // SOF payload: length(2) + precision(1) + height(2) + width(2) + num_components(1)
            if marker_idx + 8 < bytes.len() {
                let num_components = bytes[marker_idx + 8] as u64;
                // JPEG components: 1=grayscale→1bpp, 3=YCbCr→3bpp (decoded to RGB), 4=CMYK→4bpp
                return Some(num_components.clamp(1, 4));
            }
            return None;
        }
        // Skip non-SOF markers
        if marker_idx + 2 < bytes.len() {
            let seg_len =
                u16::from_be_bytes([bytes[marker_idx + 1], bytes[marker_idx + 2]]) as usize;
            if seg_len < 2 {
                return None;
            }
            i = marker_idx + 1 + seg_len;
        } else {
            break;
        }
    }
    None
}

/// Parse PNG IHDR chunk for color type and bit depth.
/// IHDR: width(4) + height(4) + bit_depth(1) + color_type(1) + ...
fn parse_png_bpp(bytes: &[u8]) -> Option<u64> {
    // PNG signature (8 bytes) + IHDR chunk: length(4) + "IHDR"(4) + data(13+)
    if bytes.len() < 29 || bytes[0..8] != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return None;
    }
    // IHDR chunk type at offset 12
    if &bytes[12..16] != b"IHDR" {
        return None;
    }
    let bit_depth = bytes[24] as u64;
    let color_type = bytes[25];
    // Determine decoded BPP based on color type.
    // The image crate expands palette to RGB and converts 16-bit to 16-bit DynamicImage variants,
    // but lazy-image converts 16→8 internally. We model peak memory (decode buffer).
    let bpp = match color_type {
        0 => {
            // Grayscale
            if bit_depth <= 8 {
                1
            } else {
                2
            }
        }
        2 => {
            // RGB
            if bit_depth <= 8 {
                3
            } else {
                6
            }
        }
        3 => {
            // Palette (indexed) → expanded to RGB/RGBA during decode depending on tRNS
            if png_palette_has_transparency(bytes) {
                4
            } else {
                3
            }
        }
        4 => {
            // Grayscale + Alpha
            if bit_depth <= 8 {
                2
            } else {
                4
            }
        }
        6 => {
            // RGBA
            if bit_depth <= 8 {
                4
            } else {
                8
            }
        }
        _ => return None,
    };
    Some(bpp)
}

fn png_palette_has_transparency(bytes: &[u8]) -> bool {
    let mut offset = 8;
    while offset + 8 <= bytes.len() {
        let chunk_len = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let chunk_type = &bytes[offset + 4..offset + 8];
        let data_start = offset + 8;
        let data_end = data_start.saturating_add(chunk_len);
        let next = data_end.saturating_add(4);
        if next > bytes.len() {
            break;
        }
        if chunk_type == b"tRNS" {
            return true;
        }
        if chunk_type == b"IDAT" || chunk_type == b"IEND" {
            return false;
        }
        offset = next;
    }
    false
}

/// Parse WebP BitstreamFeatures to determine if alpha channel is present.
fn parse_webp_bpp(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    let features = webp::BitstreamFeatures::new(bytes)?;
    Some(if features.has_alpha() { 4 } else { 3 })
}

/// Determine precise BPP from image header bytes.
fn parse_color_bpp(bytes: &[u8], format: Option<ImageFormat>) -> Option<u64> {
    match format {
        Some(ImageFormat::Jpeg) => parse_jpeg_bpp(bytes),
        Some(ImageFormat::Png) => parse_png_bpp(bytes),
        Some(ImageFormat::WebP) => parse_webp_bpp(bytes),
        _ => {
            // Try magic-byte detection for format-agnostic callers
            if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
                parse_jpeg_bpp(bytes)
            } else if bytes.len() >= 8 && bytes[0..4] == [0x89, 0x50, 0x4E, 0x47] {
                parse_png_bpp(bytes)
            } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
                parse_webp_bpp(bytes)
            } else {
                None
            }
        }
    }
}

/// Estimate memory from file size when header parsing fails.
/// Uses format-specific compression ratio heuristics.
#[allow(dead_code)] // Used via tasks.rs when napi feature is enabled
pub fn estimate_fallback_from_file_size(file_size: u64, format: Option<ImageFormat>) -> u64 {
    let ratio = match format {
        Some(ImageFormat::Jpeg) => 10, // JPEG ~10:1 compression
        Some(ImageFormat::Png) => 4,   // PNG ~3-5:1 compression
        Some(ImageFormat::WebP) => 8,  // WebP ~8:1 compression
        Some(ImageFormat::Avif) => 12, // AVIF ~12:1 compression
        _ => 10,                       // conservative default
    };
    file_size
        .saturating_mul(ratio)
        .clamp(MIN_ESTIMATE_BYTES, MAX_FALLBACK_ESTIMATE)
}

fn project_operation(dims: (u32, u32), current_bpp: u64, op: &Operation) -> ((u32, u32), u64, u64) {
    match op {
        Operation::Resize { width, height, fit } => {
            let target = (
                width.unwrap_or(dims.0).max(1),
                height.unwrap_or(dims.1).max(1),
            );
            match fit {
                ResizeFit::Fill => ((target.0, target.1), 4, FILTER_OVERHEAD_BYTES),
                ResizeFit::Inside => {
                    let (w, h) = calc_resize_dimensions(dims.0, dims.1, *width, *height);
                    ((w, h), 4, FILTER_OVERHEAD_BYTES)
                }
                ResizeFit::Cover => {
                    let (resize_w, resize_h) =
                        calc_cover_resize_dimensions(dims.0, dims.1, target.0, target.1);
                    // Peak occurs after resize before crop; use larger of resize and target
                    // to model intermediate buffer.
                    let resize_bytes = bytes_for_image(resize_w, resize_h, 4);
                    let target_bytes = bytes_for_image(target.0, target.1, 4);
                    let overhead = FILTER_OVERHEAD_BYTES;
                    // Return final dims (after crop), but include resize_bytes in peak.
                    (
                        (target.0, target.1),
                        4,
                        overhead.saturating_add(resize_bytes.saturating_sub(target_bytes)),
                    )
                }
            }
        }
        Operation::Extract {
            width,
            height,
            fit,
            crop_width,
            crop_height,
            ..
        } => {
            let target_resize = (
                width.unwrap_or(dims.0).max(1),
                height.unwrap_or(dims.1).max(1),
            );
            let (resize_w, resize_h) = match fit {
                ResizeFit::Fill => target_resize,
                ResizeFit::Inside => calc_resize_dimensions(dims.0, dims.1, *width, *height),
                ResizeFit::Cover => {
                    calc_cover_resize_dimensions(dims.0, dims.1, target_resize.0, target_resize.1)
                }
            };
            let final_w = (*crop_width).max(1).min(resize_w);
            let final_h = (*crop_height).max(1).min(resize_h);
            ((final_w, final_h), 4, FILTER_OVERHEAD_BYTES)
        }
        Operation::Crop { width, height, .. } => {
            let w = (*width).max(1).min(dims.0);
            let h = (*height).max(1).min(dims.1);
            ((w, h), current_bpp, FILTER_OVERHEAD_BYTES / 2)
        }
        Operation::Rotate(angle) => {
            // 90/270 turns swap width and height; 180 keeps the dimensions.
            let rotated = matches!(angle, RotationAngle::Cw90 | RotationAngle::Cw270);
            let next_dims = if rotated { (dims.1, dims.0) } else { dims };
            (next_dims, current_bpp, FILTER_OVERHEAD_BYTES)
        }
        Operation::FlipH | Operation::FlipV => (dims, current_bpp, FILTER_OVERHEAD_BYTES / 2),
        Operation::Brightness { .. } | Operation::Contrast { .. } => {
            (dims, current_bpp.max(3), FILTER_OVERHEAD_BYTES / 2)
        }
        Operation::AutoOrient { orientation } => {
            let rotated = matches!(orientation, 5..=8);
            let next_dims = if rotated { (dims.1, dims.0) } else { dims };
            (next_dims, current_bpp, FILTER_OVERHEAD_BYTES)
        }
        Operation::Grayscale => (dims, current_bpp.max(3), FILTER_OVERHEAD_BYTES / 2),
        Operation::ColorSpace { .. } => (dims, 3, FILTER_OVERHEAD_BYTES / 2),
    }
}

#[cfg(test)]
fn estimate_memory_from_dimensions_with_context(
    width: u32,
    height: u32,
    format: Option<ImageFormat>,
    ops: &[Operation],
    output_format: Option<&OutputFormat>,
) -> u64 {
    estimate_memory_from_dimensions_with_bpp(width, height, None, format, ops, output_format)
}

fn estimate_memory_from_dimensions_with_bpp(
    width: u32,
    height: u32,
    decoded_bpp: Option<u64>,
    format: Option<ImageFormat>,
    ops: &[Operation],
    output_format: Option<&OutputFormat>,
) -> u64 {
    // Deterministic model: Calculate peak memory directly from input pixel count × BPP
    // and pipeline intermediate buffer count. No learning or observation-based corrections.
    let mut current_dims = (width, height);
    let mut current_bpp = decoded_bpp.unwrap_or_else(|| default_bpp(format));

    let mut peak = bytes_for_image(current_dims.0, current_dims.1, current_bpp)
        .saturating_add(DECODE_OVERHEAD_BYTES);
    let mut current_bytes = bytes_for_image(current_dims.0, current_dims.1, current_bpp);

    for op in ops {
        let (next_dims, next_bpp, op_overhead) = project_operation(current_dims, current_bpp, op);
        let next_bytes = bytes_for_image(next_dims.0, next_dims.1, next_bpp);
        let op_peak = current_bytes
            .saturating_add(next_bytes)
            .saturating_add(op_overhead);
        peak = peak.max(op_peak);
        current_dims = next_dims;
        current_bpp = next_bpp;
        current_bytes = next_bytes;
    }

    let output_bpp = match output_format {
        Some(OutputFormat::Jpeg { .. }) => BPP_JPEG,
        Some(OutputFormat::Png)
        | Some(OutputFormat::WebP { .. })
        | Some(OutputFormat::Avif { .. }) => 4,
        None => current_bpp,
    };
    let output_bytes = bytes_for_image(current_dims.0, current_dims.1, output_bpp);
    peak = peak.max(current_bytes.saturating_add(output_bytes / 4));

    peak.max(MIN_ESTIMATE_BYTES)
}

/// Simple wrapper for callers without format/ops context (kept for compatibility in tests)
#[cfg(test)]
fn estimate_memory_from_dimensions(width: u32, height: u32) -> u64 {
    estimate_memory_from_dimensions_with_context(width, height, None, &[], None)
}

/// Lightweight header parse; returns None if dimensions can't be read.
/// Cache for memory estimates keyed by (width, height, format, effective_bpp)
/// No LRU needed - image dimensions are discrete (~20-50 common sizes)
type EstimateCache = Mutex<HashMap<(u32, u32, Option<ImageFormat>, u64), u64>>;
static ESTIMATE_CACHE: OnceLock<EstimateCache> = OnceLock::new();

fn get_estimate_cache() -> &'static EstimateCache {
    ESTIMATE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn estimate_memory_from_header(
    bytes: &[u8],
    ops: &[Operation],
    output_format: Option<&OutputFormat>,
) -> Option<u64> {
    let header = parse_header(bytes)?;

    // Only cache if ops are empty (common case for simple operations)
    // Complex pipelines with operations are computed fresh
    if ops.is_empty() && output_format.is_none() {
        let cache_key = (
            header.width,
            header.height,
            header.format,
            effective_bpp(&header),
        );

        // Try cache first (parking_lot try_lock returns Option)
        if let Some(cache) = get_estimate_cache().try_lock() {
            if let Some(&cached) = cache.get(&cache_key) {
                return Some(cached);
            }
        }

        // Compute estimate with precise BPP when available
        let estimate = estimate_memory_from_dimensions_with_bpp(
            header.width,
            header.height,
            header.decoded_bpp,
            header.format,
            ops,
            output_format,
        );

        // Try to cache (non-fatal if lock fails)
        if let Some(mut cache) = get_estimate_cache().try_lock() {
            if cache.len() >= ESTIMATE_CACHE_MAX_ENTRIES {
                if let Some(old_key) = cache.keys().next().cloned() {
                    cache.remove(&old_key);
                }
            }
            cache.insert(cache_key, estimate);
        }

        Some(estimate)
    } else {
        // Don't cache complex pipelines
        Some(estimate_memory_from_dimensions_with_bpp(
            header.width,
            header.height,
            header.decoded_bpp,
            header.format,
            ops,
            output_format,
        ))
    }
}

/// Parse width/height/format and color type from input bytes without full decode.
pub(super) fn parse_header(bytes: &[u8]) -> Option<HeaderEstimate> {
    let cursor = Cursor::new(bytes);
    if let Ok(reader) = image::ImageReader::new(cursor).with_guessed_format() {
        let format = reader.format();
        if let Ok((w, h)) = reader.into_dimensions() {
            let decoded_bpp = parse_color_bpp(bytes, format);
            return Some(HeaderEstimate {
                width: w,
                height: h,
                format,
                decoded_bpp,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, ImageFormat, Rgba};

    /// Serializes tests that exercise the shared global `ESTIMATE_CACHE`.
    /// Without this, `cargo test`'s parallel scheduler races the cache state
    /// across `estimate_cache_*` cases and produces flaky failures.
    static CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn estimate_memory_from_dimensions_non_zero() {
        let est = estimate_memory_from_dimensions(10, 10);
        assert!(est >= MIN_ESTIMATE_BYTES);
    }

    #[test]
    fn format_specific_estimate_differs() {
        let ops: Vec<Operation> = Vec::new();
        let jpeg_est = estimate_memory_from_dimensions_with_context(
            4000,
            4000,
            Some(ImageFormat::Jpeg),
            &ops,
            Some(&OutputFormat::Jpeg {
                quality: crate::ops::Quality::unchecked(80),
                fast_mode: false,
            }),
        );
        let png_est = estimate_memory_from_dimensions_with_context(
            4000,
            4000,
            Some(ImageFormat::Png),
            &ops,
            Some(&OutputFormat::Png),
        );
        assert!(png_est > jpeg_est);
    }

    #[test]
    fn cover_resize_accounts_intermediate() {
        let ops = vec![Operation::Resize {
            width: Some(1000),
            height: Some(1000),
            fit: ResizeFit::Cover,
        }];
        let est = estimate_memory_from_dimensions_with_context(100, 10_000, None, &ops, None);
        let resize_bytes = bytes_for_image(1000, 10_000, 4);
        assert!(est >= resize_bytes);
    }

    #[test]
    fn estimate_cache_has_bounded_size() {
        let _guard = CACHE_TEST_LOCK.lock();
        {
            let mut cache = get_estimate_cache().lock();
            cache.clear();
        }

        for width in 1..=(ESTIMATE_CACHE_MAX_ENTRIES as u32 + 32) {
            let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
                image::ImageBuffer::from_pixel(width, 1, image::Rgba([0, 0, 0, 255]));
            let mut bytes = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
                .expect("png encode should succeed");

            let estimate = estimate_memory_from_header(&bytes, &[], None);
            assert!(estimate.is_some());
        }

        let cache_len = get_estimate_cache().lock().len();
        assert!(
            cache_len <= ESTIMATE_CACHE_MAX_ENTRIES,
            "cache size {} should be <= {}",
            cache_len,
            ESTIMATE_CACHE_MAX_ENTRIES
        );

        get_estimate_cache().lock().clear();
    }

    #[test]
    fn estimate_cache_keeps_distinct_bpp_variants_separate() {
        let _guard = CACHE_TEST_LOCK.lock();
        {
            let mut cache = get_estimate_cache().lock();
            cache.clear();
        }

        let gray: image::ImageBuffer<image::Luma<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(8, 8, image::Luma([128]));
        let mut gray_bytes = Vec::new();
        gray.write_to(&mut std::io::Cursor::new(&mut gray_bytes), ImageFormat::Png)
            .expect("grayscale png encode should succeed");

        let rgb: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(8, 8, image::Rgb([0, 0, 0]));
        let mut rgb_bytes = Vec::new();
        rgb.write_to(&mut std::io::Cursor::new(&mut rgb_bytes), ImageFormat::Png)
            .expect("rgb png encode should succeed");

        assert!(estimate_memory_from_header(&gray_bytes, &[], None).is_some());
        assert!(estimate_memory_from_header(&rgb_bytes, &[], None).is_some());

        assert_eq!(
            get_estimate_cache().lock().len(),
            2,
            "cache should keep separate entries for distinct decoded BPP variants"
        );

        get_estimate_cache().lock().clear();
    }

    #[test]
    fn estimate_memory_from_header_returns_minimum_for_small_png() {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();

        let estimate = estimate_memory_from_header(&bytes, &[], Some(&OutputFormat::Png)).unwrap();
        assert!(estimate >= MIN_ESTIMATE_BYTES);
    }

    #[test]
    fn jpeg_header_parsing_returns_rgb_bpp() {
        // Create a minimal RGB JPEG (3 components)
        let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, image::Rgb([0, 0, 0]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .unwrap();

        let bpp = parse_jpeg_bpp(&bytes);
        assert_eq!(bpp, Some(3), "standard RGB JPEG should report 3 BPP");
    }

    #[test]
    fn png_header_parsing_rgba() {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();

        let bpp = parse_png_bpp(&bytes);
        assert_eq!(bpp, Some(4), "RGBA PNG should report 4 BPP");
    }

    #[test]
    fn png_header_parsing_grayscale() {
        let img: ImageBuffer<image::Luma<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, image::Luma([128]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();

        let bpp = parse_png_bpp(&bytes);
        assert_eq!(bpp, Some(1), "grayscale PNG should report 1 BPP");
    }

    #[test]
    fn png_header_parsing_rgb() {
        let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, image::Rgb([0, 0, 0]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();

        let bpp = parse_png_bpp(&bytes);
        assert_eq!(bpp, Some(3), "RGB PNG should report 3 BPP");
    }

    #[test]
    fn webp_header_parsing_no_alpha() {
        let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(10, 10, image::Rgb([0, 0, 0]));
        let encoder = webp::Encoder::from_rgb(&img, 10, 10);
        let config = webp::WebPConfig::new().unwrap();
        let mem = encoder.encode_advanced(&config).unwrap();
        let bytes = mem.to_vec();

        let bpp = parse_webp_bpp(&bytes);
        assert_eq!(bpp, Some(3), "WebP without alpha should report 3 BPP");
    }

    #[test]
    fn header_estimate_populates_decoded_bpp() {
        let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(4, 4, image::Rgb([0, 0, 0]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();

        let header = parse_header(&bytes).unwrap();
        assert_eq!(
            header.decoded_bpp,
            Some(3),
            "RGB PNG header should have decoded_bpp=3"
        );
    }

    #[test]
    fn fallback_estimate_scales_with_file_size() {
        let small = estimate_fallback_from_file_size(1_000_000, Some(ImageFormat::Jpeg)); // 1MB JPEG
        let large = estimate_fallback_from_file_size(10_000_000, Some(ImageFormat::Jpeg)); // 10MB JPEG
        assert!(large > small, "larger file should produce larger estimate");
        // 1MB JPEG × 10 = 10MB, which is >= MIN_ESTIMATE_BYTES
        assert!(small >= MIN_ESTIMATE_BYTES);
        // 10MB JPEG × 10 = 100MB, which is < MAX_FALLBACK_ESTIMATE
        assert!(large <= MAX_FALLBACK_ESTIMATE);
    }

    #[test]
    fn fallback_estimate_clamped_to_max() {
        let huge = estimate_fallback_from_file_size(1_000_000_000, Some(ImageFormat::Jpeg)); // 1GB
        assert_eq!(
            huge, MAX_FALLBACK_ESTIMATE,
            "estimate should be clamped to max"
        );
    }

    #[test]
    fn grayscale_jpeg_gets_lower_estimate_than_rgb() {
        // Grayscale JPEG has 1 component → 1 BPP, RGB has 3 → 3 BPP
        // The estimate for grayscale should be lower for the same dimensions
        let ops: Vec<Operation> = Vec::new();
        let gray_est = estimate_memory_from_dimensions_with_bpp(
            4000,
            4000,
            Some(1),
            Some(ImageFormat::Jpeg),
            &ops,
            None,
        );
        let rgb_est = estimate_memory_from_dimensions_with_bpp(
            4000,
            4000,
            Some(3),
            Some(ImageFormat::Jpeg),
            &ops,
            None,
        );
        assert!(
            gray_est < rgb_est,
            "grayscale ({gray_est}) should use less memory than RGB ({rgb_est})"
        );
    }

    #[test]
    fn cover_resize_estimate_grows_with_target() {
        let ops = vec![Operation::Resize {
            width: Some(200),
            height: Some(200),
            fit: ResizeFit::Cover,
        }];
        let est_small = estimate_memory_from_dimensions_with_context(
            10,
            10,
            None,
            &ops,
            Some(&OutputFormat::Png),
        );
        let est_large = estimate_memory_from_dimensions_with_context(
            1000,
            1000,
            None,
            &ops,
            Some(&OutputFormat::Png),
        );
        assert!(est_large >= est_small);
    }
}
