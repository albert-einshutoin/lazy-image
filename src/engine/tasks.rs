// src/engine/tasks.rs
//
// Async task implementations for NAPI.
// These tasks run in background threads and don't block Node.js main thread.

use crate::engine::decoder::{check_dimensions, decode_jpeg_mozjpeg};
use crate::engine::encoder::{encode_avif, encode_jpeg, encode_png, encode_webp};
use crate::engine::io::{extract_icc_profile, Source};
use crate::engine::pipeline::apply_ops;
use crate::engine::pool;
use crate::error::LazyImageError;
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
}

pub(crate) struct EncodeTask {
    pub source: Option<Source>,
    /// Decoded image wrapped in Arc. decode() returns Cow::Borrowed pointing here,
    /// enabling true Copy-on-Write in apply_ops (no deep copy for format-only conversion).
    pub decoded: Option<Arc<DynamicImage>>,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    /// Whether to preserve metadata in output (default: false for security & smaller files)
    pub keep_metadata: bool,
}

impl EncodeTask {
    /// Decode image from source bytes
    /// Uses mozjpeg (libjpeg-turbo) for JPEG, falls back to image crate for others
    ///
    /// **True Copy-on-Write**: Returns `Cow::Borrowed` if image is already decoded,
    /// `Cow::Owned` if decoding was required. The caller can avoid deep copies
    /// when no mutation is needed (e.g., format conversion only).
    pub(crate) fn decode(&self) -> EngineResult<Cow<'_, DynamicImage>> {
        // Prefer already decoded image (already validated)
        // Return borrowed reference - no deep copy until mutation is needed
        if let Some(ref img_arc) = self.decoded {
            return Ok(Cow::Borrowed(img_arc.as_ref()));
        }

        // Get bytes from source - zero-copy for Memory and Mapped sources
        let bytes = self
            .source
            .as_ref()
            .and_then(|s| s.as_bytes())
            .ok_or_else(|| {
                // For Path sources, we need to load them first
                // This should not happen in normal flow as Path sources are converted to Mapped in from_path
                to_engine_error(LazyImageError::source_consumed())
            })?;

        // Check magic bytes for JPEG (0xFF 0xD8)
        let img = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
            // JPEG detected - use mozjpeg for TURBO speed
            decode_jpeg_mozjpeg(bytes)?
        } else {
            // PNG, WebP, etc - use image crate
            image::load_from_memory(bytes).map_err(|e| {
                to_engine_error(LazyImageError::decode_failed(format!("decode failed: {e}")))
            })?
        };

        // Security check: reject decompression bombs
        let (w, h) = img.dimensions();
        check_dimensions(w, h)?;

        Ok(Cow::Owned(img))
    }

    /// Process image: decode → apply ops → encode
    /// This is the core processing pipeline shared by toBuffer and toFile.
    #[cfg_attr(not(any(feature = "napi", feature = "stress")), allow(dead_code))]
    pub(crate) fn process_and_encode(
        &mut self,
        mut metrics: Option<&mut crate::ProcessingMetrics>,
    ) -> EngineResult<Vec<u8>> {
        // 1. Decode
        let start_decode = std::time::Instant::now();
        let img = self.decode()?;
        if let Some(m) = metrics.as_deref_mut() {
            m.decode_time = start_decode.elapsed().as_secs_f64() * 1000.0;
        }

        // 2. Apply operations
        let start_process = std::time::Instant::now();
        let processed = apply_ops(img, &self.ops)?;
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
            OutputFormat::Jpeg { quality } => encode_jpeg(&processed, *quality, icc),
            OutputFormat::Png => encode_png(&processed, icc),
            OutputFormat::WebP { quality } => encode_webp(&processed, *quality, icc),
            OutputFormat::Avif { quality } => encode_avif(&processed, *quality, icc),
        }?;

        if let Some(m) = metrics {
            m.encode_time = start_encode.elapsed().as_secs_f64() * 1000.0;
            // Estimate memory (rough) - prevent overflow
            let (w, h) = processed.dimensions();
            m.memory_peak =
                (w as u64 * h as u64 * 4 + result.len() as u64).min(u32::MAX as u64) as u32;
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
        self.process_and_encode(None)
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        env.create_buffer_with_data(output).map(|b| b.into_raw())
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
        };

        use crate::ProcessingMetrics;
        let mut metrics = ProcessingMetrics::default();
        let data = task.process_and_encode(Some(&mut metrics))?;
        Ok((data, metrics))
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        let (data, metrics) = output;
        let js_buffer = env.create_buffer_with_data(data)?.into_raw();
        Ok(crate::OutputWithMetrics {
            data: js_buffer,
            metrics,
        })
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
    pub output_path: String,
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
        };

        // Process image using shared logic
        let data = encode_task.process_and_encode(None)?;

        // Atomic write: write to temp file in the same directory as target,
        // then rename on success. tempfile automatically cleans up on drop.
        let output_dir = std::path::Path::new(&self.output_path)
            .parent()
            .ok_or_else(|| {
                napi::Error::from(LazyImageError::internal_panic(
                    "output path has no parent directory",
                ))
            })?;

        // Create temp file in the same directory as the target file
        // This ensures rename() works (cross-filesystem rename can fail)
        let mut temp_file = NamedTempFile::new_in(output_dir).map_err(|e| {
            napi::Error::from(LazyImageError::file_write_failed(
                output_dir.to_string_lossy().to_string(),
                e,
            ))
        })?;

        let temp_path = temp_file.path().to_path_buf();
        // Check for overflow: NAPI requires u32, but we can't handle >4GB files
        let bytes_written = data.len().try_into().map_err(|_| {
            napi::Error::from(LazyImageError::internal_panic(
                "file size exceeds 4GB limit (u32::MAX)",
            ))
        })?;
        temp_file.write_all(&data).map_err(|e| {
            napi::Error::from(LazyImageError::file_write_failed(
                temp_path.display().to_string(),
                e,
            ))
        })?;

        // Ensure data is flushed to disk
        temp_file.as_file_mut().sync_all().map_err(|e| {
            napi::Error::from(LazyImageError::file_write_failed(
                temp_path.display().to_string(),
                e,
            ))
        })?;

        // Atomic rename: tempfile handles cleanup automatically if this fails
        temp_file.persist(&self.output_path).map_err(|e| {
            let io_error = std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to persist file: {}", e),
            );
            napi::Error::from(LazyImageError::file_write_failed(
                self.output_path.clone(),
                io_error,
            ))
        })?;

        Ok(bytes_written)
    }
    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

pub(crate) struct BatchTask {
    pub inputs: Vec<String>,
    pub output_dir: String,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub concurrency: u32,
    pub keep_metadata: bool,
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
                napi::Error::from(LazyImageError::file_write_failed(
                    self.output_dir.clone(),
                    e,
                ))
            })?;
        }

        // Helper closure to process a single image
        let ops = &self.ops;
        let format = &self.format;
        let output_dir = &self.output_dir;
        let keep_metadata = self.keep_metadata;
        let process_one = |input_path: &String| -> BatchResult {
            let result = (|| -> Result<String> {
                let data = fs::read(input_path).map_err(|e| {
                    napi::Error::from(LazyImageError::file_read_failed(input_path.clone(), e))
                })?;

                let icc_profile = if keep_metadata {
                    extract_icc_profile(&data).map(Arc::new)
                } else {
                    None
                };

                let img = if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
                    decode_jpeg_mozjpeg(&data)?
                } else {
                    image::load_from_memory(&data).map_err(|e| {
                        napi::Error::from(LazyImageError::decode_failed(format!(
                            "decode failed: {e}"
                        )))
                    })?
                };

                let (w, h) = img.dimensions();
                check_dimensions(w, h)?;

                let processed = apply_ops(Cow::Owned(img), ops)?;

                // Encode - only preserve ICC profile if keep_metadata is true
                let icc = if keep_metadata {
                    icc_profile.as_ref().map(|v| v.as_slice())
                } else {
                    None // Strip metadata by default for security & smaller files
                };
                let encoded = match format {
                    OutputFormat::Jpeg { quality } => {
                        encode_jpeg(&processed, *quality, icc)?
                    }
                    OutputFormat::Png => encode_png(&processed, icc)?,
                    OutputFormat::WebP { quality } => {
                        encode_webp(&processed, *quality, icc)?
                    }
                    OutputFormat::Avif { quality } => {
                        encode_avif(&processed, *quality, icc)?
                    }
                };

                let filename = Path::new(input_path).file_name().ok_or_else(|| {
                    napi::Error::from(LazyImageError::internal_panic("invalid filename"))
                })?;

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

                let mut temp_file = NamedTempFile::new_in(output_dir).map_err(|e| {
                    napi::Error::from(LazyImageError::file_write_failed(output_dir.to_string(), e))
                })?;

                let temp_path = temp_file.path().to_path_buf();
                temp_file.write_all(&encoded).map_err(|e| {
                    napi::Error::from(LazyImageError::file_write_failed(
                        temp_path.display().to_string(),
                        e,
                    ))
                })?;

                temp_file.as_file_mut().sync_all().map_err(|e| {
                    napi::Error::from(LazyImageError::file_write_failed(
                        temp_path.display().to_string(),
                        e,
                    ))
                })?;

                // Atomic rename
                temp_file.persist(&output_path).map_err(|e| {
                    let io_error = std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("failed to persist file: {}", e),
                    );
                    napi::Error::from(LazyImageError::file_write_failed(
                        output_path.display().to_string(),
                        io_error,
                    ))
                })?;

                Ok(output_path.to_string_lossy().to_string())
            })();

            match result {
                Ok(path) => BatchResult {
                    source: input_path.clone(),
                    success: true,
                    error: None,
                    output_path: Some(path),
                },
                Err(e) => {
                    // Preserve error information with context
                    let error_msg = format!("{}: {}", input_path, e);
                    BatchResult {
                        source: input_path.clone(),
                        success: false,
                        error: Some(error_msg),
                        output_path: None,
                    }
                }
            }
        };

        // Validate concurrency parameter
        // concurrency = 0 means "use default" (auto-detected via available_parallelism)
        // concurrency = 1..MAX_CONCURRENCY means "use specified number of threads"
        if self.concurrency > pool::MAX_CONCURRENCY as u32 {
            return Err(napi::Error::from(LazyImageError::internal_panic(format!(
                "invalid concurrency value: {} (must be 0 or 1-{})",
                self.concurrency, pool::MAX_CONCURRENCY
            ))));
        }

        // Use global thread pool for better performance
        let results: Vec<BatchResult> = if self.concurrency == 0 {
            // Use global thread pool with default concurrency
            // (automatically calculated from available_parallelism)
            pool::get_pool().install(|| self.inputs.par_iter().map(process_one).collect())
        } else {
            // For custom concurrency, create a temporary pool with specified threads
            // Note: This creates a new pool per request, which is acceptable
            // for custom concurrency requirements
            use rayon::ThreadPoolBuilder;
            let pool = ThreadPoolBuilder::new()
                .num_threads(self.concurrency as usize)
                .build()
                .map_err(|e| {
                    napi::Error::from(LazyImageError::internal_panic(format!(
                        "failed to create thread pool: {}",
                        e
                    )))
                })?;

            pool.install(|| self.inputs.par_iter().map(process_one).collect())
        };

        Ok(results)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}
