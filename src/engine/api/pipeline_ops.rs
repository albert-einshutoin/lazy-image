#![cfg_attr(not(feature = "napi"), allow(dead_code))]

// src/engine/api/pipeline_ops.rs
//
// Pipeline operations that mutate the queued op list (resize/crop/rotate/etc.)
// plus the `preset()` entry point. These all return `Reference<ImageEngine>`
// for JS-side method chaining.

#[cfg(feature = "napi")]
use super::engine::{napi_err, ImageEngine, MetadataPolicy};
#[cfg(feature = "napi")]
use super::types::{
    FirewallLimitOptions, KeepMetadataOptions, PresetResult, ResizeOptions, SanitizeOptions,
};
#[cfg(feature = "napi")]
use crate::engine::firewall::{FirewallConfig, FirewallPolicy};
#[cfg(feature = "napi")]
use crate::engine::validation;
#[cfg(feature = "napi")]
use crate::error::LazyImageError;
#[cfg(feature = "napi")]
use crate::ops::{Operation, OutputFormat, PresetConfig, ResizeFit, RotationAngle};

#[cfg(feature = "napi")]
use image::ImageReader;
#[cfg(feature = "napi")]
use std::io::Cursor;
#[cfg(feature = "napi")]
use std::str::FromStr;

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;

#[cfg(feature = "napi")]
#[napi]
impl ImageEngine {
    // =========================================================================
    // PIPELINE OPERATIONS - All return Reference for JS method chaining
    // =========================================================================

    /// Resize image. Width or height can be null to maintain aspect ratio.
    /// When both width and height are provided, `fit` controls behavior:
    /// - "inside" (default): maintain aspect ratio and fit within the box
    /// - "cover": maintain aspect ratio and crop to fill the box
    /// - "fill": ignore aspect ratio and force exact dimensions
    ///
    /// Supports both signatures:
    /// - resize({ width?, height?, fit? })
    /// - resize(width?, height?, fit?)
    #[napi]
    pub fn resize(
        &mut self,
        env: Env,
        this: Reference<ImageEngine>,
        options_or_width: Either<ResizeOptions, Option<f64>>,
        height: Option<f64>,
        fit: Option<String>,
    ) -> Result<Reference<ImageEngine>> {
        let (width, height, fit) = match options_or_width {
            Either::A(options) => (options.width, options.height, options.fit),
            Either::B(width) => (width, height, fit),
        };

        let fit_mode = if let Some(value) = fit {
            ResizeFit::from_str(&value).map_err(|e| napi_err(&env, e))?
        } else {
            ResizeFit::default()
        };

        let (width, height) =
            validation::sanitize_resize_dimensions(width, height).map_err(|e| napi_err(&env, e))?;

        self.ops.push(Operation::Resize {
            width,
            height,
            fit: fit_mode,
        });
        Ok(this)
    }

    /// Crop a region from the image.
    #[napi]
    pub fn crop(
        &mut self,
        env: Env,
        this: Reference<ImageEngine>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<Reference<ImageEngine>> {
        let (x, y, width, height) =
            validation::sanitize_crop(x, y, width, height).map_err(|e| napi_err(&env, e))?;
        self.ops.push(Operation::Crop {
            x,
            y,
            width,
            height,
        });
        Ok(this)
    }

    /// Rotate by degrees (90, 180, 270 only)
    #[napi]
    pub fn rotate(
        &mut self,
        env: Env,
        this: Reference<ImageEngine>,
        degrees: i32,
    ) -> Result<Reference<ImageEngine>> {
        match RotationAngle::from_degrees(degrees) {
            // Identity rotation: queue nothing (no-op), matching prior behavior.
            Ok(None) => Ok(this),
            Ok(Some(angle)) => {
                self.ops.push(Operation::Rotate(angle));
                Ok(this)
            }
            Err(e) => Err(napi_err(&env, e)),
        }
    }

    /// Flip horizontally
    #[napi(js_name = "flipH")]
    pub fn flip_h(&mut self, this: Reference<ImageEngine>) -> Reference<ImageEngine> {
        self.ops.push(Operation::FlipH);
        this
    }

    /// Flip vertically
    #[napi(js_name = "flipV")]
    pub fn flip_v(&mut self, this: Reference<ImageEngine>) -> Reference<ImageEngine> {
        self.ops.push(Operation::FlipV);
        this
    }

    /// Convert to grayscale
    #[napi]
    pub fn grayscale(&mut self, this: Reference<ImageEngine>) -> Reference<ImageEngine> {
        self.ops.push(Operation::Grayscale);
        this
    }

    /// Enable or disable EXIF auto-orientation (default: enabled).
    /// `true` = apply EXIF Orientation automatically (sharp-compatible)
    /// `false` = ignore EXIF Orientation
    #[napi(js_name = "autoOrient")]
    pub fn auto_orient(
        &mut self,
        this: Reference<ImageEngine>,
        enabled: bool,
    ) -> Reference<ImageEngine> {
        self.auto_orient = enabled;
        this
    }

    /// Preserve metadata in output.
    /// - ICC profile: Preserved when `icc: true` (default when options provided)
    /// - EXIF: Preserved when `exif: true`. Orientation is auto-reset to 1 after auto-orient.
    /// - GPS: Stripped by default when `stripGps: true` (privacy-first, exceeds Sharp)
    /// - XMP: Not yet supported (emits warning)
    ///
    /// By default, all metadata is stripped for security and smaller file sizes.
    #[napi(js_name = "keepMetadata")]
    pub fn keep_metadata(
        &mut self,
        _env: Env,
        this: Reference<ImageEngine>,
        options: Option<KeepMetadataOptions>,
    ) -> Result<Reference<ImageEngine>> {
        let opts = options.unwrap_or_default();
        let icc = opts.icc.unwrap_or(true);
        let exif = opts.exif.unwrap_or(false);
        let xmp = opts.xmp.unwrap_or(false);
        let strip_gps = opts.strip_gps.unwrap_or(true); // Default: strip GPS for privacy

        // XMP is not yet supported - emit warning once
        if xmp && !self.xmp_warning_emitted {
            let warning =
                "keepMetadata: XMP preservation is not implemented yet; XMP data will be stripped.";
            eprintln!("{}", warning);
            self.xmp_warning_emitted = true;
        }

        // Build the policy as a sum type: `strip_gps` is only meaningful when
        // EXIF is kept, so an incoherent "keep GPS without EXIF" state can't be
        // constructed here.
        self.metadata_policy = MetadataPolicy::from_flags(icc, exif, strip_gps);
        Ok(this)
    }

    /// Enable Image Firewall mode with built-in policies (strict or lenient).
    /// Strict mode enforces aggressive limits and rejects dangerous metadata (best for zero-trust inputs).
    /// Lenient mode keeps generous limits but still guards against decompression bombs.
    #[napi]
    pub fn sanitize(
        &mut self,
        env: Env,
        this: Reference<ImageEngine>,
        options: Option<SanitizeOptions>,
    ) -> Result<Reference<ImageEngine>> {
        let requested = options
            .and_then(|opts| opts.policy)
            .unwrap_or_else(|| "strict".to_string());
        let lowered = requested.to_lowercase();
        let policy = match lowered.as_str() {
            "strict" => FirewallPolicy::Strict,
            "lenient" => FirewallPolicy::Lenient,
            _ => {
                return Err(napi_err(
                    &env,
                    LazyImageError::invalid_firewall_policy(requested),
                ))
            }
        };

        self.firewall = FirewallConfig::apply_policy(policy);
        if self.firewall.reject_metadata {
            self.metadata_policy.apply_firewall(true);
            // No need to clear the OnceLock fields: output methods already
            // check metadata_policy and skip metadata when firewall is active.
        }

        if let Some(decoded) = &self.decoded {
            self.firewall
                .enforce_pixels(decoded.width(), decoded.height())
                .map_err(|e| napi_err(&env, e))?;
        }

        if let Some(source) = &self.source {
            self.firewall
                .enforce_source_len(source.len())
                .map_err(|e| napi_err(&env, e))?;
            if let Some(bytes) = source.as_bytes() {
                self.firewall
                    .scan_metadata(bytes)
                    .map_err(|e| napi_err(&env, e))?;

                // When the image is not yet decoded (lazy path, e.g. fromPath),
                // read the header to check dimensions against the firewall policy.
                // This ensures sanitize() rejects oversized images immediately for
                // both from(buffer) and fromPath(path) construction paths.
                if self.decoded.is_none() {
                    let cursor = Cursor::new(bytes);
                    if let Ok(reader) = ImageReader::new(cursor).with_guessed_format() {
                        if let Ok((w, h)) = reader.into_dimensions() {
                            self.firewall
                                .enforce_pixels(w, h)
                                .map_err(|e| napi_err(&env, e))?;
                        }
                    }
                }
            }
        }

        Ok(this)
    }

    /// Override Image Firewall limits (maxPixels, maxBytes, timeoutMs).
    /// Any field set to 0 disables that particular limit.
    #[napi]
    pub fn limits(
        &mut self,
        env: Env,
        this: Reference<ImageEngine>,
        options: FirewallLimitOptions,
    ) -> Result<Reference<ImageEngine>> {
        if !self.firewall.enabled || matches!(self.firewall.policy, FirewallPolicy::Disabled) {
            self.firewall = FirewallConfig::custom();
        } else {
            self.firewall.enabled = true;
            self.firewall.policy = FirewallPolicy::Custom;
        }

        match validation::sanitize_limit("maxPixels", options.max_pixels)
            .map_err(|e| napi_err(&env, e))?
        {
            validation::LimitUpdate::Unchanged => {}
            validation::LimitUpdate::Disable => self.firewall.max_pixels = None,
            validation::LimitUpdate::Set(value) => self.firewall.max_pixels = Some(value),
        }

        match validation::sanitize_limit("maxBytes", options.max_bytes)
            .map_err(|e| napi_err(&env, e))?
        {
            validation::LimitUpdate::Unchanged => {}
            validation::LimitUpdate::Disable => self.firewall.max_bytes = None,
            validation::LimitUpdate::Set(value) => self.firewall.max_bytes = Some(value),
        }

        match validation::sanitize_limit("timeoutMs", options.timeout_ms)
            .map_err(|e| napi_err(&env, e))?
        {
            validation::LimitUpdate::Unchanged => {}
            validation::LimitUpdate::Disable => self.firewall.timeout_ms = None,
            validation::LimitUpdate::Set(value) => self.firewall.timeout_ms = Some(value),
        }

        Ok(this)
    }

    /// Adjust brightness (-100 to 100)
    #[napi]
    pub fn brightness(
        &mut self,
        env: Env,
        this: Reference<ImageEngine>,
        value: f64,
    ) -> Result<Reference<ImageEngine>> {
        let value = validation::ensure_finite_integer("brightness", value)
            .and_then(|int| {
                if !(-100..=100).contains(&int) {
                    Err(LazyImageError::invalid_argument(
                        "brightness",
                        int.to_string(),
                        "expected value between -100 and 100",
                    ))
                } else {
                    Ok(int as i32)
                }
            })
            .map_err(|e| napi_err(&env, e))?;

        self.ops.push(Operation::Brightness { value });
        Ok(this)
    }

    /// Adjust contrast (-100 to 100)
    #[napi]
    pub fn contrast(
        &mut self,
        env: Env,
        this: Reference<ImageEngine>,
        value: f64,
    ) -> Result<Reference<ImageEngine>> {
        let value = validation::ensure_finite_integer("contrast", value)
            .and_then(|int| {
                if !(-100..=100).contains(&int) {
                    Err(LazyImageError::invalid_argument(
                        "contrast",
                        int.to_string(),
                        "expected value between -100 and 100",
                    ))
                } else {
                    Ok(int as i32)
                }
            })
            .map_err(|e| napi_err(&env, e))?;

        self.ops.push(Operation::Contrast { value });
        Ok(this)
    }

    /// Normalize pixel format to RGB/RGBA without performing any color space transformation.
    /// This does not apply ICC profile conversion; it only guarantees the pixel layout is RGB/RGBA.
    /// Use a dedicated color management library for true color space conversions.
    #[napi(js_name = "normalizePixelFormat")]
    pub fn normalize_pixel_format(
        &mut self,
        this: Reference<ImageEngine>,
    ) -> Result<Reference<ImageEngine>> {
        // Normalize to RGB/RGBA pixel format (no color space conversion or ICC processing)
        self.ops.push(Operation::ColorSpace {
            target: crate::ops::ColorSpace::Srgb,
        });
        Ok(this)
    }

    // =========================================================================
    // PRESETS - Common configurations for web optimization
    // =========================================================================

    /// Apply a built-in preset for common use cases.
    ///
    /// Available presets:
    /// - "thumbnail": 150x150, WebP quality 75 (gallery thumbnails)
    /// - "avatar": 200x200, WebP quality 80 (profile pictures)
    /// - "hero": 1920 width, JPEG quality 85 (hero images, banners)
    /// - "social": 1200x630, JPEG quality 80 (OGP/Twitter cards)
    ///
    /// Returns the preset configuration for use with toBuffer/toFile.
    ///
    /// @deprecated This method both mutates engine state (queues the preset's
    /// resize) *and* returns a value for a separate output call, violating
    /// command-query separation — a footgun if the returned config is ignored or
    /// reused. Prefer the self-contained `encode({ preset: 'thumbnail' })` /
    /// `toBufferWithPreset(name)` / `toFileWithPreset(name)` APIs instead.
    #[napi]
    pub fn preset(
        &mut self,
        env: Env,
        _this: Reference<ImageEngine>,
        name: String,
    ) -> Result<PresetResult> {
        let config = match PresetConfig::get(&name) {
            Some(config) => config,
            None => {
                let lazy_err = LazyImageError::invalid_preset(name.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        // Apply resize operation
        self.ops.push(Operation::Resize {
            width: config.width,
            height: config.height,
            fit: ResizeFit::Inside,
        });

        // Return preset info for the user to use with toBuffer/toFile
        let (format_str, quality) = match &config.format {
            OutputFormat::Jpeg { quality, .. } => ("jpeg", Some(quality.get())),
            OutputFormat::Png => ("png", None),
            OutputFormat::WebP { quality } => ("webp", Some(quality.get())),
            OutputFormat::Avif { quality } => ("avif", Some(quality.get())),
        };

        // Store last preset for convenience helpers
        self.last_preset = Some(config.clone());

        Ok(PresetResult {
            format: format_str.to_string(),
            quality,
            width: config.width,
            height: config.height,
        })
    }
}
