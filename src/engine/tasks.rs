#![cfg_attr(not(feature = "napi"), allow(dead_code))]

// src/engine/tasks.rs
//
// Async task implementations for NAPI.
// These tasks run in background threads and don't block Node.js main thread.

use super::firewall::FirewallConfig;
use crate::engine::decoder::{
    check_dimensions, decode_image, detect_format, ensure_dimensions_safe,
};
use crate::engine::encoder::{
    embed_exif_jpeg, embed_exif_png, embed_exif_webp, encode_avif, encode_jpeg_with_settings,
    encode_png, encode_webp,
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
use napi::{bindgen_prelude::BufferSlice, Env, Task};
#[cfg(feature = "napi")]
use rayon::prelude::*;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;

// Resource usage delegates to the centralized platform module.
use super::platform;
type ResourceUsage = platform::ResourceUsage;

fn get_resource_usage() -> Option<ResourceUsage> {
    platform::get_resource_usage()
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
            self.stage_start = Instant::now();
        }
    }

    fn mark_process_done(&mut self) {
        if let Some(m) = self.metrics.as_deref_mut() {
            m.ops_ms = self.stage_start.elapsed().as_secs_f64() * 1000.0;
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

            // Input/output sizes and compression ratio
            m.bytes_in = self.input_size.min(u32::MAX as u64) as u32;
            m.bytes_out = (output_len as u64).min(u32::MAX as u64) as u32;
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
    pub effective_concurrency: Option<u32>,
    pub auto_concurrency: Option<bool>,
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

fn decode_internal_from_parts<'a>(
    source: Option<&'a Source>,
    decoded: Option<&'a Arc<DynamicImage>>,
    firewall: &FirewallConfig,
) -> std::result::Result<Cow<'a, DynamicImage>, LazyImageError> {
    // Prefer already decoded image (already validated)
    // Return borrowed reference - no deep copy until mutation is needed
    if let Some(img_arc) = decoded {
        check_dimensions(img_arc.width(), img_arc.height())?;
        firewall.enforce_pixels(img_arc.width(), img_arc.height())?;
        return Ok(Cow::Borrowed(img_arc.as_ref()));
    }

    // Get bytes from source - zero-copy for Memory and Mapped sources
    let bytes = match source {
        Some(source) => {
            if let Some(bytes) = source.as_bytes() {
                bytes
            } else {
                // Path sources require loading first - this should not happen in normal flow
                // as from_path() converts Path to Mapped. If this occurs, it's a programming error.
                return Err(LazyImageError::decode_failed(
                    "Path source requires loading first. Use Mapped source (from_path) instead."
                        .to_string(),
                ));
            }
        }
        None => {
            return Err(LazyImageError::source_consumed());
        }
    };

    firewall.enforce_source_len(bytes.len())?;
    firewall.scan_metadata(bytes)?;

    ensure_dimensions_safe(bytes)?;

    let (img, _detected_format) = decode_image(bytes)?;

    // Security check: reject decompression bombs
    let (w, h) = img.dimensions();
    check_dimensions(w, h)?;
    firewall.enforce_pixels(w, h)?;

    Ok(Cow::Owned(img))
}

#[allow(clippy::too_many_arguments)]
fn process_and_encode_from_parts(
    source: Option<&Source>,
    decoded: Option<&Arc<DynamicImage>>,
    ops: &[Operation],
    format: &OutputFormat,
    icc_profile: Option<&Arc<Vec<u8>>>,
    icc_present: bool,
    exif_data: Option<&Arc<Vec<u8>>>,
    auto_orient: bool,
    keep_icc: bool,
    keep_exif: bool,
    strip_gps: bool,
    firewall: &FirewallConfig,
    metrics: Option<&mut crate::ProcessingMetrics>,
) -> std::result::Result<Vec<u8>, LazyImageError> {
    // Get input size from source
    // Use len() method which works for both Memory and Mapped sources
    let input_size = source.map(|s| s.len() as u64).unwrap_or(0);
    let input_bytes = source.and_then(|s| s.as_bytes());
    let input_format = input_bytes.and_then(detect_input_format);

    // Memory backpressure: estimate before decode and acquire weighted permit.
    // Use file-size-based fallback instead of hard-coded 100MB when header parsing fails.
    let estimated_memory = source
        .and_then(|s| s.as_bytes())
        .and_then(|bytes| memory::estimate_memory_from_header(bytes, ops, Some(format)))
        .unwrap_or_else(|| {
            if source.is_none() {
                return memory::ESTIMATED_MEMORY_PER_OPERATION;
            }
            let detected_format = source
                .and_then(|s| s.as_bytes())
                .and_then(crate::engine::decoder::detect_format);
            memory::estimate_fallback_from_file_size(input_size, detected_format)
        });
    let source_reserved = source.map(|s| s.reserved_memory_bytes()).unwrap_or(0);
    let sem = memory::memory_semaphore();
    let total_memory = estimated_memory
        .saturating_add(source_reserved)
        .min(sem.capacity());
    let permit = sem.acquire(total_memory);
    // keep guard alive for entire processing scope
    let _permit_guard = permit;

    // Centralize metrics recording
    let mut metrics_recorder = MetricsRecorder::new(metrics, input_size);

    // Pre-read orientation from EXIF header (before full decode)
    let orientation = if auto_orient {
        if let Some(bytes) = input_bytes {
            // Enforce byte limit & metadata scan before EXIF parsing to honor firewall settings
            firewall.enforce_source_len(bytes.len())?;
            firewall.scan_metadata(bytes)?;
            crate::engine::decoder::detect_exif_orientation(bytes)
        } else {
            None
        }
    } else {
        None
    };

    // 1. Decode
    let img = decode_internal_from_parts(source, decoded, firewall)?;
    firewall.enforce_timeout(metrics_recorder.start_total, "decode")?;
    metrics_recorder.mark_decode_done();

    // 2. Apply operations
    let with_auto_orient;
    let effective_ops = if let Some(o) = orientation {
        // Clone only when we need to inject AutoOrient at the front.
        with_auto_orient = {
            let mut op_list = ops.to_vec();
            op_list.insert(0, Operation::AutoOrient { orientation: o });
            op_list
        };
        with_auto_orient.as_slice()
    } else {
        ops
    };
    let icc_state = if icc_present {
        IccState::Present
    } else {
        IccState::Absent
    };
    let initial_state = ColorState::from_dynamic_image(&img, icc_state);
    let tracked = apply_ops_tracked(img, effective_ops, initial_state)?;
    let final_color_state = tracked.state;
    let processed = tracked.image;
    firewall.enforce_timeout(metrics_recorder.start_total, "process")?;
    metrics_recorder.mark_process_done();

    // 3. Encode - only preserve ICC profile if keep_icc is true
    let icc = if keep_icc {
        icc_profile.map(|v| v.as_slice())
    } else {
        None // Strip metadata by default for security & smaller files
    };

    // 4. Encode image to target format
    let mut result = match format {
        OutputFormat::Jpeg { quality, fast_mode } => {
            encode_jpeg_with_settings(&processed, *quality, icc, *fast_mode)
        }
        OutputFormat::Png => encode_png(&processed, icc),
        OutputFormat::WebP { quality } => encode_webp(&processed, *quality, icc),
        OutputFormat::Avif { quality } => encode_avif(&processed, *quality, icc),
    }?;

    // 5. Embed EXIF metadata if requested.
    if keep_exif {
        if let Some(exif_data) = exif_data {
            result = match format {
                OutputFormat::Jpeg { .. } => {
                    embed_exif_jpeg(result, exif_data.as_slice(), auto_orient, strip_gps)?
                }
                OutputFormat::Png => {
                    embed_exif_png(result, exif_data.as_slice(), auto_orient, strip_gps)?
                }
                OutputFormat::WebP { .. } => {
                    embed_exif_webp(result, exif_data.as_slice(), auto_orient, strip_gps)?
                }
                OutputFormat::Avif { .. } => result,
            };
        }
    }
    firewall.enforce_timeout(metrics_recorder.start_total, "encode")?;

    // Get final resource usage & finalize metrics
    let final_usage = get_resource_usage();
    // Use tracked color state to reason about ICC preservation.
    let icc_present = matches!(final_color_state.icc, IccState::Present);
    let icc_preserved = keep_icc && icc_present;
    // metadata_stripped: true when source had ICC but we did not preserve it
    let metadata_stripped = icc_present && !icc_preserved;
    let metadata_blocked_by_policy =
        (keep_icc || keep_exif) && firewall.reject_metadata && icc_present;
    let mut policy_violations = Vec::new();
    if metadata_blocked_by_policy {
        policy_violations.push("firewall_rejected_metadata".to_string());
    }

    let metrics_context = MetricsContext {
        input_format,
        output_format: format.as_str().to_string(),
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
        decode_internal_from_parts(self.source.as_ref(), self.decoded.as_ref(), &self.firewall)
    }

    /// Process image: decode → apply ops → encode
    /// This is the core processing pipeline shared by toBuffer and toFile.
    /// Returns LazyImageError directly (not wrapped in napi::Error) so that
    /// Task::reject can properly create error objects with code/category.
    ///
    /// Note: Takes &self (not &mut self) to allow sharing without cloning Arc-wrapped data.
    pub(crate) fn process_and_encode(
        &self,
        metrics: Option<&mut crate::ProcessingMetrics>,
    ) -> std::result::Result<Vec<u8>, LazyImageError> {
        process_and_encode_from_parts(
            self.source.as_ref(),
            self.decoded.as_ref(),
            &self.ops,
            &self.format,
            self.icc_profile.as_ref(),
            self.icc_present,
            self.exif_data.as_ref(),
            self.auto_orient,
            self.keep_icc,
            self.keep_exif,
            self.strip_gps,
            &self.firewall,
            metrics,
        )
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
    type JsValue = napi::bindgen_prelude::Buffer;

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
        BufferSlice::from_data(&env, output)?.into_buffer(&env)
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
        let task = make_task_with_decoded(OutputFormat::Png);
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
            source: Some(Source::from_vec(png)),
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

    #[test]
    fn auto_orient_flag_disables_orientation_lookup() {
        // When auto_orient is false, process_and_encode should NOT insert
        // an AutoOrient operation even if the source has EXIF orientation.
        // We test this indirectly: a PNG source has no EXIF, so orientation
        // is None regardless. But we verify the flag is stored correctly.
        let png = sample_png_bytes();
        let task = EncodeTask {
            source: Some(Source::from_vec(png)),
            decoded: None,
            ops: vec![],
            format: OutputFormat::Png,
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: false,
            keep_icc: false,
            keep_exif: false,
            strip_gps: true,
            firewall: FirewallConfig::disabled(),
        };
        assert!(!task.auto_orient);
        // process_and_encode should succeed (no orientation applied)
        let result = task.process_and_encode(None);
        assert!(result.is_ok());
    }

    #[test]
    fn auto_orient_flag_default_is_true() {
        // Verify the default task helper sets auto_orient to true
        let task = make_task_with_decoded(OutputFormat::Png);
        assert!(task.auto_orient);
    }

    #[test]
    fn fast_mode_propagates_to_jpeg_encode() {
        // Verify that OutputFormat::Jpeg with fast_mode=true produces valid JPEG
        let task = make_task_with_decoded(OutputFormat::Jpeg {
            quality: 80,
            fast_mode: true,
        });
        let result = task
            .process_and_encode(None)
            .expect("encode should succeed");
        // JPEG magic bytes
        assert_eq!(&result[0..2], &[0xFF, 0xD8]);
        assert_eq!(&result[result.len() - 2..], &[0xFF, 0xD9]);
    }

    #[test]
    fn fast_mode_false_produces_valid_jpeg() {
        let task = make_task_with_decoded(OutputFormat::Jpeg {
            quality: 80,
            fast_mode: false,
        });
        let result = task
            .process_and_encode(None)
            .expect("encode should succeed");
        assert_eq!(&result[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn fast_mode_true_vs_false_both_valid() {
        let fast_task = make_task_with_decoded(OutputFormat::Jpeg {
            quality: 80,
            fast_mode: true,
        });
        let normal_task = make_task_with_decoded(OutputFormat::Jpeg {
            quality: 80,
            fast_mode: false,
        });
        let fast_result = fast_task.process_and_encode(None).unwrap();
        let normal_result = normal_task.process_and_encode(None).unwrap();
        // Both should produce valid JPEGs
        assert_eq!(&fast_result[0..2], &[0xFF, 0xD8]);
        assert_eq!(&normal_result[0..2], &[0xFF, 0xD8]);
        // Both should be non-empty
        assert!(!fast_result.is_empty());
        assert!(!normal_result.is_empty());
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
        use crate::ProcessingMetrics;
        let mut metrics = ProcessingMetrics::default();
        match process_and_encode_from_parts(
            self.source.as_ref(),
            self.decoded.as_ref(),
            &self.ops,
            &self.format,
            self.icc_profile.as_ref(),
            self.icc_present,
            self.exif_data.as_ref(),
            self.auto_orient,
            self.keep_icc,
            self.keep_exif,
            self.strip_gps,
            &self.firewall,
            Some(&mut metrics),
        ) {
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
        let js_buffer = BufferSlice::from_data(&env, data)?.into_buffer(&env)?;
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

fn write_encoded_output_with_count(
    output_path: &str,
    data: &[u8],
) -> std::result::Result<u32, LazyImageError> {
    use std::io::Write;
    use tempfile::NamedTempFile;

    let output_dir = std::path::Path::new(output_path).parent().ok_or_else(|| {
        LazyImageError::invalid_argument(
            "path",
            output_path.to_string(),
            "output path must include a parent directory",
        )
    })?;

    let mut temp_file = NamedTempFile::new_in(output_dir).map_err(|e| {
        LazyImageError::file_write_failed(output_dir.to_string_lossy().to_string(), e)
    })?;

    let temp_path = temp_file.path().to_path_buf();
    let bytes_written = data
        .len()
        .try_into()
        .map_err(|_| LazyImageError::internal_panic("file size exceeds 4GB limit (u32::MAX)"))?;
    temp_file
        .write_all(data)
        .map_err(|e| LazyImageError::file_write_failed(temp_path.display().to_string(), e))?;
    temp_file
        .as_file_mut()
        .sync_all()
        .map_err(|e| LazyImageError::file_write_failed(temp_path.display().to_string(), e))?;

    temp_file.persist(output_path).map_err(|e| {
        let io_error = std::io::Error::other(format!("failed to persist file: {}", e));
        LazyImageError::file_write_failed(output_path.to_string(), io_error)
    })?;

    Ok(bytes_written)
}

fn write_encoded_output(output_path: &str, data: &[u8]) -> std::result::Result<(), LazyImageError> {
    use std::io::Write;
    use tempfile::NamedTempFile;

    let output_dir = std::path::Path::new(output_path).parent().ok_or_else(|| {
        LazyImageError::invalid_argument(
            "path",
            output_path.to_string(),
            "output path must include a parent directory",
        )
    })?;

    let mut temp_file = NamedTempFile::new_in(output_dir).map_err(|e| {
        LazyImageError::file_write_failed(output_dir.to_string_lossy().to_string(), e)
    })?;

    let temp_path = temp_file.path().to_path_buf();
    temp_file
        .write_all(data)
        .map_err(|e| LazyImageError::file_write_failed(temp_path.display().to_string(), e))?;
    temp_file
        .as_file_mut()
        .sync_all()
        .map_err(|e| LazyImageError::file_write_failed(temp_path.display().to_string(), e))?;

    temp_file.persist(output_path).map_err(|e| {
        let io_error = std::io::Error::other(format!("failed to persist file: {}", e));
        LazyImageError::file_write_failed(output_path.to_string(), io_error)
    })?;

    Ok(())
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

pub struct WriteFileWithMetricsTask {
    pub source: Option<Source>,
    pub decoded: Option<Arc<DynamicImage>>,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    pub icc_present: bool,
    pub exif_data: Option<Arc<Vec<u8>>>,
    pub auto_orient: bool,
    pub keep_icc: bool,
    pub keep_exif: bool,
    pub strip_gps: bool,
    pub firewall: FirewallConfig,
    pub output_path: String,
    #[cfg(feature = "napi")]
    pub(crate) last_error: Option<LazyImageError>,
}

#[cfg(feature = "napi")]
#[napi]
impl Task for WriteFileTask {
    type Output = u32; // Bytes written
    type JsValue = u32;

    fn compute(&mut self) -> Result<Self::Output> {
        let data = match process_and_encode_from_parts(
            self.source.as_ref(),
            self.decoded.as_ref(),
            &self.ops,
            &self.format,
            self.icc_profile.as_ref(),
            self.icc_present,
            self.exif_data.as_ref(),
            self.auto_orient,
            self.keep_icc,
            self.keep_exif,
            self.strip_gps,
            &self.firewall,
            None,
        ) {
            Ok(data) => data,
            Err(lazy_err) => {
                // Store the error for use in reject
                self.last_error = Some(lazy_err.clone());
                return Err(napi::Error::from(lazy_err));
            }
        };

        match write_encoded_output_with_count(&self.output_path, &data) {
            Ok(bytes_written) => Ok(bytes_written),
            Err(lazy_err) => {
                self.last_error = Some(lazy_err.clone());
                Err(napi::Error::from(lazy_err))
            }
        }
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

#[cfg(feature = "napi")]
#[napi]
impl Task for WriteFileWithMetricsTask {
    type Output = (u32, crate::ProcessingMetrics);
    type JsValue = crate::FileOutputWithMetrics;

    fn compute(&mut self) -> Result<Self::Output> {
        use crate::ProcessingMetrics;

        let mut metrics = ProcessingMetrics::default();
        let data = match process_and_encode_from_parts(
            self.source.as_ref(),
            self.decoded.as_ref(),
            &self.ops,
            &self.format,
            self.icc_profile.as_ref(),
            self.icc_present,
            self.exif_data.as_ref(),
            self.auto_orient,
            self.keep_icc,
            self.keep_exif,
            self.strip_gps,
            &self.firewall,
            Some(&mut metrics),
        ) {
            Ok(data) => data,
            Err(lazy_err) => {
                self.last_error = Some(lazy_err.clone());
                return Err(napi::Error::from(lazy_err));
            }
        };

        match write_encoded_output_with_count(&self.output_path, &data) {
            Ok(bytes_written) => Ok((bytes_written, metrics)),
            Err(lazy_err) => {
                self.last_error = Some(lazy_err.clone());
                Err(napi::Error::from(lazy_err))
            }
        }
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        let (bytes_written, metrics) = output;
        Ok(crate::FileOutputWithMetrics {
            bytes_written,
            metrics,
        })
    }

    fn reject(&mut self, env: Env, err: napi::Error) -> Result<Self::JsValue> {
        let lazy_err = self
            .last_error
            .take()
            .unwrap_or_else(|| LazyImageError::generic(err.to_string()));
        let napi_err = crate::error::napi_error_with_code(&env, lazy_err)?;
        Err(napi_err)
    }
}

struct BatchFileSuccess {
    output_path: String,
    metrics: Option<crate::ProcessingMetrics>,
}

#[allow(clippy::too_many_arguments)]
fn process_batch_file(
    input_path: &str,
    output_dir: &str,
    ops: &[Operation],
    format: &OutputFormat,
    auto_orient: bool,
    keep_icc: bool,
    keep_exif: bool,
    strip_gps: bool,
    firewall: &FirewallConfig,
    collect_metrics: bool,
) -> std::result::Result<BatchFileSuccess, LazyImageError> {
    use std::path::Path;
    use std::sync::Arc;

    let input_len = std::fs::metadata(input_path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                LazyImageError::file_not_found(input_path.to_string())
            } else {
                LazyImageError::file_read_failed(input_path.to_string(), e)
            }
        })?
        .len() as usize;
    firewall.enforce_source_len(input_len)?;

    let source = crate::engine::io::load_file_safe(Path::new(input_path))?;
    let data = source.as_bytes().expect("source always has bytes");

    firewall.scan_metadata(data)?;

    let extracted_icc = extract_icc_profile(data)?;
    let icc_present = extracted_icc.is_some();
    let icc_profile = if keep_icc {
        extracted_icc.map(Arc::new)
    } else {
        None
    };
    let exif_data = if keep_exif {
        extract_exif_raw(data).map(Arc::new)
    } else {
        None
    };

    let mut metrics = collect_metrics.then(crate::ProcessingMetrics::default);
    let encoded = process_and_encode_from_parts(
        Some(&source),
        None,
        ops,
        format,
        icc_profile.as_ref(),
        icc_present,
        exif_data.as_ref(),
        auto_orient,
        keep_icc,
        keep_exif,
        strip_gps,
        firewall,
        metrics.as_mut(),
    )?;

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
    write_encoded_output(output_path.to_string_lossy().as_ref(), &encoded)?;

    Ok(BatchFileSuccess {
        output_path: output_path.to_string_lossy().to_string(),
        metrics,
    })
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

pub struct BatchWithMetricsTask {
    pub inputs: Vec<String>,
    pub output_dir: String,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub concurrency: u32,
    pub auto_orient: bool,
    pub keep_icc: bool,
    pub keep_exif: bool,
    pub strip_gps: bool,
    pub firewall: FirewallConfig,
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

        // concurrency = 0 means "use default" (auto-detected via CPU and memory)
        // concurrency = 1..MAX_CONCURRENCY means "use specified number of concurrent operations"
        if self.concurrency > pool::MAX_CONCURRENCY as u32 {
            let lazy_err = LazyImageError::invalid_argument(
                "concurrency",
                self.concurrency.to_string(),
                format!("must be 0 or 1-{}", pool::MAX_CONCURRENCY),
            );
            self.last_error = Some(lazy_err.clone());
            return Err(napi::Error::from(lazy_err));
        }

        let effective_concurrency = pool::resolve_effective_concurrency(self.concurrency);
        let effective_concurrency_u32 = u32::try_from(effective_concurrency).ok();
        let auto_concurrency = Some(self.concurrency == 0);

        // Helper closure to process a single image
        let ops = &self.ops;
        let format = &self.format;
        let output_dir = &self.output_dir;
        let keep_icc = self.keep_icc;
        let keep_exif = self.keep_exif;
        let strip_gps = self.strip_gps;
        let firewall = self.firewall.clone();
        let process_one = |input_path: &String| -> BatchResult {
            let result = process_batch_file(
                input_path,
                output_dir,
                ops,
                format,
                self.auto_orient,
                keep_icc,
                keep_exif,
                strip_gps,
                &firewall,
                false,
            );

            match result {
                Ok(output) => BatchResult {
                    source: input_path.clone(),
                    success: true,
                    error: None,
                    output_path: Some(output.output_path),
                    error_code: None,
                    error_category: None,
                    effective_concurrency: effective_concurrency_u32,
                    auto_concurrency,
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
                        effective_concurrency: effective_concurrency_u32,
                        auto_concurrency,
                    }
                }
            }
        };

        // Use the shared rayon pool to avoid per-call pool construction overhead.
        // Memory backpressure is still handled by WeightedSemaphore.
        //
        // When effective_concurrency < inputs.len(), run exactly that many worker
        // tasks on the global pool and pull work dynamically via an atomic index.
        // This preserves the concurrency cap while avoiding chunk-induced tail latency.
        let results: Vec<BatchResult> = if effective_concurrency >= self.inputs.len() {
            pool::get_pool().install(|| self.inputs.par_iter().map(process_one).collect())
        } else {
            use std::sync::atomic::{AtomicUsize, Ordering};
            use std::sync::{Arc, Mutex};

            let next_index = Arc::new(AtomicUsize::new(0));
            let indexed_results = Arc::new(Mutex::new(Vec::<(usize, BatchResult)>::with_capacity(
                self.inputs.len(),
            )));

            pool::get_pool().install(|| {
                rayon::scope(|scope| {
                    for _ in 0..effective_concurrency {
                        let next_index = Arc::clone(&next_index);
                        let indexed_results = Arc::clone(&indexed_results);
                        let inputs = &self.inputs;
                        scope.spawn(move |_| loop {
                            let idx = next_index.fetch_add(1, Ordering::Relaxed);
                            if idx >= inputs.len() {
                                break;
                            }

                            let result = process_one(&inputs[idx]);
                            let mut guard = indexed_results
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            guard.push((idx, result));
                        });
                    }
                });
            });

            let mut indexed_results = {
                let mut guard = indexed_results
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                std::mem::take(&mut *guard)
            };
            indexed_results.sort_unstable_by_key(|(idx, _)| *idx);
            indexed_results
                .into_iter()
                .map(|(_, result)| result)
                .collect()
        };

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

#[cfg(feature = "napi")]
#[napi]
impl Task for BatchWithMetricsTask {
    type Output = crate::BatchOutputWithMetrics;
    type JsValue = crate::BatchOutputWithMetrics;

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

        if self.concurrency > pool::MAX_CONCURRENCY as u32 {
            let lazy_err = LazyImageError::invalid_argument(
                "concurrency",
                self.concurrency.to_string(),
                format!("must be 0 or 1-{}", pool::MAX_CONCURRENCY),
            );
            self.last_error = Some(lazy_err.clone());
            return Err(napi::Error::from(lazy_err));
        }

        let effective_concurrency = pool::resolve_effective_concurrency(self.concurrency);
        let effective_concurrency_u32 = u32::try_from(effective_concurrency).unwrap_or(u32::MAX);
        let auto_concurrency = self.concurrency == 0;

        let ops = &self.ops;
        let format = &self.format;
        let output_dir = &self.output_dir;
        let keep_icc = self.keep_icc;
        let keep_exif = self.keep_exif;
        let strip_gps = self.strip_gps;
        let firewall = self.firewall.clone();

        let process_one = |input_path: &String| -> crate::BatchResultWithMetrics {
            match process_batch_file(
                input_path,
                output_dir,
                ops,
                format,
                self.auto_orient,
                keep_icc,
                keep_exif,
                strip_gps,
                &firewall,
                true,
            ) {
                Ok(output) => crate::BatchResultWithMetrics {
                    source: input_path.clone(),
                    success: true,
                    error: None,
                    output_path: Some(output.output_path),
                    error_code: None,
                    error_category: None,
                    effective_concurrency: Some(effective_concurrency_u32),
                    auto_concurrency: Some(auto_concurrency),
                    metrics: output.metrics,
                },
                Err(err) => {
                    let error_code = err.code();
                    let error_msg = format!("[{}] {}: {}", error_code.as_str(), input_path, err);
                    let category = error_code.category();
                    crate::BatchResultWithMetrics {
                        source: input_path.clone(),
                        success: false,
                        error: Some(error_msg),
                        output_path: None,
                        error_code: Some(error_code.as_str().to_string()),
                        error_category: Some(category),
                        effective_concurrency: Some(effective_concurrency_u32),
                        auto_concurrency: Some(auto_concurrency),
                        metrics: None,
                    }
                }
            }
        };

        let items: Vec<crate::BatchResultWithMetrics> =
            if effective_concurrency >= self.inputs.len() {
                pool::get_pool().install(|| self.inputs.par_iter().map(process_one).collect())
            } else {
                use std::sync::atomic::{AtomicUsize, Ordering};
                use std::sync::{Arc, Mutex};

                let inputs = Arc::new(&self.inputs);
                let out = Arc::new(Mutex::new(Vec::with_capacity(self.inputs.len())));
                let next = AtomicUsize::new(0);

                pool::get_pool().install(|| {
                    rayon::scope(|s| {
                        for _ in 0..effective_concurrency {
                            let inputs = Arc::clone(&inputs);
                            let out = Arc::clone(&out);
                            let next_ref = &next;
                            s.spawn(move |_| loop {
                                let idx = next_ref.fetch_add(1, Ordering::Relaxed);
                                if idx >= inputs.len() {
                                    break;
                                }
                                let item = process_one(&inputs[idx]);
                                out.lock().unwrap().push((idx, item));
                            });
                        }
                    });
                });

                let mut pairs = Arc::try_unwrap(out).unwrap().into_inner().unwrap();
                pairs.sort_by_key(|(idx, _)| *idx);
                pairs.into_iter().map(|(_, item)| item).collect()
            };

        let mut successful_items = 0u32;
        let mut failed_items = 0u32;
        let mut total_bytes_in = 0f64;
        let mut total_bytes_out = 0f64;
        let mut total_decode_ms = 0f64;
        let mut total_ops_ms = 0f64;
        let mut total_encode_ms = 0f64;
        let mut total_cpu_time = 0f64;
        let mut total_wall_ms = 0f64;

        for item in &items {
            if item.success {
                successful_items += 1;
            } else {
                failed_items += 1;
            }
            if let Some(metrics) = item.metrics.as_ref() {
                total_bytes_in += metrics.bytes_in as f64;
                total_bytes_out += metrics.bytes_out as f64;
                total_decode_ms += metrics.decode_ms;
                total_ops_ms += metrics.ops_ms;
                total_encode_ms += metrics.encode_ms;
                total_cpu_time += metrics.cpu_time;
                total_wall_ms += metrics.total_ms;
            }
        }

        Ok(crate::BatchOutputWithMetrics {
            items,
            summary: crate::BatchMetricsSummary {
                total_items: self.inputs.len() as u32,
                successful_items,
                failed_items,
                effective_concurrency: effective_concurrency_u32,
                auto_concurrency,
                total_bytes_in,
                total_bytes_out,
                total_decode_ms,
                total_ops_ms,
                total_encode_ms,
                total_cpu_time,
                total_wall_ms,
            },
        })
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }

    fn reject(&mut self, env: Env, err: napi::Error) -> Result<Self::JsValue> {
        let lazy_err = self
            .last_error
            .take()
            .unwrap_or_else(|| LazyImageError::generic(err.to_string()));
        let napi_err = crate::error::napi_error_with_code(&env, lazy_err)?;
        Err(napi_err)
    }
}
