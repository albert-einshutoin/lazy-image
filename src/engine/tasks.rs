// src/engine/tasks.rs
//
// Async task implementations for NAPI.
// These tasks run in background threads and don't block Node.js main thread.

use super::firewall::FirewallConfig;
use crate::engine::decoder::{
    check_dimensions, decode_image, detect_format, ensure_dimensions_safe,
};
use crate::engine::encoder::{
    embed_exif_jpeg, encode_avif, encode_jpeg_with_settings, encode_png, encode_webp,
};
#[allow(unused_imports)]
use crate::engine::io::{extract_exif_raw, extract_icc_profile, Source};
use crate::engine::memory;
use crate::engine::pipeline::{apply_ops_tracked, ColorState, IccState};
#[cfg(feature = "napi")]
use crate::engine::pool;
#[allow(unused_imports)]
use crate::error::{ErrorCategory, LazyImageError};
use crate::ops::{Operation, OutputFormat};
use crate::PROCESSING_METRICS_VERSION;
use image::{DynamicImage, GenericImageView, ImageFormat};
#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
use napi::{Env, JsBuffer, Task};
#[cfg(feature = "napi")]
use rayon::prelude::*;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;

/// Resource usage information for telemetry
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "freebsd"))]
#[derive(Clone, Copy)]
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
#[derive(Clone, Copy)]
struct ResourceUsage {
    cpu_time: f64,
    memory_rss: u64,
}

#[derive(Default)]
struct MetricsContext {
    input_format: Option<String>,
    output_format: String,
    icc_preserved: bool,
    metadata_stripped: bool,
    policy_violations: Vec<String>,
}

fn detect_input_format(bytes: &[u8]) -> Option<String> {
    detect_format(bytes).map(format_to_string)
}

fn format_to_string(fmt: ImageFormat) -> String {
    match fmt {
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::Png => "png",
        ImageFormat::WebP => "webp",
        ImageFormat::Avif => "avif",
        ImageFormat::Gif => "gif",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Ico => "ico",
        ImageFormat::Tiff => "tiff",
        other => other.to_mime_type(),
    }
    .to_string()
}

/// Helper for unified metrics collection.
/// Measures decode -> process -> encode in milliseconds and sets CPU/memory and I/O sizes in one place.
struct MetricsRecorder<'m> {
    metrics: Option<&'m mut crate::ProcessingMetrics>,
    start_total: Instant,
    stage_start: Instant,
    usage_start: Option<ResourceUsage>,
    input_size: u64,
}

impl<'m> MetricsRecorder<'m> {
    fn new(metrics: Option<&'m mut crate::ProcessingMetrics>, input_size: u64) -> Self {
        let now = Instant::now();
        Self {
            metrics,
            start_total: now,
            stage_start: now,
            usage_start: get_resource_usage(),
            input_size,
        }
    }

    fn mark_decode_done(&mut self) {
        if let Some(m) = self.metrics.as_deref_mut() {
            m.decode_ms = self.stage_start.elapsed().as_secs_f64() * 1000.0;
            m.decode_time = m.decode_ms;
            self.stage_start = Instant::now();
        }
    }

    fn mark_process_done(&mut self) {
        if let Some(m) = self.metrics.as_deref_mut() {
            m.ops_ms = self.stage_start.elapsed().as_secs_f64() * 1000.0;
            m.process_time = m.ops_ms;
            self.stage_start = Instant::now();
        }
    }

    fn finalize(
        &mut self,
        processed_dims: (u32, u32),
        output_len: usize,
        usage_end: &Option<ResourceUsage>,
        context: MetricsContext,
    ) {
        if let Some(m) = self.metrics.as_deref_mut() {
            // Encode stage
            m.encode_ms = self.stage_start.elapsed().as_secs_f64() * 1000.0;
            m.encode_time = m.encode_ms;
            // Whole pipeline
            m.total_ms = self.start_total.elapsed().as_secs_f64() * 1000.0;
            m.processing_time = m.total_ms / 1000.0;
            m.version = PROCESSING_METRICS_VERSION.to_string();

            // CPU / memory
            if let (Some(start), Some(end)) = (self.usage_start.as_ref(), usage_end.as_ref()) {
                m.cpu_time = (end.cpu_time - start.cpu_time).max(0.0);
                m.peak_rss = end.memory_rss.min(u32::MAX as u64) as u32;
            } else {
                let (w, h) = processed_dims;
                m.peak_rss =
                    ((w as u64 * h as u64 * 4) + output_len as u64).min(u32::MAX as u64) as u32;
            }
            m.memory_peak = m.peak_rss;

            // Input/output sizes and compression ratio
            m.bytes_in = self.input_size.min(u32::MAX as u64) as u32;
            m.bytes_out = (output_len as u64).min(u32::MAX as u64) as u32;
            m.input_size = m.bytes_in;
            m.output_size = m.bytes_out;
            m.compression_ratio = if m.bytes_in > 0 {
                m.bytes_out as f64 / m.bytes_in as f64
            } else {
                0.0
            };

            // Formats & metadata
            m.format_in = context.input_format;
            m.format_out = context.output_format;
            m.icc_preserved = context.icc_preserved;
            m.metadata_stripped = context.metadata_stripped;
            m.policy_violations = context.policy_violations;
        }
    }
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

pub struct EncodeTask {
    pub source: Option<Source>,
    /// Decoded image wrapped in Arc. decode() returns Cow::Borrowed pointing here,
    /// enabling true Copy-on-Write in apply_ops (no deep copy for format-only conversion).
    pub decoded: Option<Arc<DynamicImage>>,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    /// Whether the input originally had an ICC profile (even if stripped)
    pub icc_present: bool,
    /// Raw EXIF data extracted from source image (for preservation)
    pub exif_data: Option<Arc<Vec<u8>>>,
    pub auto_orient: bool,
    /// Whether to preserve ICC profile in output (default: false for security & smaller files)
    pub keep_icc: bool,
    /// Whether to preserve EXIF metadata in output (default: false for security)
    pub keep_exif: bool,
    /// Whether to strip GPS tags from EXIF (default: true for privacy protection)
    pub strip_gps: bool,
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

        let (img, _detected_format) = decode_image(bytes)?;

        // Security check: reject decompression bombs
        let (w, h) = img.dimensions();
        check_dimensions(w, h)?;
        self.firewall.enforce_pixels(w, h)?;

        Ok(Cow::Owned(img))
    }

    /// Process image: decode → apply ops → encode
    /// This is the core processing pipeline shared by toBuffer and toFile.
    /// Returns LazyImageError directly (not wrapped in napi::Error) so that
    /// Task::reject can properly create error objects with code/category.
    ///
    /// Note: Takes &self (not &mut self) to allow sharing without cloning Arc-wrapped data.
    pub(crate) fn process_and_encode(
        &self,
        mut metrics: Option<&mut crate::ProcessingMetrics>,
    ) -> std::result::Result<Vec<u8>, LazyImageError> {
        // Get input size from source
        // Use len() method which works for both Memory and Mapped sources
        let input_size = self.source.as_ref().map(|s| s.len() as u64).unwrap_or(0);
        let input_bytes = self.source.as_ref().and_then(|s| s.as_bytes());
        let input_format = input_bytes.and_then(detect_input_format);

        // Memory backpressure: estimate before decode and acquire weighted permit
        let estimated_memory = self
            .source
            .as_ref()
            .and_then(|s| s.as_bytes())
            .and_then(|bytes| {
                memory::estimate_memory_from_header(bytes, &self.ops, Some(&self.format))
            })
            .unwrap_or(memory::ESTIMATED_MEMORY_PER_OPERATION);
        let permit = memory::memory_semaphore().acquire(estimated_memory);
        // keep guard alive for entire processing scope
        let _permit_guard = permit;

        // Centralize metrics recording
        let mut metrics_recorder = MetricsRecorder::new(metrics.as_deref_mut(), input_size);

        // Pre-read orientation from EXIF header (before full decode)
        let orientation = if self.auto_orient {
            if let Some(bytes) = input_bytes {
                // Enforce byte limit & metadata scan before EXIF parsing to honor firewall settings
                self.firewall.enforce_source_len(bytes.len())?;
                self.firewall.scan_metadata(bytes)?;
                crate::engine::decoder::detect_exif_orientation(bytes)
            } else {
                None
            }
        } else {
            None
        };

        // 1. Decode
        let img = self.decode_internal()?;
        self.firewall
            .enforce_timeout(metrics_recorder.start_total, "decode")?;
        metrics_recorder.mark_decode_done();

        // 2. Apply operations
        let mut effective_ops = self.ops.clone();
        if let Some(o) = orientation {
            // Insert at the very beginning to normalize before user operations
            effective_ops.insert(0, Operation::AutoOrient { orientation: o });
        }
        let icc_state = if self.icc_present {
            IccState::Present
        } else {
            IccState::Absent
        };
        let initial_state = ColorState::from_dynamic_image(&img, icc_state);
        let tracked = apply_ops_tracked(img, &effective_ops, initial_state)?;
        let final_color_state = tracked.state;
        let processed = tracked.image;
        self.firewall
            .enforce_timeout(metrics_recorder.start_total, "process")?;
        metrics_recorder.mark_process_done();

        // 3. Encode - only preserve ICC profile if keep_icc is true
        let icc = if self.keep_icc {
            self.icc_profile.as_ref().map(|v| v.as_slice())
        } else {
            None // Strip metadata by default for security & smaller files
        };

        // 4. Encode image to target format
        let mut result = match &self.format {
            OutputFormat::Jpeg { quality, fast_mode } => {
                encode_jpeg_with_settings(&processed, *quality, icc, *fast_mode)
            }
            OutputFormat::Png => encode_png(&processed, icc),
            OutputFormat::WebP { quality } => encode_webp(&processed, *quality, icc),
            OutputFormat::Avif { quality } => encode_avif(&processed, *quality, icc),
        }?;

        // 5. Embed EXIF metadata if requested (JPEG only for now)
        if self.keep_exif {
            if let Some(exif_data) = &self.exif_data {
                if let OutputFormat::Jpeg { .. } = &self.format {
                    // Embed EXIF with sanitization:
                    // - Reset Orientation to 1 if auto_orient was applied
                    // - Strip GPS tags if strip_gps is true (default)
                    result = embed_exif_jpeg(
                        result,
                        exif_data.as_slice(),
                        self.auto_orient, // reset orientation if auto-orient was applied
                        self.strip_gps,
                    )?;
                }
                // TODO: PNG/WebP EXIF embedding (less common, lower priority)
            }
        }
        self.firewall
            .enforce_timeout(metrics_recorder.start_total, "encode")?;

        // Get final resource usage & finalize metrics
        let final_usage = get_resource_usage();
        // Use tracked color state to reason about ICC preservation.
        let icc_present = matches!(final_color_state.icc, IccState::Present);
        let icc_preserved = self.keep_icc && icc_present;
        // metadata_stripped: true when source had ICC but we did not preserve it
        let metadata_stripped = icc_present && !icc_preserved;
        let metadata_blocked_by_policy =
            (self.keep_icc || self.keep_exif) && self.firewall.reject_metadata && icc_present;
        let mut policy_violations = Vec::new();
        if metadata_blocked_by_policy {
            policy_violations.push("firewall_rejected_metadata".to_string());
        }

        let metrics_context = MetricsContext {
            input_format,
            output_format: self.format.as_str().to_string(),
            icc_preserved,
            metadata_stripped,
            policy_violations,
        };
        metrics_recorder.finalize(
            processed.dimensions(),
            result.len(),
            &final_usage,
            metrics_context,
        );

        Ok(result)
    }
}

// Test-only helper for decode assertions in engine.rs tests.
#[cfg(test)]
impl EncodeTask {
    pub(crate) fn decode(&self) -> std::result::Result<Cow<'_, DynamicImage>, LazyImageError> {
        self.decode_internal()
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

// ============================================================================================
// Tests for coverage when NAPI is disabled (CI runs coverage with --no-default-features)
// ============================================================================================
#[cfg(all(test, not(feature = "napi")))]
mod non_napi_tests {
    use super::*;
    use crate::engine::firewall::FirewallConfig;
    use crate::engine::io::Source;
    use crate::ops::ResizeFit;
    use image::{ImageBuffer, ImageFormat, Rgba};

    fn sample_png_bytes() -> Vec<u8> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn make_task_with_decoded(format: OutputFormat) -> EncodeTask {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        EncodeTask {
            source: None,
            decoded: Some(Arc::new(dyn_img)),
            ops: vec![Operation::Resize {
                width: Some(2),
                height: Some(2),
                fit: ResizeFit::Inside,
            }],
            format,
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: true,
            keep_icc: false,
            keep_exif: false,
            strip_gps: true,
            firewall: FirewallConfig::disabled(),
        }
    }

    #[test]
    fn process_and_encode_outputs_image() {
        let mut task = make_task_with_decoded(OutputFormat::Png);
        let encoded = task
            .process_and_encode(None)
            .expect("encode should succeed");
        assert!(!encoded.is_empty());
    }

    #[test]
    fn decode_internal_errors_when_source_missing() {
        let task = EncodeTask {
            source: None,
            decoded: None,
            ops: vec![],
            format: OutputFormat::Png,
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: true,
            keep_icc: false,
            keep_exif: false,
            strip_gps: true,
            firewall: FirewallConfig::disabled(),
        };
        let err = task.decode_internal().unwrap_err();
        assert!(matches!(err, LazyImageError::SourceConsumed));
    }

    #[test]
    fn firewall_limits_are_enforced_on_decode() {
        let png = sample_png_bytes();
        let mut firewall = FirewallConfig::custom();
        firewall.max_bytes = Some(1); // smaller than PNG size to force rejection

        let task = EncodeTask {
            source: Some(Source::Memory(Arc::new(png))),
            decoded: None,
            ops: vec![],
            format: OutputFormat::Png,
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: true,
            keep_icc: false,
            keep_exif: false,
            strip_gps: true,
            firewall,
        };
        let err = task.decode_internal().unwrap_err();
        assert!(matches!(err, LazyImageError::FirewallViolation { .. }));
    }
}

pub struct EncodeWithMetricsTask {
    pub source: Option<Source>,
    /// Decoded image wrapped in Arc for sharing. See EncodeTask for Copy-on-Write details.
    pub decoded: Option<Arc<DynamicImage>>,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    pub icc_present: bool,
    /// Raw EXIF data extracted from source image (for preservation)
    pub exif_data: Option<Arc<Vec<u8>>>,
    pub auto_orient: bool,
    /// Whether to preserve ICC profile in output
    pub keep_icc: bool,
    /// Whether to preserve EXIF metadata in output
    pub keep_exif: bool,
    /// Whether to strip GPS tags from EXIF
    pub strip_gps: bool,
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
        // P2 Optimization: Share EncodeTask fields via reference instead of cloning
        // Since EncodeWithMetricsTask has the same fields as EncodeTask, create a view
        let task = EncodeTask {
            source: self.source.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
            format: self.format.clone(),
            icc_profile: self.icc_profile.clone(),
            icc_present: self.icc_present,
            exif_data: self.exif_data.clone(),
            auto_orient: self.auto_orient,
            keep_icc: self.keep_icc,
            keep_exif: self.keep_exif,
            strip_gps: self.strip_gps,
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
                self.last_error = Some(lazy_err.clone());
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

pub struct WriteFileTask {
    pub source: Option<Source>,
    /// Decoded image wrapped in Arc for sharing. See EncodeTask for Copy-on-Write details.
    pub decoded: Option<Arc<DynamicImage>>,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    pub icc_present: bool,
    /// Raw EXIF data extracted from source image (for preservation)
    pub exif_data: Option<Arc<Vec<u8>>>,
    pub auto_orient: bool,
    /// Whether to preserve ICC profile in output
    pub keep_icc: bool,
    /// Whether to preserve EXIF metadata in output
    pub keep_exif: bool,
    /// Whether to strip GPS tags from EXIF
    pub strip_gps: bool,
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

        // P2 Optimization: Create EncodeTask without redundant mut
        let encode_task = EncodeTask {
            source: self.source.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
            format: self.format.clone(),
            icc_profile: self.icc_profile.clone(),
            icc_present: self.icc_present,
            exif_data: self.exif_data.clone(),
            auto_orient: self.auto_orient,
            keep_icc: self.keep_icc,
            keep_exif: self.keep_exif,
            strip_gps: self.strip_gps,
            firewall: self.firewall.clone(),
            #[cfg(feature = "napi")]
            last_error: None,
        };

        // Process image using shared logic (now using &self not &mut self)
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
                let lazy_err = LazyImageError::invalid_argument(
                    "path",
                    self.output_path.clone(),
                    "output path must include a parent directory",
                );
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

pub struct BatchTask {
    pub inputs: Vec<String>,
    pub output_dir: String,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub concurrency: u32,
    pub auto_orient: bool,
    /// Whether to preserve ICC profile in output
    pub keep_icc: bool,
    /// Whether to preserve EXIF metadata in output
    pub keep_exif: bool,
    /// Whether to strip GPS tags from EXIF
    pub strip_gps: bool,
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
        let keep_icc = self.keep_icc;
        let keep_exif = self.keep_exif;
        let strip_gps = self.strip_gps;
        let firewall = self.firewall.clone();
        let process_one = |input_path: &String| -> BatchResult {
            let result = (|| -> std::result::Result<String, LazyImageError> {
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
                // If modified, decoding may fail, produce corrupted images, or cause OS-dependent SIGBUS/SIGSEGV.
                // On Windows, deleting a memory-mapped file fails (platform limitation).
                let mmap = unsafe {
                    Mmap::map(&file)
                        .map_err(|e| LazyImageError::mmap_failed(input_path.clone(), e))?
                };
                let mmap_arc = Arc::new(mmap);
                let data = mmap_arc.as_ref();

                firewall.enforce_source_len(data.len())?;
                firewall.scan_metadata(data)?;

                let estimated_memory =
                    memory::estimate_memory_from_header(data, &ops, Some(format))
                        .unwrap_or(memory::ESTIMATED_MEMORY_PER_OPERATION);
                let _permit_guard = memory::memory_semaphore().acquire(estimated_memory);

                let start_total = std::time::Instant::now();

                let orientation = if self.auto_orient {
                    crate::engine::decoder::detect_exif_orientation(data)
                } else {
                    None
                };
                let icc_profile = if keep_icc {
                    extract_icc_profile(data)?.map(Arc::new)
                } else {
                    None
                };
                // Extract EXIF data for preservation (JPEG only)
                let exif_data = if keep_exif {
                    extract_exif_raw(data).map(Arc::new)
                } else {
                    None
                };

                let (img, _detected_format) = decode_image(data)?;
                firewall.enforce_timeout(start_total, "decode")?;

                let (w, h) = img.dimensions();
                check_dimensions(w, h)?;
                firewall.enforce_pixels(w, h)?;

                let mut effective_ops = ops.clone();
                if let Some(o) = orientation {
                    effective_ops.insert(0, Operation::AutoOrient { orientation: o });
                }

                let icc_state = if icc_profile.is_some() {
                    IccState::Present
                } else {
                    IccState::Absent
                };
                let initial_state = ColorState::from_dynamic_image(&img, icc_state);
                let tracked = apply_ops_tracked(Cow::Owned(img), &effective_ops, initial_state)?;
                let processed = tracked.image;
                firewall.enforce_timeout(start_total, "process")?;

                // Encode - only preserve ICC profile if keep_icc is true
                let icc = if keep_icc {
                    icc_profile.as_ref().map(|v| v.as_slice())
                } else {
                    None // Strip metadata by default for security & smaller files
                };

                let mut encoded = match format {
                    OutputFormat::Jpeg { quality, fast_mode } => {
                        encode_jpeg_with_settings(&processed, *quality, icc, *fast_mode)?
                    }
                    OutputFormat::Png => encode_png(&processed, icc)?,
                    OutputFormat::WebP { quality } => encode_webp(&processed, *quality, icc)?,
                    OutputFormat::Avif { quality } => encode_avif(&processed, *quality, icc)?,
                };

                // Embed EXIF metadata if requested (JPEG only)
                if keep_exif {
                    if let Some(ref exif) = exif_data {
                        if let OutputFormat::Jpeg { .. } = format {
                            encoded = embed_exif_jpeg(
                                encoded,
                                exif.as_slice(),
                                self.auto_orient, // reset orientation if auto-orient applied
                                strip_gps,
                            )?;
                        }
                    }
                }

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
                    let error_code = err.code();
                    let error_msg = format!("[{}] {}: {}", error_code.as_str(), input_path, err);
                    let category = error_code.category();
                    BatchResult {
                        source: input_path.clone(),
                        success: false,
                        error: Some(error_msg),
                        output_path: None,
                        error_code: Some(error_code.as_str().to_string()),
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

        // Note: effective_concurrency is not used for manual chunking anymore.
        // Memory backpressure is handled automatically by WeightedSemaphore,
        // and Rayon's work-stealing scheduler manages thread utilization.
        // The concurrency parameter now primarily affects the thread pool size
        // (see pool::calculate_optimal_concurrency() for CPU/memory-based limits).

        // Use global thread pool with Rayon's work-stealing scheduler
        // Memory backpressure is automatically handled by WeightedSemaphore in process_and_encode()
        // (see memory.rs:72-84 for acquire/release logic)
        // This eliminates sequential chunk processing that leaves threads idle between chunks
        let results: Vec<BatchResult> = pool::get_pool().install(|| {
            self.inputs.par_iter().map(process_one).collect()
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
