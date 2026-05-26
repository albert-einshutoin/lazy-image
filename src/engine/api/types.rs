// src/engine/api/types.rs
//
// NAPI object types used by the public ImageEngine surface. These are pulled
// out of `engine.rs`/`pipeline_ops.rs`/`batch_ops.rs` so each operations file
// stays focused on its impl block and remains under the 800-line ceiling.

#![cfg_attr(not(feature = "napi"), allow(dead_code))]

#[cfg(feature = "napi")]
#[derive(Default)]
#[napi(object)]
pub struct KeepMetadataOptions {
    /// Preserve ICC profile (default: true when options are provided)
    pub icc: Option<bool>,
    /// Preserve EXIF metadata (camera info, settings, etc.)
    /// Note: Orientation tag is automatically reset to 1 after auto-orient
    pub exif: Option<bool>,
    /// XMP preservation is not implemented yet (will emit runtime warning when requested)
    pub xmp: Option<bool>,
    /// Strip GPS/location tags from EXIF (default: true for privacy protection)
    /// This is a security-first default that exceeds Sharp's capabilities
    pub strip_gps: Option<bool>,
}

#[cfg(feature = "napi")]
#[derive(Default)]
#[napi(object)]
pub struct ResizeOptions {
    /// Target width in pixels
    pub width: Option<f64>,
    /// Target height in pixels
    pub height: Option<f64>,
    /// Resize fit mode: inside (default), cover, fill
    pub fit: Option<String>,
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct SanitizeOptions {
    pub policy: Option<String>,
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct FirewallLimitOptions {
    pub max_pixels: Option<f64>,
    pub max_bytes: Option<f64>,
    pub timeout_ms: Option<f64>,
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[cfg(feature = "napi")]
/// Result of applying a preset, contains recommended output settings
#[napi(object)]
pub struct PresetResult {
    /// Recommended output format
    pub format: String,
    /// Recommended quality (None for PNG)
    pub quality: Option<u8>,
    /// Target width (None if aspect ratio preserved)
    pub width: Option<u32>,
    /// Target height (None if aspect ratio preserved)
    pub height: Option<u32>,
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct BatchOptions {
    /// Output format ("jpeg", "png", "webp", "avif")
    pub format: String,
    /// Optional quality (1-100), uses format default when omitted. Omit for PNG.
    pub quality: Option<f64>,
    /// Optional fast mode flag (JPEG only, default: false)
    pub fast_mode: Option<bool>,
    /// Optional number of parallel workers:
    /// 0/undefined = auto-detect, 1-1024 = manual override
    pub concurrency: Option<f64>,
}
