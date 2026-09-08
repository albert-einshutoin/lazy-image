// src/engine/firewall.rs
//
// Image Firewall configuration and enforcement helpers.

use crate::engine::io::{classify_public_upload_icc, extract_icc_profile};
use crate::error::LazyImageError;
use std::time::Instant;

// Base safety limits enforced at decode time regardless of firewall policy.
// These match the global MAX_DIMENSION / MAX_PIXELS constants and act as an
// always-on safety net against decompression bombs even when sanitize() is not
// called.  Policy-specific limits (strict/lenient) are layered on top.
const BASE_MAX_DIMENSION: u32 = crate::engine::MAX_DIMENSION; // 32_768
const BASE_MAX_PIXELS: u64 = crate::engine::MAX_PIXELS; // 100_000_000
const BASE_METADATA_LIMIT: u64 = 10 * 1024 * 1024; // 10 MB — reject obviously malicious ICC/EXIF even without sanitize()

const STRICT_MAX_PIXELS: u64 = 40_000_000; // ~8K x 5K
const LENIENT_MAX_PIXELS: u64 = 75_000_000; // generous but below global MAX_PIXELS
const STRICT_MAX_BYTES: u64 = 32 * 1024 * 1024; // 32MB input cap
const LENIENT_MAX_BYTES: u64 = 48 * 1024 * 1024; // 48MB input cap
const STRICT_TIMEOUT_MS: u64 = 5_000; // 5s wall clock (allows JPEG/WebP, strict on slow AVIF)
const LENIENT_TIMEOUT_MS: u64 = 30_000; // 30s wall clock (allows AVIF on large images)
const LENIENT_METADATA_LIMIT: u64 = 512 * 1024; // 512KB ICC cap
const STRICT_MAX_EXIF_BYTES: u64 = 64 * 1024; // 64KB EXIF cap (strict)
const LENIENT_MAX_EXIF_BYTES: u64 = 512 * 1024; // 512KB EXIF cap (lenient)

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirewallPolicy {
    Disabled,
    Strict,
    Lenient,
    PublicUpload,
    Custom,
}

// `Copy` is derived: every field is `Copy` (bools, `FirewallPolicy`, `Option<u64>`),
// so the config crosses into tasks and rayon workers as a cheap bitwise copy
// instead of an explicit `.clone()` at each call site.
#[derive(Clone, Copy, Debug)]
pub struct FirewallConfig {
    pub enabled: bool,
    pub policy: FirewallPolicy,
    pub max_pixels: Option<u64>,
    pub max_bytes: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub reject_metadata: bool,
    metadata_max_bytes: Option<u64>,
    exif_max_bytes: Option<u64>,
}

impl Default for FirewallConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            policy: FirewallPolicy::Disabled,
            max_pixels: None,
            max_bytes: None,
            timeout_ms: None,
            reject_metadata: false,
            metadata_max_bytes: None,
            exif_max_bytes: None,
        }
    }
}

impl FirewallConfig {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn strict() -> Self {
        Self {
            enabled: true,
            policy: FirewallPolicy::Strict,
            max_pixels: Some(STRICT_MAX_PIXELS),
            max_bytes: Some(STRICT_MAX_BYTES),
            timeout_ms: Some(STRICT_TIMEOUT_MS),
            reject_metadata: true,
            metadata_max_bytes: None,
            exif_max_bytes: Some(STRICT_MAX_EXIF_BYTES),
        }
    }

    pub fn lenient() -> Self {
        Self {
            enabled: true,
            policy: FirewallPolicy::Lenient,
            max_pixels: Some(LENIENT_MAX_PIXELS),
            max_bytes: Some(LENIENT_MAX_BYTES),
            timeout_ms: Some(LENIENT_TIMEOUT_MS),
            reject_metadata: false,
            metadata_max_bytes: Some(LENIENT_METADATA_LIMIT),
            exif_max_bytes: Some(LENIENT_MAX_EXIF_BYTES),
        }
    }

    /// Strict resource limits with bounded, validated ICC preservation.
    pub fn public_upload() -> Self {
        Self {
            enabled: true,
            policy: FirewallPolicy::PublicUpload,
            max_pixels: Some(STRICT_MAX_PIXELS),
            max_bytes: Some(STRICT_MAX_BYTES),
            timeout_ms: Some(STRICT_TIMEOUT_MS),
            reject_metadata: false,
            metadata_max_bytes: Some(LENIENT_METADATA_LIMIT),
            exif_max_bytes: Some(STRICT_MAX_EXIF_BYTES),
        }
    }

    pub fn custom() -> Self {
        Self {
            enabled: true,
            policy: FirewallPolicy::Custom,
            max_pixels: None,
            max_bytes: None,
            timeout_ms: None,
            reject_metadata: false,
            metadata_max_bytes: None,
            exif_max_bytes: None,
        }
    }

    pub fn apply_policy(policy: FirewallPolicy) -> Self {
        match policy {
            FirewallPolicy::Disabled => Self::disabled(),
            FirewallPolicy::Strict => Self::strict(),
            FirewallPolicy::Lenient => Self::lenient(),
            FirewallPolicy::PublicUpload => Self::public_upload(),
            FirewallPolicy::Custom => Self::custom(),
        }
    }

    /// Always-on base safety check for image dimensions.
    /// Rejects images exceeding `BASE_MAX_DIMENSION` or `BASE_MAX_PIXELS`
    /// **regardless** of whether the firewall is enabled.  This ensures that
    /// `toBuffer()` / `toFile()` without `sanitize()` still reject
    /// decompression bombs.
    ///
    /// When the firewall IS enabled, `enforce_pixels()` applies the stricter
    /// policy-specific limits on top of these base limits.
    pub fn enforce_base_limits(&self, width: u32, height: u32) -> Result<(), LazyImageError> {
        if width > BASE_MAX_DIMENSION || height > BASE_MAX_DIMENSION {
            return Err(LazyImageError::dimension_exceeds_limit(
                width.max(height),
                BASE_MAX_DIMENSION,
            ));
        }
        let pixels = width as u64 * height as u64;
        if pixels > BASE_MAX_PIXELS {
            return Err(LazyImageError::pixel_count_exceeds_limit(
                pixels,
                BASE_MAX_PIXELS,
            ));
        }
        Ok(())
    }

    pub fn enforce_source_len(&self, len: usize) -> Result<(), LazyImageError> {
        if !self.enabled {
            return Ok(());
        }
        if let Some(limit) = self.max_bytes {
            let len_u64 = len as u64;
            if len_u64 > limit {
                return Err(LazyImageError::firewall_violation(format!(
                    "Image Firewall: input size {} bytes exceeds limit of {} bytes. \
                     Use .limits({{ maxBytes: {} }}) to allow larger files or switch to lenient policy.",
                    len_u64, limit, len_u64 + 1024 * 1024
                )));
            }
        }
        Ok(())
    }

    pub fn enforce_pixels(&self, width: u32, height: u32) -> Result<(), LazyImageError> {
        if !self.enabled {
            return Ok(());
        }
        if let Some(limit) = self.max_pixels {
            let pixels = width as u64 * height as u64;
            if pixels > limit {
                return Err(LazyImageError::firewall_violation(format!(
                    "Image Firewall: {}x{} ({} pixels) exceeds limit of {} pixels. \
                     Resize the image first with .resize() or use .limits({{ maxPixels: {} }}).",
                    width,
                    height,
                    pixels,
                    limit,
                    pixels + 1_000_000
                )));
            }
        }
        Ok(())
    }

    /// Check whether the wall-clock time since `started_at` exceeds the
    /// configured timeout and, if so, return a `FirewallViolation`.
    ///
    /// **Best-effort, inter-stage only.** This function is called *between*
    /// pipeline stages (after decode, after processing ops, after encode) —
    /// never *during* a stage.  A single long-running codec operation (e.g.
    /// complex progressive JPEG decode, large rav1e AVIF encode) can run
    /// well past the timeout because there is no preemption inside the
    /// underlying C/Rust codec libraries.
    ///
    /// Implications:
    ///   - The timeout value is a **lower bound** on when a violation can be
    ///     detected, not an upper bound on total wall-clock time.
    ///   - For hard, wall-clock guarantees callers should wrap the entire
    ///     pipeline in an external watchdog timer (e.g. `setTimeout` /
    ///     `AbortController` on the JS side, or `tokio::time::timeout`).
    pub fn enforce_timeout(
        &self,
        started_at: Instant,
        stage: &'static str,
    ) -> Result<(), LazyImageError> {
        if !self.enabled {
            return Ok(());
        }
        if let Some(limit_ms) = self.timeout_ms {
            let elapsed_ms = started_at.elapsed().as_millis() as u64;
            if elapsed_ms > limit_ms {
                return Err(LazyImageError::firewall_violation(format!(
                    "Image Firewall: processing exceeded {}ms timeout at {} stage (elapsed: {}ms). \
                     Use .limits({{ timeoutMs: {} }}) for longer operations or switch to lenient policy.",
                    limit_ms, stage, elapsed_ms, elapsed_ms + 5000
                )));
            }
        }
        Ok(())
    }

    /// Always-on base metadata size check.
    /// Rejects ICC profiles larger than `BASE_METADATA_LIMIT` (10 MB) and EXIF
    /// segments larger than `BASE_METADATA_LIMIT` even when the firewall is
    /// disabled.  This catches obviously malicious payloads without requiring
    /// the caller to opt-in via `sanitize()`.
    pub fn scan_metadata_base(&self, data: &[u8]) -> Result<(), LazyImageError> {
        let icc = if self.policy == FirewallPolicy::PublicUpload {
            classify_public_upload_icc(data).profile
        } else {
            extract_icc_profile(data)?
        };
        if let Some(icc) = icc {
            let icc_len = icc.len() as u64;
            if icc_len > BASE_METADATA_LIMIT {
                return Err(LazyImageError::firewall_violation(format!(
                    "Image safety: ICC profile ({icc_len} bytes) exceeds base limit of {BASE_METADATA_LIMIT} bytes. \
                     This may indicate a malformed or malicious file."
                )));
            }
        }
        if let Some(exif_size) = scan_exif_size(data) {
            if exif_size > BASE_METADATA_LIMIT {
                return Err(LazyImageError::firewall_violation(format!(
                    "Image safety: EXIF metadata ({exif_size} bytes) exceeds base limit of {BASE_METADATA_LIMIT} bytes. \
                     This may indicate a malformed or malicious file."
                )));
            }
        }
        Ok(())
    }

    pub fn scan_metadata(&self, data: &[u8]) -> Result<(), LazyImageError> {
        if !self.enabled {
            return Ok(());
        }

        // --- ICC profile scanning ---
        let icc = if self.policy == FirewallPolicy::PublicUpload {
            classify_public_upload_icc(data).profile
        } else {
            extract_icc_profile(data)?
        };
        if let Some(icc) = icc {
            if self.reject_metadata {
                return Err(LazyImageError::firewall_violation(
                    "Image Firewall: embedded ICC profile blocked under strict policy. \
                     Use .sanitize({ policy: 'lenient' }) to allow ICC profiles.",
                ));
            }

            if let Some(limit) = self.metadata_max_bytes {
                let icc_len = icc.len() as u64;
                if icc_len > limit {
                    return Err(LazyImageError::firewall_violation(format!(
                        "Image Firewall: ICC profile ({icc_len} bytes) exceeds limit of {limit} bytes. \
                         This may indicate a malformed or malicious file."
                    )));
                }
            }
        }

        // --- EXIF metadata scanning ---
        if let Some(exif_size) = scan_exif_size(data) {
            if let Some(limit) = self.exif_max_bytes {
                if exif_size > limit {
                    return Err(LazyImageError::firewall_violation(format!(
                        "Image Firewall: EXIF metadata ({exif_size} bytes) exceeds limit of {limit} bytes. \
                         This may indicate a malformed or malicious file."
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Scan JPEG data for EXIF APP1 segments and return their total size.
/// Returns `None` if the data is not JPEG or contains no EXIF segments.
fn scan_exif_size(data: &[u8]) -> Option<u64> {
    const APP1: u8 = 0xE1;
    const SOS: u8 = 0xDA;
    const EOI: u8 = 0xD9;
    const EXIF_ID: &[u8] = b"Exif\0\0";

    // Only JPEG data starts with FF D8
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return None;
    }

    let mut i = 2; // skip SOI
    let mut total_exif: u64 = 0;

    while i + 1 < data.len() {
        if data[i] != 0xFF {
            break;
        }
        while i < data.len() && data[i] == 0xFF {
            i += 1;
        }
        if i >= data.len() {
            break;
        }
        let marker = data[i];
        i += 1;

        if marker == SOS || marker == EOI {
            break;
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            continue;
        }

        if i + 1 >= data.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > data.len() {
            break;
        }

        if marker == APP1 && seg_len >= 8 {
            let payload_start = i + 2;
            let payload_end = i + seg_len;
            if payload_end <= data.len() && payload_end - payload_start >= EXIF_ID.len() {
                let segment = &data[payload_start..payload_end];
                if segment.starts_with(EXIF_ID) {
                    total_exif += seg_len as u64;
                }
            }
        }

        i += seg_len;
    }

    if total_exif > 0 {
        Some(total_exif)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use img_parts::{png::Png, Bytes, ImageICC};

    fn build_icc_payload(len: usize) -> Vec<u8> {
        let size = len.max(132).next_multiple_of(4);
        let mut data = vec![0u8; size];
        data[..4].copy_from_slice(&(size as u32).to_be_bytes());
        data[4..8].copy_from_slice(b"TEST");
        data[8] = 2;
        data[12..16].copy_from_slice(b"mntr");
        data[16..20].copy_from_slice(b"RGB ");
        data[20..24].copy_from_slice(b"XYZ ");
        data[36..40].copy_from_slice(b"acsp");
        data
    }

    fn png_with_icc(len: usize) -> Vec<u8> {
        let img = ImageBuffer::from_fn(2, 2, |x, y| Rgb([x as u8, y as u8, 0]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        let mut png = Png::from_bytes(Bytes::from(buf)).unwrap();
        png.set_icc_profile(Some(Bytes::from(build_icc_payload(len))));
        let mut out = Vec::new();
        png.encoder().write_to(&mut out).unwrap();
        out
    }

    fn jpeg_with_exif(exif_payload_size: usize) -> Vec<u8> {
        let img = image::DynamicImage::ImageRgb8(ImageBuffer::from_fn(2, 2, |x, y| {
            Rgb([x as u8, y as u8, 0])
        }));
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let pixels = rgb.into_raw();
        let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
        comp.set_size(w as usize, h as usize);
        comp.set_quality(80.0);
        comp.set_color_space(mozjpeg::ColorSpace::JCS_YCbCr);
        comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));
        let mut jpeg_data = Vec::new();
        {
            let mut writer = comp.start_compress(&mut jpeg_data).unwrap();
            let stride = w as usize * 3;
            for row in pixels.chunks(stride) {
                writer.write_scanlines(row).unwrap();
            }
            writer.finish().unwrap();
        }
        let exif_header = b"Exif\0\0";
        let payload_len = exif_header.len() + exif_payload_size;
        let seg_len = (2 + payload_len) as u16;
        let mut result = Vec::new();
        result.extend_from_slice(&jpeg_data[..2]);
        result.push(0xFF);
        result.push(0xE1);
        result.extend_from_slice(&seg_len.to_be_bytes());
        result.extend_from_slice(exif_header);
        result.extend(std::iter::repeat_n(0xAA, exif_payload_size));
        result.extend_from_slice(&jpeg_data[2..]);
        result
    }

    // =========================================================================
    // Always-on base limit tests
    // =========================================================================

    #[test]
    fn base_limits_enforced_even_when_disabled() {
        let cfg = FirewallConfig::disabled();
        assert!(!cfg.enabled);

        // Small image passes
        assert!(cfg.enforce_base_limits(100, 100).is_ok());

        // Dimension exceeding BASE_MAX_DIMENSION fails
        let over = crate::engine::MAX_DIMENSION + 1;
        let err = cfg.enforce_base_limits(over, 1).unwrap_err();
        assert!(matches!(
            err,
            crate::error::LazyImageError::DimensionExceedsLimit { .. }
        ));

        // Pixel count exceeding BASE_MAX_PIXELS fails
        // 10001 x 10001 = 100_020_001 > 100_000_000
        let err = cfg.enforce_base_limits(10001, 10001).unwrap_err();
        assert!(matches!(
            err,
            crate::error::LazyImageError::PixelCountExceedsLimit { .. }
        ));
    }

    #[test]
    fn base_metadata_limit_enforced_even_when_disabled() {
        let cfg = FirewallConfig::disabled();
        assert!(!cfg.enabled);

        // Small ICC passes
        let small_png = png_with_icc(256);
        assert!(cfg.scan_metadata_base(&small_png).is_ok());

        // Oversized ICC (> 10MB) fails
        let oversized_png = png_with_icc((BASE_METADATA_LIMIT + 1) as usize);
        let err = cfg.scan_metadata_base(&oversized_png).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("ICC profile") && msg.contains("exceeds base limit"),
            "Expected base metadata limit message, got: {msg}"
        );
    }

    #[test]
    fn strict_policy_enforces_pixels_and_metadata() {
        let cfg = FirewallConfig::strict();
        assert!(cfg.enforce_pixels(2000, 2000).is_ok());
        assert!(cfg.enforce_pixels(7000, 7000).is_err());
        let png = png_with_icc(256);
        assert!(cfg.scan_metadata(&png).is_err());
    }

    #[test]
    fn lenient_policy_allows_small_icc() {
        let cfg = FirewallConfig::lenient();
        let safe_png = png_with_icc(256);
        assert!(cfg.scan_metadata(&safe_png).is_ok());
        let oversized_png = png_with_icc((LENIENT_METADATA_LIMIT + 1) as usize);
        assert!(cfg.scan_metadata(&oversized_png).is_err());
    }

    #[test]
    fn public_upload_uses_strict_limits_and_bounded_icc() {
        let cfg = FirewallConfig::public_upload();
        assert_eq!(cfg.policy, FirewallPolicy::PublicUpload);
        assert_eq!(cfg.max_pixels, Some(STRICT_MAX_PIXELS));
        assert_eq!(cfg.max_bytes, Some(STRICT_MAX_BYTES));
        assert_eq!(cfg.timeout_ms, Some(STRICT_TIMEOUT_MS));
        assert!(!cfg.reject_metadata);
        assert!(cfg.scan_metadata(&png_with_icc(256)).is_ok());
        assert!(cfg
            .scan_metadata(&png_with_icc((LENIENT_METADATA_LIMIT + 1) as usize))
            .is_err());
    }

    #[test]
    fn timeout_enforced() {
        let cfg = FirewallConfig {
            enabled: true,
            policy: FirewallPolicy::Custom,
            max_pixels: None,
            max_bytes: None,
            timeout_ms: Some(1),
            reject_metadata: false,
            metadata_max_bytes: None,
            exif_max_bytes: None,
        };
        let fake_start = Instant::now() - std::time::Duration::from_millis(5);
        assert!(cfg.enforce_timeout(fake_start, "decode").is_err());
    }

    /// Documents that `enforce_timeout` is **inter-stage**: it detects elapsed
    /// time at the point of call, but cannot interrupt a codec that is still
    /// running.  A stage that started before the deadline and finishes after
    /// it will only be caught by the *next* `enforce_timeout` call.
    #[test]
    fn timeout_is_inter_stage_best_effort() {
        let cfg = FirewallConfig {
            enabled: true,
            policy: FirewallPolicy::Custom,
            max_pixels: None,
            max_bytes: None,
            timeout_ms: Some(50),
            reject_metadata: false,
            metadata_max_bytes: None,
            exif_max_bytes: None,
        };

        let started = Instant::now();

        // Immediately after start the timeout has not elapsed.
        assert!(
            cfg.enforce_timeout(started, "decode").is_ok(),
            "enforce_timeout must pass when elapsed < limit"
        );

        // Simulate a slow codec stage by sleeping past the deadline.
        std::thread::sleep(std::time::Duration::from_millis(60));

        // The violation is only detected at the *next* inter-stage check.
        let result = cfg.enforce_timeout(started, "process");
        assert!(
            result.is_err(),
            "enforce_timeout must fire at the next inter-stage checkpoint"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("process"),
            "error should name the stage where the check ran, got: {msg}"
        );
    }

    #[test]
    fn strict_policy_allows_small_exif() {
        let cfg = FirewallConfig::strict();
        let jpeg = jpeg_with_exif(100);
        assert!(cfg.scan_metadata(&jpeg).is_ok());
    }

    #[test]
    fn lenient_policy_allows_small_exif() {
        let cfg = FirewallConfig::lenient();
        let jpeg = jpeg_with_exif(100);
        assert!(cfg.scan_metadata(&jpeg).is_ok());
    }

    #[test]
    fn exif_size_limit_rejects_oversized_exif() {
        let cfg = FirewallConfig {
            enabled: true,
            policy: FirewallPolicy::Custom,
            max_pixels: None,
            max_bytes: None,
            timeout_ms: None,
            reject_metadata: false,
            metadata_max_bytes: None,
            exif_max_bytes: Some(100),
        };
        let jpeg = jpeg_with_exif(200);
        let result = cfg.scan_metadata(&jpeg);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("EXIF metadata") && err_msg.contains("exceeds limit"),
            "Expected EXIF size limit message, got: {err_msg}"
        );
    }

    #[test]
    fn non_jpeg_data_passes_exif_scan() {
        let cfg = FirewallConfig::strict();
        let img = ImageBuffer::from_fn(2, 2, |x, y| Rgb([x as u8, y as u8, 0]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        assert!(cfg.scan_metadata(&buf).is_ok());
    }

    #[test]
    fn scan_exif_size_returns_none_for_non_jpeg() {
        assert!(scan_exif_size(&[0x89, 0x50, 0x4E, 0x47, 0x00]).is_none());
        assert!(scan_exif_size(b"RIFF____WEBP").is_none());
        assert!(scan_exif_size(&[]).is_none());
    }

    #[test]
    fn scan_exif_size_returns_size_for_jpeg_with_exif() {
        let jpeg = jpeg_with_exif(200);
        let size = scan_exif_size(&jpeg);
        assert!(size.is_some());
        assert_eq!(size.unwrap(), 208);
    }

    /// Verify that header-based dimension check catches oversized images
    /// without full decode. This simulates the sanitize() lazy-path fix (#370):
    /// read dimensions from header, then enforce_pixels() against the firewall policy.
    #[test]
    fn enforce_pixels_rejects_oversized_image_from_header() {
        use image::{ImageFormat, ImageReader};
        use std::io::Cursor;

        // Create a 3000x3000 image (9M pixels, exceeds strict's 40M? No — 9M < 40M).
        // Use a custom policy with a 1M pixel limit to trigger rejection.
        let img = image::DynamicImage::ImageRgb8(ImageBuffer::from_fn(200, 200, |x, y| {
            Rgb([x as u8, y as u8, 0])
        }));
        let mut png_data = Vec::new();
        img.write_to(&mut Cursor::new(&mut png_data), ImageFormat::Png)
            .unwrap();

        let cfg = FirewallConfig {
            enabled: true,
            policy: FirewallPolicy::Custom,
            max_pixels: Some(10_000), // 10k pixels — 200×200=40k exceeds this
            max_bytes: None,
            timeout_ms: None,
            reject_metadata: false,
            metadata_max_bytes: None,
            exif_max_bytes: None,
        };

        // Simulate lazy-path sanitize: read header → enforce_pixels
        let cursor = Cursor::new(&png_data);
        let reader = ImageReader::new(cursor).with_guessed_format().unwrap();
        let (w, h) = reader.into_dimensions().unwrap();
        let result = cfg.enforce_pixels(w, h);
        assert!(
            result.is_err(),
            "enforce_pixels should reject 200x200 (40k pixels) against 10k limit"
        );
    }

    /// Verify that enforce_pixels allows images within limits when reading from header.
    #[test]
    fn enforce_pixels_allows_small_image_from_header() {
        use image::{ImageFormat, ImageReader};
        use std::io::Cursor;

        let img = image::DynamicImage::ImageRgb8(ImageBuffer::from_fn(10, 10, |x, y| {
            Rgb([x as u8, y as u8, 0])
        }));
        let mut png_data = Vec::new();
        img.write_to(&mut Cursor::new(&mut png_data), ImageFormat::Png)
            .unwrap();

        let cfg = FirewallConfig::strict(); // strict: 40M pixels

        let cursor = Cursor::new(&png_data);
        let reader = ImageReader::new(cursor).with_guessed_format().unwrap();
        let (w, h) = reader.into_dimensions().unwrap();
        assert!(cfg.enforce_pixels(w, h).is_ok());
    }

    #[test]
    fn scan_exif_size_returns_none_for_jpeg_without_exif() {
        let img = image::DynamicImage::ImageRgb8(ImageBuffer::from_fn(2, 2, |x, y| {
            Rgb([x as u8, y as u8, 0])
        }));
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let pixels = rgb.into_raw();
        let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
        comp.set_size(w as usize, h as usize);
        comp.set_quality(80.0);
        comp.set_color_space(mozjpeg::ColorSpace::JCS_YCbCr);
        comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));
        let mut jpeg_data = Vec::new();
        {
            let mut writer = comp.start_compress(&mut jpeg_data).unwrap();
            let stride = w as usize * 3;
            for row in pixels.chunks(stride) {
                writer.write_scanlines(row).unwrap();
            }
            writer.finish().unwrap();
        }
        assert!(scan_exif_size(&jpeg_data).is_none());
    }
}
