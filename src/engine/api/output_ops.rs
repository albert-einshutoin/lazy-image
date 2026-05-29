#![cfg_attr(not(feature = "napi"), allow(dead_code))]

// src/engine/api/output_ops.rs
//
// Async output methods on `ImageEngine`: `toBuffer`/`toFile` family,
// byte-budget native search, and the unified `encode`/`encodeToFile` API.
//
// Each method packs a `TaskContext` for an `AsyncTask` worker — the heavy
// pixel work happens off the Node.js main thread. This file is large but the
// per-method logic is uniform; keeping it together avoids duplicating the
// `TaskContext` build boilerplate across multiple files.

#[cfg(feature = "napi")]
use super::engine::{napi_err, ImageEngine};
#[cfg(feature = "napi")]
use crate::engine::tasks::{
    EncodeTargetBytesTask, EncodeTask, EncodeWithMetricsTask, TaskContext, WriteFileTask,
    WriteFileWithMetricsTask,
};
#[cfg(feature = "napi")]
use crate::engine::validation;
#[cfg(feature = "napi")]
use crate::error::LazyImageError;
#[cfg(feature = "napi")]
use crate::ops::{Operation, OutputFormat, PresetConfig, ResizeFit};

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;

// =============================================================================
// INTERNAL HELPER
// =============================================================================

#[cfg(feature = "napi")]
impl ImageEngine {
    /// Build a [`TaskContext`] from the engine's current state for the given
    /// output format.
    ///
    /// All fields that are derived from `self` (source, decoded image, ops,
    /// metadata policy, ICC/EXIF data, firewall, auto-orient) are assembled
    /// here once. The only varying input across output methods is the resolved
    /// `OutputFormat`, which is passed as a parameter.
    ///
    /// `last_error` is always initialised to `None`; task implementations set
    /// it in their `compute` / `reject` path.
    fn build_task_context(&mut self, format: OutputFormat) -> TaskContext {
        let source = self.source.clone();
        let decoded = self.decoded.clone();
        let ops = self.ops.clone();
        let mut policy = self.metadata_policy;
        policy.apply_firewall(self.firewall.reject_metadata);
        let auto_orient = self.auto_orient;
        let icc_present = self.icc_profile().is_some();
        let icc_profile = if policy.effective_icc() {
            self.icc_profile().cloned()
        } else {
            None
        };
        let exif_data = if policy.effective_exif() {
            self.exif_data().cloned()
        } else {
            None
        };

        TaskContext {
            source,
            decoded,
            ops,
            format,
            icc_profile,
            icc_present,
            exif_data,
            auto_orient,
            metadata_policy: policy,
            firewall: self.firewall,
            #[cfg(feature = "napi")]
            last_error: None,
        }
    }
}

#[cfg(feature = "napi")]
#[napi]
impl ImageEngine {
    // =========================================================================
    // OUTPUT - Triggers async computation
    // =========================================================================

    /// Encode to buffer asynchronously.
    /// format: "jpeg", "jpg", "png", "webp", "avif"
    /// quality: 1-100 for lossy formats (default: JPEG=85, WebP=80, AVIF=60). Omit for PNG.
    /// fastMode: If true, uses faster encoding for JPEG (2-4x faster, slightly larger files). Default: false.
    ///
    /// **Non-destructive**: This method can be called multiple times on the same engine instance.
    /// The source data is cloned internally, allowing multiple format outputs.
    #[napi(ts_return_type = "Promise<Buffer>")]
    pub fn to_buffer(
        &mut self,
        env: Env,
        format: String,
        quality: Option<f64>,
        fast_mode: Option<bool>,
    ) -> Result<AsyncTask<EncodeTask>> {
        let fast_mode = fast_mode.unwrap_or(false);
        let quality = validation::sanitize_quality(quality).map_err(|e| napi_err(&env, e))?;
        let output_format = match OutputFormat::from_str_with_options(&format, quality, fast_mode) {
            Ok(format) => format,
            Err(_e) => {
                let lazy_err = LazyImageError::unsupported_format(format.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        Ok(AsyncTask::new(EncodeTask {
            // Use source directly - zero-copy for Memory and Mapped sources
            ctx: self.build_task_context(output_format),
        }))
    }

    /// Convenience: encode using the last applied preset by name.
    /// Equivalent to calling `preset(name)` then `toBuffer(preset.format, preset.quality)`.
    #[napi(js_name = "toBufferWithPreset", ts_return_type = "Promise<Buffer>")]
    pub fn to_buffer_with_preset(
        &mut self,
        env: Env,
        preset_name: String,
    ) -> Result<AsyncTask<EncodeTask>> {
        let preset = match PresetConfig::get(&preset_name) {
            Some(config) => config,
            None => {
                let lazy_err = LazyImageError::invalid_preset(preset_name.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        // Apply resize ops in-place, mirroring preset()
        self.ops.push(Operation::Resize {
            width: preset.width,
            height: preset.height,
            fit: ResizeFit::Inside,
        });

        self.last_preset = Some(preset.clone());

        let (format_str, quality, fast_mode) = match &preset.format {
            OutputFormat::Jpeg { quality, fast_mode } => {
                ("jpeg", Some(quality.get()), Some(*fast_mode))
            }
            OutputFormat::Png => ("png", None, None),
            OutputFormat::WebP { quality } => ("webp", Some(quality.get()), None),
            OutputFormat::Avif { quality } => ("avif", Some(quality.get()), None),
        };

        self.to_buffer(
            env,
            format_str.to_string(),
            quality.map(|q| q as f64),
            fast_mode,
        )
    }

    /// Encode to buffer asynchronously with performance metrics.
    /// Returns `{ data: Buffer, metrics: ProcessingMetrics }`.
    ///
    /// **Non-destructive**: This method can be called multiple times on the same engine instance.
    /// The source data is cloned internally, allowing multiple format outputs.
    #[napi(ts_return_type = "Promise<OutputWithMetrics>")]
    pub fn to_buffer_with_metrics(
        &mut self,
        env: Env,
        format: String,
        quality: Option<f64>,
        fast_mode: Option<bool>,
    ) -> Result<AsyncTask<EncodeWithMetricsTask>> {
        let fast_mode = fast_mode.unwrap_or(false);
        let quality = validation::sanitize_quality(quality).map_err(|e| napi_err(&env, e))?;
        let output_format = match OutputFormat::from_str_with_options(&format, quality, fast_mode) {
            Ok(format) => format,
            Err(_e) => {
                let lazy_err = LazyImageError::unsupported_format(format.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        Ok(AsyncTask::new(EncodeWithMetricsTask {
            // Use source directly - zero-copy for Memory and Mapped sources
            ctx: self.build_task_context(output_format),
        }))
    }

    /// Convenience: encode with metrics using a preset name.
    /// Equivalent to `preset(name)` then `toBufferWithMetrics(preset.format, preset.quality)`.
    #[napi(
        js_name = "toBufferWithMetricsPreset",
        ts_return_type = "Promise<OutputWithMetrics>"
    )]
    pub fn to_buffer_with_metrics_preset(
        &mut self,
        env: Env,
        preset_name: String,
    ) -> Result<AsyncTask<EncodeWithMetricsTask>> {
        let preset = match PresetConfig::get(&preset_name) {
            Some(config) => config,
            None => {
                let lazy_err = LazyImageError::invalid_preset(preset_name.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        self.ops.push(Operation::Resize {
            width: preset.width,
            height: preset.height,
            fit: ResizeFit::Inside,
        });

        self.last_preset = Some(preset.clone());

        let (format_str, quality, fast_mode) = match &preset.format {
            OutputFormat::Jpeg { quality, fast_mode } => {
                ("jpeg", Some(quality.get()), Some(*fast_mode))
            }
            OutputFormat::Png => ("png", None, None),
            OutputFormat::WebP { quality } => ("webp", Some(quality.get()), None),
            OutputFormat::Avif { quality } => ("avif", Some(quality.get()), None),
        };

        self.to_buffer_with_metrics(
            env,
            format_str.to_string(),
            quality.map(|q| q as f64),
            fast_mode,
        )
    }

    /// Encode to buffer with a byte-budget constraint.
    ///
    /// Performs a binary search over quality (entirely in Rust) to find the
    /// highest quality that keeps the output within `target_bytes`. This
    /// eliminates ~7 JS↔NAPI round-trips per call compared to the previous
    /// JS-side implementation.
    ///
    /// Returns `TargetBytesResult` with the encoded data, chosen quality,
    /// actual size, and whether the budget was met.
    ///
    /// @internal This is the low-level N-API bridge used by the JS `encode(...)`
    /// helper; prefer the public structured-options API. Not covered by semver.
    #[napi(
        js_name = "toBufferTargetBytesNative",
        ts_args_type = "format: OutputFormat, targetBytes: number, minQuality?: number | undefined | null, maxQuality?: number | undefined | null, fastMode?: boolean | undefined | null, strict?: boolean | undefined | null",
        ts_return_type = "Promise<TargetBytesResult>"
    )]
    // This is an internal N-API compatibility bridge used by the JS helper;
    // the public JS API accepts a structured options object.
    #[allow(clippy::too_many_arguments)]
    pub fn to_buffer_target_bytes_native(
        &mut self,
        env: Env,
        format: String,
        target_bytes: u32,
        min_quality: Option<u32>,
        max_quality: Option<u32>,
        fast_mode: Option<bool>,
        strict: Option<bool>,
    ) -> Result<AsyncTask<EncodeTargetBytesTask>> {
        let fast_mode = fast_mode.unwrap_or(false);

        // Validate quality range without silently clamping so direct native
        // callers get the same 1-100 contract as the rest of the API.
        let min_q_raw = min_quality.unwrap_or(30);
        let max_q_raw = max_quality.unwrap_or(100);
        if !(1..=100).contains(&min_q_raw) {
            return Err(napi_err(
                &env,
                LazyImageError::invalid_argument(
                    "minQuality",
                    min_q_raw.to_string(),
                    "must be between 1 and 100",
                ),
            ));
        }
        if !(1..=100).contains(&max_q_raw) {
            return Err(napi_err(
                &env,
                LazyImageError::invalid_argument(
                    "maxQuality",
                    max_q_raw.to_string(),
                    "must be between 1 and 100",
                ),
            ));
        }
        let min_q = min_q_raw as u8;
        let max_q = max_q_raw as u8;
        if min_q > max_q {
            return Err(napi_err(
                &env,
                LazyImageError::invalid_argument(
                    "minQuality",
                    min_q.to_string(),
                    "must be less than or equal to maxQuality",
                ),
            ));
        }
        if target_bytes == 0 {
            return Err(napi_err(
                &env,
                LazyImageError::invalid_argument("targetBytes", "0", "must be a positive number"),
            ));
        }

        // Parse format (quality is irrelevant here, will be overridden per iteration)
        let output_format =
            match OutputFormat::from_str_with_options(&format, Some(max_q), fast_mode) {
                Ok(f) => f,
                Err(_) => {
                    let lazy_err = LazyImageError::unsupported_format(format.clone());
                    return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
                }
            };

        Ok(AsyncTask::new(EncodeTargetBytesTask {
            ctx: self.build_task_context(output_format),
            target_bytes,
            min_quality: min_q,
            max_quality: max_q,
            quality_floor_policy_strict: strict.unwrap_or(false),
        }))
    }

    /// Encode and write directly to a file asynchronously.
    /// **Memory-efficient**: Combined with fromPath(), this enables
    /// full file-to-file processing without touching Node.js heap.
    ///
    /// **Non-destructive**: This method can be called multiple times on the same engine instance.
    /// The source data is cloned internally, allowing multiple format outputs.
    ///
    /// Returns the number of bytes written.
    #[napi(js_name = "toFile", ts_return_type = "Promise<number>")]
    pub fn to_file(
        &mut self,
        env: Env,
        path: String,
        format: String,
        quality: Option<f64>,
        fast_mode: Option<bool>,
    ) -> Result<AsyncTask<WriteFileTask>> {
        let fast_mode = fast_mode.unwrap_or(false);
        let quality = validation::sanitize_quality(quality).map_err(|e| napi_err(&env, e))?;
        validation::validate_output_path(&path).map_err(|e| napi_err(&env, e))?;

        let output_format = match OutputFormat::from_str_with_options(&format, quality, fast_mode) {
            Ok(format) => format,
            Err(_e) => {
                let lazy_err = LazyImageError::unsupported_format(format.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        Ok(AsyncTask::new(WriteFileTask {
            // Use source directly - zero-copy for Memory and Mapped sources
            ctx: self.build_task_context(output_format),
            output_path: path,
        }))
    }

    /// Encode and write directly to a file asynchronously, returning metrics.
    #[napi(
        js_name = "toFileWithMetrics",
        ts_return_type = "Promise<FileOutputWithMetrics>"
    )]
    pub fn to_file_with_metrics(
        &mut self,
        env: Env,
        path: String,
        format: String,
        quality: Option<f64>,
        fast_mode: Option<bool>,
    ) -> Result<AsyncTask<WriteFileWithMetricsTask>> {
        let fast_mode = fast_mode.unwrap_or(false);
        let quality = validation::sanitize_quality(quality).map_err(|e| napi_err(&env, e))?;
        validation::validate_output_path(&path).map_err(|e| napi_err(&env, e))?;

        let output_format = match OutputFormat::from_str_with_options(&format, quality, fast_mode) {
            Ok(format) => format,
            Err(_e) => {
                let lazy_err = LazyImageError::unsupported_format(format.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        Ok(AsyncTask::new(WriteFileWithMetricsTask {
            ctx: self.build_task_context(output_format),
            output_path: path,
        }))
    }

    // =========================================================================
    // UNIFIED OUTPUT API — encode() and encodeToFile()
    //
    // These two methods consolidate all the individual output methods above
    // into a single options-object API.  Existing methods remain for backward
    // compatibility; new code should prefer encode() / encodeToFile().
    // =========================================================================

    /// Unified encode-to-buffer method.
    ///
    /// Accepts an options object with `format`, `quality`, `fastMode`, `preset`,
    /// and `metrics` fields.  When `preset` is supplied the preset's settings
    /// take precedence over format/quality/fastMode.
    ///
    /// Returns `EncodeResult { data, metrics? }`.
    #[napi(ts_return_type = "Promise<EncodeResult>")]
    pub fn encode(
        &mut self,
        env: Env,
        options: crate::EncodeOptions,
    ) -> Result<AsyncTask<crate::engine::tasks::UnifiedEncodeTask>> {
        let want_metrics = options.metrics.unwrap_or(false);

        // Resolve format/quality/fastMode — preset wins when supplied.
        let (format_str, quality_f64, fast_mode) = if let Some(ref preset_name) = options.preset {
            let preset = match PresetConfig::get(preset_name) {
                Some(c) => c,
                None => {
                    let lazy_err = LazyImageError::invalid_preset(preset_name.clone());
                    return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
                }
            };

            // Apply the preset's resize ops.
            self.ops.push(Operation::Resize {
                width: preset.width,
                height: preset.height,
                fit: ResizeFit::Inside,
            });
            self.last_preset = Some(preset.clone());

            let (f, q, fm) = match &preset.format {
                OutputFormat::Jpeg { quality, fast_mode } => (
                    "jpeg".to_string(),
                    Some(quality.get() as f64),
                    Some(*fast_mode),
                ),
                OutputFormat::Png => ("png".to_string(), None, None),
                OutputFormat::WebP { quality } => {
                    ("webp".to_string(), Some(quality.get() as f64), None)
                }
                OutputFormat::Avif { quality } => {
                    ("avif".to_string(), Some(quality.get() as f64), None)
                }
            };
            (f, q, fm.unwrap_or(false))
        } else {
            let fmt = options.format.unwrap_or_else(|| "jpeg".to_string());
            (fmt, options.quality, options.fast_mode.unwrap_or(false))
        };

        let quality = validation::sanitize_quality(quality_f64).map_err(|e| napi_err(&env, e))?;
        let output_format =
            match OutputFormat::from_str_with_options(&format_str, quality, fast_mode) {
                Ok(f) => f,
                Err(_) => {
                    let lazy_err = LazyImageError::unsupported_format(format_str.clone());
                    return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
                }
            };

        Ok(AsyncTask::new(crate::engine::tasks::UnifiedEncodeTask {
            ctx: self.build_task_context(output_format),
            want_metrics,
        }))
    }

    /// Unified encode-to-file method.
    ///
    /// Same options as `encode()`, but writes the result directly to `path`.
    /// Returns `FileEncodeResult { bytesWritten, metrics? }`.
    #[napi(js_name = "encodeToFile", ts_return_type = "Promise<FileEncodeResult>")]
    pub fn encode_to_file(
        &mut self,
        env: Env,
        path: String,
        options: crate::EncodeOptions,
    ) -> Result<AsyncTask<crate::engine::tasks::UnifiedEncodeToFileTask>> {
        validation::validate_output_path(&path).map_err(|e| napi_err(&env, e))?;
        let want_metrics = options.metrics.unwrap_or(false);

        let (format_str, quality_f64, fast_mode) = if let Some(ref preset_name) = options.preset {
            let preset = match PresetConfig::get(preset_name) {
                Some(c) => c,
                None => {
                    let lazy_err = LazyImageError::invalid_preset(preset_name.clone());
                    return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
                }
            };

            self.ops.push(Operation::Resize {
                width: preset.width,
                height: preset.height,
                fit: ResizeFit::Inside,
            });
            self.last_preset = Some(preset.clone());

            let (f, q, fm) = match &preset.format {
                OutputFormat::Jpeg { quality, fast_mode } => (
                    "jpeg".to_string(),
                    Some(quality.get() as f64),
                    Some(*fast_mode),
                ),
                OutputFormat::Png => ("png".to_string(), None, None),
                OutputFormat::WebP { quality } => {
                    ("webp".to_string(), Some(quality.get() as f64), None)
                }
                OutputFormat::Avif { quality } => {
                    ("avif".to_string(), Some(quality.get() as f64), None)
                }
            };
            (f, q, fm.unwrap_or(false))
        } else {
            let fmt = options.format.unwrap_or_else(|| "jpeg".to_string());
            (fmt, options.quality, options.fast_mode.unwrap_or(false))
        };

        let quality = validation::sanitize_quality(quality_f64).map_err(|e| napi_err(&env, e))?;
        let output_format =
            match OutputFormat::from_str_with_options(&format_str, quality, fast_mode) {
                Ok(f) => f,
                Err(_) => {
                    let lazy_err = LazyImageError::unsupported_format(format_str.clone());
                    return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
                }
            };

        Ok(AsyncTask::new(
            crate::engine::tasks::UnifiedEncodeToFileTask {
                ctx: self.build_task_context(output_format),
                want_metrics,
                output_path: path,
            },
        ))
    }

    /// Convenience: encode to file using the preset's recommended format/quality.
    #[napi(js_name = "toFileWithPreset", ts_return_type = "Promise<number>")]
    pub fn to_file_with_preset(
        &mut self,
        env: Env,
        path: String,
        preset_name: String,
    ) -> Result<AsyncTask<WriteFileTask>> {
        let preset = match PresetConfig::get(&preset_name) {
            Some(config) => config,
            None => {
                let lazy_err = LazyImageError::invalid_preset(preset_name.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        self.ops.push(Operation::Resize {
            width: preset.width,
            height: preset.height,
            fit: ResizeFit::Inside,
        });

        self.last_preset = Some(preset.clone());

        let (format_str, quality, fast_mode) = match &preset.format {
            OutputFormat::Jpeg { quality, fast_mode } => {
                ("jpeg", Some(quality.get()), Some(*fast_mode))
            }
            OutputFormat::Png => ("png", None, None),
            OutputFormat::WebP { quality } => ("webp", Some(quality.get()), None),
            OutputFormat::Avif { quality } => ("avif", Some(quality.get()), None),
        };

        self.to_file(
            env,
            path,
            format_str.to_string(),
            quality.map(|q| q as f64),
            fast_mode,
        )
    }
}
