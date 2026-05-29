#![cfg_attr(not(feature = "napi"), allow(dead_code))]

// src/engine/api/batch_ops.rs
//
// Sync inspection helpers (`dimensions`, `hasIccProfile`) and the parallel
// batch processing entry points (`processBatch`, `processBatchWithMetrics`).
// `dimensions_internal` is shared logic that the public `dimensions` NAPI
// method calls into; it is also exercised directly by Rust tests.

#[cfg(feature = "napi")]
use super::engine::{napi_err, ImageEngine};
#[cfg(feature = "napi")]
use super::types::{BatchOptions, Dimensions};
#[cfg(feature = "napi")]
use crate::engine::tasks::{BatchTask, BatchWithMetricsTask};
#[cfg(feature = "napi")]
use crate::engine::validation;
#[cfg(feature = "napi")]
use crate::error::LazyImageError;
#[cfg(feature = "napi")]
use crate::ops::OutputFormat;

#[cfg(feature = "napi")]
use image::{GenericImageView, ImageReader};
#[cfg(feature = "napi")]
use std::io::Cursor;

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;

#[cfg(feature = "napi")]
impl ImageEngine {
    /// Get image dimensions WITHOUT full decoding (internal method, no Env required).
    /// For file paths, reads only the header bytes (extremely fast).
    /// For in-memory buffers, uses header-only parsing.
    /// This method is public for testing purposes.
    pub fn dimensions_internal(&mut self) -> std::result::Result<Dimensions, LazyImageError> {
        // If already decoded, use that
        if let Some(ref img) = self.decoded {
            let (w, h) = img.dimensions();
            return Ok(Dimensions {
                width: w,
                height: h,
            });
        }

        // Try to read dimensions from header only (no full decode)
        let source = match self.source.as_ref() {
            Some(source) => source,
            None => {
                return Err(LazyImageError::source_consumed());
            }
        };

        self.firewall.enforce_source_len(source.len())?;

        // Use as_bytes() to get zero-copy access for Memory and Mapped sources
        if let Some(bytes) = source.as_bytes() {
            self.firewall.scan_metadata(bytes)?;
            // For in-memory or memory-mapped data, use cursor
            let cursor = Cursor::new(bytes);
            let reader = match ImageReader::new(cursor).with_guessed_format() {
                Ok(reader) => reader,
                Err(e) => {
                    let err_msg = format!("failed to read image header: {e}");
                    return Err(LazyImageError::decode_failed(err_msg));
                }
            };

            let (width, height) = match reader.into_dimensions() {
                Ok(dims) => dims,
                Err(e) => {
                    let err_msg = format!("failed to read dimensions: {e}");
                    return Err(LazyImageError::decode_failed(err_msg));
                }
            };

            Ok(Dimensions { width, height })
        } else {
            Err(LazyImageError::source_consumed())
        }
    }
}

#[cfg(feature = "napi")]
#[napi]
impl ImageEngine {
    // =========================================================================
    // SYNC UTILITIES
    // =========================================================================

    /// Get image dimensions WITHOUT full decoding.
    /// For file paths, reads only the header bytes (extremely fast).
    /// For in-memory buffers, uses header-only parsing.
    #[napi]
    pub fn dimensions(&mut self, env: Env) -> Result<Dimensions> {
        match self.dimensions_internal() {
            Ok(dims) => Ok(dims),
            Err(lazy_err) => {
                let napi_err = crate::error::napi_error_with_code(&env, lazy_err)?;
                Err(napi_err)
            }
        }
    }

    /// Check if an ICC color profile was extracted from the source image.
    /// Returns the profile size in bytes, or null if no profile exists.
    /// This triggers lazy extraction of the ICC profile if not yet extracted.
    #[napi(js_name = "hasIccProfile")]
    pub fn has_icc_profile(&self) -> Option<u32> {
        self.icc_profile().map(|p| p.len() as u32)
    }

    /// Process multiple images in parallel with the same operations.
    ///
    /// - inputs: Array of input file paths
    /// - output_dir: Directory to write processed images
    /// - options: Output settings
    ///   - format: Output format ("jpeg", "png", "webp", "avif")
    ///   - quality: Optional quality (1-100, uses format-specific default if omitted; omit for PNG)
    ///   - fastMode: Optional fast mode flag (only applies to JPEG, default: false)
    ///   - concurrency: Optional number of parallel workers:
    ///     - 0 or undefined: Auto-detect based on CPU cores and memory limits (smart concurrency)
    ///       Detects container memory limits (cgroup v1/v2) and adjusts to prevent OOM kills.
    ///       Ideal for serverless/containerized environments with memory constraints.
    ///     - 1-1024: Manual override - use specified number of concurrent operations
    ///
    /// Backward compatibility: the legacy positional signature
    ///   processBatch(inputs, outputDir, format, quality?, fastMode?, concurrency?)
    /// is still accepted for now but will be removed in a future major release.
    // The exported N-API signature intentionally keeps legacy positional
    // arguments alongside the options object for JS compatibility.
    #[allow(clippy::too_many_arguments)]
    #[napi(js_name = "processBatch", ts_return_type = "Promise<BatchResult[]>")]
    pub fn process_batch(
        &self,
        env: Env,
        inputs: Vec<String>,
        output_dir: String,
        options_or_format: Either<BatchOptions, String>,
        quality: Option<f64>,
        fast_mode: Option<bool>,
        concurrency: Option<f64>,
    ) -> Result<AsyncTask<BatchTask>> {
        if output_dir.trim().is_empty() {
            return Err(napi_err(
                &env,
                LazyImageError::invalid_argument(
                    "outputDir",
                    "<empty>",
                    "output directory must not be empty",
                ),
            ));
        }

        let (format, quality, fast_mode, concurrency) = match options_or_format {
            Either::A(options) => (
                options.format,
                options.quality,
                options.fast_mode,
                options.concurrency,
            ),
            Either::B(format) => (format, quality, fast_mode, concurrency),
        };

        let fast_mode = fast_mode.unwrap_or(false);
        let quality = validation::sanitize_quality(quality).map_err(|e| napi_err(&env, e))?;
        let concurrency =
            validation::sanitize_concurrency(concurrency).map_err(|e| napi_err(&env, e))?;

        let output_format = match OutputFormat::from_str_with_options(&format, quality, fast_mode) {
            Ok(format) => format,
            Err(_e) => {
                let lazy_err = LazyImageError::unsupported_format(format.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };
        let ops = self.ops.clone();
        let mut policy = self.metadata_policy;
        policy.apply_firewall(self.firewall.reject_metadata);
        Ok(AsyncTask::new(BatchTask {
            inputs,
            output_dir,
            ops,
            format: output_format,
            concurrency,
            metadata_policy: policy,
            auto_orient: self.auto_orient,
            firewall: self.firewall,
            #[cfg(feature = "napi")]
            last_error: None,
        }))
    }

    // The exported N-API signature intentionally keeps legacy positional
    // arguments alongside the options object for JS compatibility.
    #[allow(clippy::too_many_arguments)]
    #[napi(
        js_name = "processBatchWithMetrics",
        ts_return_type = "Promise<BatchOutputWithMetrics>"
    )]
    pub fn process_batch_with_metrics(
        &self,
        env: Env,
        inputs: Vec<String>,
        output_dir: String,
        options_or_format: Either<BatchOptions, String>,
        quality: Option<f64>,
        fast_mode: Option<bool>,
        concurrency: Option<f64>,
    ) -> Result<AsyncTask<BatchWithMetricsTask>> {
        if output_dir.trim().is_empty() {
            return Err(napi_err(
                &env,
                LazyImageError::invalid_argument(
                    "outputDir",
                    "<empty>",
                    "output directory must not be empty",
                ),
            ));
        }

        let (format, quality, fast_mode, concurrency) = match options_or_format {
            Either::A(options) => (
                options.format,
                options.quality,
                options.fast_mode,
                options.concurrency,
            ),
            Either::B(format) => (format, quality, fast_mode, concurrency),
        };

        let fast_mode = fast_mode.unwrap_or(false);
        let quality = validation::sanitize_quality(quality).map_err(|e| napi_err(&env, e))?;
        let concurrency =
            validation::sanitize_concurrency(concurrency).map_err(|e| napi_err(&env, e))?;

        let output_format = match OutputFormat::from_str_with_options(&format, quality, fast_mode) {
            Ok(format) => format,
            Err(_e) => {
                let lazy_err = LazyImageError::unsupported_format(format.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };
        let ops = self.ops.clone();
        let mut policy = self.metadata_policy;
        policy.apply_firewall(self.firewall.reject_metadata);

        Ok(AsyncTask::new(BatchWithMetricsTask {
            inputs,
            output_dir,
            ops,
            format: output_format,
            concurrency,
            metadata_policy: policy,
            auto_orient: self.auto_orient,
            firewall: self.firewall,
            #[cfg(feature = "napi")]
            last_error: None,
        }))
    }
}
