// src/engine/tasks.rs
//
// Async task implementations for NAPI.
// These tasks run in background threads and don't block Node.js main thread.

use super::firewall::FirewallConfig;
use crate::engine::decoder::{
    check_dimensions, decode_jpeg_mozjpeg, decode_with_image_crate, ensure_dimensions_safe,
};
use crate::engine::encoder::{encode_avif, encode_jpeg_with_settings, encode_png, encode_webp};
use crate::engine::io::{extract_icc_profile, Source};
use crate::engine::pipeline::apply_ops;
#[cfg(feature = "napi")]
use crate::engine::pool;
use crate::error::{ErrorCategory, LazyImageError};
use crate::ops::{Operation, OutputFormat};
use image::{DynamicImage, GenericImageView};
#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
use napi::{Env, JsBuffer, Task};
#[cfg(feature = "napi")]
use rayon::prelude::*;
use std::borrow::Cow;
use std::sync::Arc;

/// Resource usage information for telemetry
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "freebsd"))]
struct ResourceUsage {
    cpu_time: f64,   // User + system CPU time in seconds
    memory_rss: u64, // Resident set size in bytes
}

/// Get current process resource usage (CPU time and RSS memory)
/// Returns None on unsupported platforms or if getrusage fails
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "freebsd"))]
fn get_resource_usage() -> Option<ResourceUsage> {
    use libc::{getrusage, rusage, RUSAGE_SELF};
    use std::mem;

    unsafe {
        let mut usage: rusage = mem::zeroed();
        if getrusage(RUSAGE_SELF, &mut usage) == 0 {
            // CPU time = user time + system time
            let cpu_time = usage.ru_utime.tv_sec as f64
                + usage.ru_utime.tv_usec as f64 / 1_000_000.0
                + usage.ru_stime.tv_sec as f64
                + usage.ru_stime.tv_usec as f64 / 1_000_000.0;

            // RSS memory (resident set size) in bytes
            // On Linux, ru_maxrss is in KB; on macOS/FreeBSD, it's in bytes
            #[cfg(target_os = "linux")]
            let memory_rss = usage.ru_maxrss as u64 * 1024;
            #[cfg(any(target_os = "macos", target_os = "freebsd"))]
            let memory_rss = usage.ru_maxrss as u64;

            Some(ResourceUsage {
                cpu_time,
                memory_rss,
            })
        } else {
            None
        }
    }
}

/// Get current process resource usage (stub for unsupported platforms)
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "freebsd")))]
fn get_resource_usage() -> Option<ResourceUsage> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "freebsd")))]
struct ResourceUsage {
    cpu_time: f64,
    memory_rss: u64,
}

// Type alias for Result - use napi::Result when napi is enabled, otherwise use standard Result
#[cfg(feature = "napi")]
type EngineResult<T> = Result<T>;
#[cfg(not(feature = "napi"))]
type EngineResult<T> = std::result::Result<T, LazyImageError>;

// Helper function to convert LazyImageError to the appropriate error type
#[cfg(feature = "napi")]
fn to_engine_error(err: LazyImageError) -> napi::Error {
    napi::Error::from(err)
}

#[cfg(not(feature = "napi"))]
fn to_engine_error(err: LazyImageError) -> LazyImageError {
    err
}

// Re-export BatchResult for api.rs
#[cfg(feature = "napi")]
#[napi(object)]
pub struct BatchResult {
    pub source: String,
    pub success: bool,
    pub error: Option<String>,
    pub output_path: Option<String>,
    pub error_code: Option<String>,
    pub error_category: Option<ErrorCategory>,
}

pub(crate) struct EncodeTask {
    pub source: Option<Source>,
    /// Decoded image wrapped in Arc. decode() returns Cow::Borrowed pointing here,
    /// enabling true Copy-on-Write in apply_ops (no deep copy for format-only conversion).
    pub decoded: Option<Arc<DynamicImage>>,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    /// Whether to preserve ICC profile in output (default: false for security & smaller files)
    /// Note: Currently only ICC profile is supported. EXIF and XMP metadata are not preserved.
    pub keep_metadata: bool,
    pub firewall: FirewallConfig,
    /// Last error that occurred during compute (for use in reject)
    #[cfg(feature = "napi")]
    pub(crate) last_error: Option<LazyImageError>,
}

impl EncodeTask {
    /// Decode image from source bytes
    /// Uses mozjpeg (libjpeg-turbo) for JPEG, falls back to image crate for others
    ///
    /// **True Copy-on-Write**: Returns `Cow::Borrowed` if image is already decoded,
    /// `Cow::Owned` if decoding was required. The caller can avoid deep copies
    /// when no mutation is needed (e.g., format conversion only).
    ///
    /// Returns LazyImageError directly (not wrapped in napi::Error) for use in process_and_encode.
    pub(crate) fn decode_internal(
        &self,
    ) -> std::result::Result<Cow<'_, DynamicImage>, LazyImageError> {
        // Prefer already decoded image (already validated)
        // Return borrowed reference - no deep copy until mutation is needed
        if let Some(ref img_arc) = self.decoded {
            check_dimensions(img_arc.width(), img_arc.height())?;
            self.firewall
                .enforce_pixels(img_arc.width(), img_arc.height())?;
            return Ok(Cow::Borrowed(img_arc.as_ref()));
        }

        // Get bytes from source - zero-copy for Memory and Mapped sources
        let bytes = match self.source.as_ref() {
            Some(source) => {
                if let Some(bytes) = source.as_bytes() {
                    bytes
                } else {
                    // Path sources require loading first - this should not happen in normal flow
                    // as from_path() converts Path to Mapped. If this occurs, it's a programming error.
                    return Err(LazyImageError::decode_failed(
                        "Path source requires loading first. Use Mapped source (from_path) instead.".to_string(),
                    ));
                }
            }
            None => {
                return Err(LazyImageError::source_consumed());
            }
        };

        self.firewall.enforce_source_len(bytes.len())?;
        self.firewall.scan_metadata(bytes)?;

        ensure_dimensions_safe(bytes)?;

        // Check magic bytes for JPEG (0xFF 0xD8)
        let img = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
            // JPEG detected - use mozjpeg for TURBO speed
            decode_jpeg_mozjpeg(bytes)?
        } else {
            // PNG, WebP, etc - use image crate (guarded by panic policy)
            decode_with_image_crate(bytes)?
        };

        // Security check: reject decompression bombs
        let (w, h) = img.dimensions();
        check_dimensions(w, h)?;
        self.firewall.enforce_pixels(w, h)?;

        Ok(Cow::Owned(img))
    }

    /// Decode image from source bytes (public API for backward compatibility)
    /// Uses mozjpeg (libjpeg-turbo) for JPEG, falls back to image crate for others
    ///
    /// **True Copy-on-Write**: Returns `Cow::Borrowed` if image is already decoded,
    /// `Cow::Owned` if decoding was required. The caller can avoid deep copies
    /// when no mutation is needed (e.g., format conversion only).
    pub(crate) fn decode(&self) -> EngineResult<Cow<'_, DynamicImage>> {
        self.decode_internal().map_err(to_engine_error)
    }

    /// Process image: decode → apply ops → encode
    /// This is the core processing pipeline shared by toBuffer and toFile.
    /// Returns LazyImageError directly (not wrapped in napi::Error) so that
    /// Task::reject can properly create error objects with code/category.
    #[cfg_attr(not(any(feature = "napi", feature = "stress")), allow(dead_code))]
    pub(crate) fn process_and_encode(
        &mut self,
        mut metrics: Option<&mut crate::ProcessingMetrics>,
    ) -> std::result::Result<Vec<u8>, LazyImageError> {
        // Get initial resource usage for CPU time and memory tracking
        let initial_usage = get_resource_usage();
        let start_total = std::time::Instant::now();

        // Get input size from source
        // Use len() method which works for both Memory and Mapped sources
        let input_size = self.source.as_ref().map(|s| s.len() as u64).unwrap_or(0);

        // 1. Decode
        let start_decode = std::time::Instant::now();
        let img = self.decode_internal()?;
        self.firewall.enforce_timeout(start_total, "decode")?;
        if let Some(m) = metrics.as_deref_mut() {
            m.decode_time = start_decode.elapsed().as_secs_f64() * 1000.0;
        }

        // 2. Apply operations
        let start_process = std::time::Instant::now();
        let processed = apply_ops(img, &self.ops)?;
        self.firewall.enforce_timeout(start_total, "process")?;
        if let Some(m) = metrics.as_deref_mut() {
            m.process_time = start_process.elapsed().as_secs_f64() * 1000.0;
        }

        // 3. Encode - only preserve ICC profile if keep_metadata is true
        let start_encode = std::time::Instant::now();
        let icc = if self.keep_metadata {
            self.icc_profile.as_ref().map(|v| v.as_slice())
        } else {
            None // Strip metadata by default for security & smaller files
        };
        let result = match &self.format {
            OutputFormat::Jpeg { quality, fast_mode } => {
                encode_jpeg_with_settings(&processed, *quality, icc, *fast_mode)
            }
            OutputFormat::Png => encode_png(&processed, icc),
            OutputFormat::WebP { quality } => encode_webp(&processed, *quality, icc),
            OutputFormat::Avif { quality } => encode_avif(&processed, *quality, icc),
        }?;
        self.firewall.enforce_timeout(start_total, "encode")?;

        // Get final resource usage
        let final_usage = get_resource_usage();
        let total_elapsed = start_total.elapsed();

        if let Some(m) = metrics {
            m.encode_time = start_encode.elapsed().as_secs_f64() * 1000.0;

            // Calculate CPU time (difference between final and initial)
            if let (Some(initial), Some(final_usage)) = (initial_usage, final_usage) {
                m.cpu_time = (final_usage.cpu_time - initial.cpu_time).max(0.0);
                // Use the maximum RSS seen during processing
                // IMPORTANT: ru_maxrss represents the cumulative maximum RSS of the entire process,
                // not just this operation. This is a limitation of getrusage() API.
                // For accurate per-operation memory tracking, consider process-specific memory profiling.
                // get_resource_usage() already converts to bytes (handles Linux KB vs macOS bytes)
                m.memory_peak = (final_usage.memory_rss.min(u32::MAX as u64)) as u32;
            } else {
                // Fallback: estimate memory (rough) - prevent overflow
                let (w, h) = processed.dimensions();
                m.memory_peak =
                    (w as u64 * h as u64 * 4 + result.len() as u64).min(u32::MAX as u64) as u32;
            }

            // Total processing time (wall clock)
            m.processing_time = total_elapsed.as_secs_f64();

            // Input and output sizes (clamp to u32::MAX for NAPI compatibility)
            m.input_size = input_size.min(u32::MAX as u64) as u32;
            m.output_size = (result.len() as u64).min(u32::MAX as u64) as u32;

            // Compression ratio
            if m.input_size > 0 {
                m.compression_ratio = m.output_size as f64 / m.input_size as f64;
            } else {
                m.compression_ratio = 0.0;
            }
        }

        Ok(result)
    }
}

// run_stress_iteration moved to engine/stress.rs to allow stress-only builds

#[cfg(feature = "napi")]
#[napi]
impl Task for EncodeTask {
    type Output = Vec<u8>;
    type JsValue = JsBuffer;

    fn compute(&mut self) -> Result<Self::Output> {
        match self.process_and_encode(None) {
            Ok(result) => {
                self.last_error = None;
                Ok(result)
            }
            Err(lazy_err) => {
                // Store the error for use in reject
                self.last_error = Some(lazy_err.clone());
                // Convert to napi::Error for the Result type
                Err(napi::Error::from(lazy_err))
            }
        }
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        env.create_buffer_with_data(output).map(|b| b.into_raw())
    }

    fn reject(&mut self, env: Env, err: napi::Error) -> Result<Self::JsValue> {
        // Use stored error if available, otherwise try to extract from napi::Error
        let lazy_err = self.last_error.take().unwrap_or_else(|| {
            // Fallback: create a generic error from the napi::Error message
            // This should not happen if all error paths properly preserve LazyImageError
            LazyImageError::generic(err.to_string())
        });
        let napi_err = crate::error::napi_error_with_code(&env, lazy_err)?;
        Err(napi_err)
    }
}

pub(crate) struct EncodeWithMetricsTask {
    pub source: Option<Source>,
    /// Decoded image wrapped in Arc for sharing. See EncodeTask for Copy-on-Write details.
    pub decoded: Option<Arc<DynamicImage>>,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    pub keep_metadata: bool,
    pub firewall: FirewallConfig,
    /// Last error that occurred during compute (for use in reject)
    #[cfg(feature = "napi")]
    pub(crate) last_error: Option<LazyImageError>,
}

#[cfg(feature = "napi")]
#[napi]
impl Task for EncodeWithMetricsTask {
    type Output = (Vec<u8>, crate::ProcessingMetrics);
    type JsValue = crate::OutputWithMetrics;

    fn compute(&mut self) -> Result<Self::Output> {
        //Reuse EncodeTask logic
        let mut task = EncodeTask {
            source: self.source.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
            format: self.format.clone(),
            icc_profile: self.icc_profile.clone(),
            keep_metadata: self.keep_metadata,
            firewall: self.firewall.clone(),
            #[cfg(feature = "napi")]
            last_error: None,
        };

        use crate::ProcessingMetrics;
        let mut metrics = ProcessingMetrics::default();
        match task.process_and_encode(Some(&mut metrics)) {
            Ok(data) => {
                self.last_error = None;
                Ok((data, metrics))
            }
            Err(lazy_err) => {
                // Store the error for use in reject
                self.last_error = Some(lazy_err.clone());
                // Convert to napi::Error for the Result type
                Err(napi::Error::from(lazy_err))
            }
        }
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        let (data, metrics) = output;
        let js_buffer = env.create_buffer_with_data(data)?.into_raw();
        Ok(crate::OutputWithMetrics {
            data: js_buffer,
            metrics,
        })
    }

    fn reject(&mut self, env: Env, err: napi::Error) -> Result<Self::JsValue> {
        // Use stored error if available, otherwise try to extract from napi::Error
        let lazy_err = self.last_error.take().unwrap_or_else(|| {
            // Fallback: create a generic error from the napi::Error message
            // This should not happen if all error paths properly preserve LazyImageError
            LazyImageError::generic(err.to_string())
        });
        let napi_err = crate::error::napi_error_with_code(&env, lazy_err)?;
        Err(napi_err)
    }
}

pub(crate) struct WriteFileTask {
    pub source: Option<Source>,
    /// Decoded image wrapped in Arc for sharing. See EncodeTask for Copy-on-Write details.
    pub decoded: Option<Arc<DynamicImage>>,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    pub keep_metadata: bool,
    pub firewall: FirewallConfig,
    pub output_path: String,
    /// Last error that occurred during compute (for use in reject)
    #[cfg(feature = "napi")]
    pub(crate) last_error: Option<LazyImageError>,
}

#[cfg(feature = "napi")]
#[napi]
impl Task for WriteFileTask {
    type Output = u32; // Bytes written
    type JsValue = u32;

    fn compute(&mut self) -> Result<Self::Output> {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create EncodeTask and use its process_and_encode method
        let mut encode_task = EncodeTask {
            source: self.source.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
            format: self.format.clone(),
            icc_profile: self.icc_profile.clone(),
            keep_metadata: self.keep_metadata,
            firewall: self.firewall.clone(),
            #[cfg(feature = "napi")]
            last_error: None,
        };

        // Process image using shared logic
        let data = match encode_task.process_and_encode(None) {
            Ok(data) => data,
            Err(lazy_err) => {
                // Store the error for use in reject
                self.last_error = Some(lazy_err.clone());
                return Err(napi::Error::from(lazy_err));
            }
        };

        // Atomic write: write to temp file in the same directory as target,
        // then rename on success. tempfile automatically cleans up on drop.
        let output_dir = std::path::Path::new(&self.output_path)
            .parent()
            .ok_or_else(|| {
                let lazy_err =
                    LazyImageError::internal_panic("output path has no parent directory");
                self.last_error = Some(lazy_err.clone());
                napi::Error::from(lazy_err)
            })?;

        // Create temp file in the same directory as the target file
        // This ensures rename() works (cross-filesystem rename can fail)
        let mut temp_file = NamedTempFile::new_in(output_dir).map_err(|e| {
            let lazy_err =
                LazyImageError::file_write_failed(output_dir.to_string_lossy().to_string(), e);
            self.last_error = Some(lazy_err.clone());
            napi::Error::from(lazy_err)
        })?;

        let temp_path = temp_file.path().to_path_buf();
        // Check for overflow: NAPI requires u32, but we can't handle >4GB files
        let bytes_written = data.len().try_into().map_err(|_| {
            let lazy_err = LazyImageError::internal_panic("file size exceeds 4GB limit (u32::MAX)");
            self.last_error = Some(lazy_err.clone());
            napi::Error::from(lazy_err)
        })?;
        temp_file.write_all(&data).map_err(|e| {
            let lazy_err = LazyImageError::file_write_failed(temp_path.display().to_string(), e);
            self.last_error = Some(lazy_err.clone());
            napi::Error::from(lazy_err)
        })?;

        // Ensure data is flushed to disk
        temp_file.as_file_mut().sync_all().map_err(|e| {
            let lazy_err = LazyImageError::file_write_failed(temp_path.display().to_string(), e);
            self.last_error = Some(lazy_err.clone());
            napi::Error::from(lazy_err)
        })?;

        // Atomic rename: tempfile handles cleanup automatically if this fails
        temp_file.persist(&self.output_path).map_err(|e| {
            let io_error = std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to persist file: {}", e),
            );
            let lazy_err = LazyImageError::file_write_failed(self.output_path.clone(), io_error);
            self.last_error = Some(lazy_err.clone());
            napi::Error::from(lazy_err)
        })?;

        Ok(bytes_written)
    }
    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }

    fn reject(&mut self, env: Env, err: napi::Error) -> Result<Self::JsValue> {
        // Use stored error if available, otherwise try to extract from napi::Error
        let lazy_err = self.last_error.take().unwrap_or_else(|| {
            // Fallback: create a generic error from the napi::Error message
            // This should not happen if all error paths properly preserve LazyImageError
            LazyImageError::generic(err.to_string())
        });
        let napi_err = crate::error::napi_error_with_code(&env, lazy_err)?;
        Err(napi_err)
    }
}

pub(crate) struct BatchTask {
    pub inputs: Vec<String>,
    pub output_dir: String,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub concurrency: u32,
    pub keep_metadata: bool,
    pub firewall: FirewallConfig,
    /// Last error that occurred during compute (for use in reject)
    #[cfg(feature = "napi")]
    pub(crate) last_error: Option<LazyImageError>,
}

#[cfg(feature = "napi")]
#[napi]
impl Task for BatchTask {
    type Output = Vec<BatchResult>;
    type JsValue = Vec<BatchResult>;

    fn compute(&mut self) -> Result<Self::Output> {
        use std::fs;
        use std::path::Path;

        if !Path::new(&self.output_dir).exists() {
            fs::create_dir_all(&self.output_dir).map_err(|e| {
                let lazy_err = LazyImageError::file_write_failed(self.output_dir.clone(), e);
                self.last_error = Some(lazy_err.clone());
                napi::Error::from(lazy_err)
            })?;
        }

        // Helper closure to process a single image
        let ops = &self.ops;
        let format = &self.format;
        let output_dir = &self.output_dir;
        let keep_metadata = self.keep_metadata;
        let firewall = self.firewall.clone();
        let process_one = |input_path: &String| -> BatchResult {
            let result = (|| -> std::result::Result<String, LazyImageError> {
                let start_total = std::time::Instant::now();
                // Use memory mapping for zero-copy access (same as from_path)
                use memmap2::Mmap;
                use std::fs::File;
                use std::sync::Arc;

                let file = match File::open(input_path) {
                    Ok(file) => file,
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::NotFound {
                            return Err(LazyImageError::file_not_found(input_path.clone()));
                        }
                        return Err(LazyImageError::file_read_failed(input_path.clone(), e));
                    }
                };

                // Safety: We assume the file won't be modified externally during processing.
                // This is a common assumption in image processing libraries.
                // Note: On Windows, memory-mapped files cannot be deleted while mapped.
                let mmap = unsafe {
                    Mmap::map(&file)
                        .map_err(|e| LazyImageError::mmap_failed(input_path.clone(), e))?
                };
                let mmap_arc = Arc::new(mmap);
                let data = mmap_arc.as_ref();

                firewall.enforce_source_len(data.len())?;
                firewall.scan_metadata(data)?;

                let icc_profile = if keep_metadata {
                    extract_icc_profile(data).map(Arc::new)
                } else {
                    None
                };

                let img = if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
                    decode_jpeg_mozjpeg(data)?
                } else {
                    decode_with_image_crate(data)?
                };
                firewall.enforce_timeout(start_total, "decode")?;

                let (w, h) = img.dimensions();
                check_dimensions(w, h)?;
                firewall.enforce_pixels(w, h)?;

                let processed = apply_ops(Cow::Owned(img), ops)?;
                firewall.enforce_timeout(start_total, "process")?;

                // Encode - only preserve ICC profile if keep_metadata is true
                let icc = if keep_metadata {
                    icc_profile.as_ref().map(|v| v.as_slice())
                } else {
                    None // Strip metadata by default for security & smaller files
                };
                let encoded = match format {
                    OutputFormat::Jpeg { quality, fast_mode } => {
                        encode_jpeg_with_settings(&processed, *quality, icc, *fast_mode)?
                    }
                    OutputFormat::Png => encode_png(&processed, icc)?,
                    OutputFormat::WebP { quality } => encode_webp(&processed, *quality, icc)?,
                    OutputFormat::Avif { quality } => encode_avif(&processed, *quality, icc)?,
                };
                firewall.enforce_timeout(start_total, "encode")?;

                let filename = Path::new(input_path)
                    .file_name()
                    .ok_or_else(|| LazyImageError::internal_panic("invalid filename"))?;

                let extension = match format {
                    OutputFormat::Jpeg { .. } => "jpg",
                    OutputFormat::Png => "png",
                    OutputFormat::WebP { .. } => "webp",
                    OutputFormat::Avif { .. } => "avif",
                };

                let output_filename = Path::new(filename).with_extension(extension);
                let output_path = Path::new(output_dir).join(output_filename);

                // Atomic write: use tempfile for safe file writing
                use std::io::Write;
                use tempfile::NamedTempFile;

                let mut temp_file = NamedTempFile::new_in(output_dir)
                    .map_err(|e| LazyImageError::file_write_failed(output_dir.to_string(), e))?;

                let temp_path = temp_file.path().to_path_buf();
                temp_file.write_all(&encoded).map_err(|e| {
                    LazyImageError::file_write_failed(temp_path.display().to_string(), e)
                })?;

                temp_file.as_file_mut().sync_all().map_err(|e| {
                    LazyImageError::file_write_failed(temp_path.display().to_string(), e)
                })?;

                // Atomic rename
                temp_file.persist(&output_path).map_err(|e| {
                    let io_error = std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("failed to persist file: {}", e),
                    );
                    LazyImageError::file_write_failed(output_path.display().to_string(), io_error)
                })?;

                Ok(output_path.to_string_lossy().to_string())
            })();

            match result {
                Ok(path) => BatchResult {
                    source: input_path.clone(),
                    success: true,
                    error: None,
                    output_path: Some(path),
                    error_code: None,
                    error_category: None,
                },
                Err(err) => {
                    let category = err.category();
                    let error_msg = format!("{}: {}", input_path, err);
                    BatchResult {
                        source: input_path.clone(),
                        success: false,
                        error: Some(error_msg),
                        output_path: None,
                        error_code: Some(category.code().to_string()),
                        error_category: Some(category),
                    }
                }
            }
        };

        // Validate concurrency parameter
        // concurrency = 0 means "use default" (auto-detected via CPU and memory)
        // concurrency = 1..MAX_CONCURRENCY means "use specified number of concurrent operations"
        if self.concurrency > pool::MAX_CONCURRENCY as u32 {
            let lazy_err = LazyImageError::internal_panic(format!(
                "invalid concurrency value: {} (must be 0 or 1-{})",
                self.concurrency,
                pool::MAX_CONCURRENCY
            ));
            self.last_error = Some(lazy_err.clone());
            return Err(napi::Error::from(lazy_err));
        }

        // Determine effective concurrency
        let effective_concurrency = if self.concurrency == 0 {
            // Auto-detect: use smart concurrency based on CPU and memory
            pool::calculate_optimal_concurrency()
        } else {
            // Manual override: use user-specified concurrency
            self.concurrency as usize
        };

        // Use global thread pool with chunks-based concurrency limiting
        // This avoids creating a new pool per request (which is expensive)
        // and instead uses chunks to limit concurrent operations
        let results: Vec<BatchResult> = pool::get_pool().install(|| {
            if effective_concurrency >= self.inputs.len() {
                // If concurrency >= input count, process all in parallel
                self.inputs.par_iter().map(process_one).collect()
            } else {
                // Use chunks to limit concurrent operations
                // Process chunks sequentially, but each chunk in parallel
                // This ensures at most `effective_concurrency` operations run simultaneously
                // while still using the global thread pool efficiently
                self.inputs
                    .chunks(effective_concurrency)
                    .flat_map(|chunk| {
                        // Process each chunk in parallel within the global pool
                        chunk.par_iter().map(process_one).collect::<Vec<_>>()
                    })
                    .collect()
            }
        });

        Ok(results)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }

    fn reject(&mut self, env: Env, err: napi::Error) -> Result<Self::JsValue> {
        // Use stored error if available, otherwise try to extract from napi::Error
        let lazy_err = self.last_error.take().unwrap_or_else(|| {
            // Fallback: create a generic error from the napi::Error message
            // This should not happen if all error paths properly preserve LazyImageError
            LazyImageError::generic(err.to_string())
        });
        let napi_err = crate::error::napi_error_with_code(&env, lazy_err)?;
        Err(napi_err)
    }
}
