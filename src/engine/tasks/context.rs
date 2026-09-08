// src/engine/tasks/context.rs
//
// Shared per-task state and decode entry point used by every task type.
//
// `TaskContext` carries the data common to every single-image encode/write
// task (source, decoded image, ops, format, ICC/EXIF, policy, firewall).
// Each task struct embeds a `TaskContext` instead of repeating these fields.

use super::super::api::MetadataPolicy;
use super::super::firewall::FirewallConfig;
use super::processing::{process_and_encode_from_parts, EncodeContext};
use crate::engine::decoder::{
    check_dimensions, decode_image, detect_format, ensure_dimensions_safe,
};
use crate::engine::io::IccSourceState;
#[allow(unused_imports)]
use crate::engine::io::Source;
#[allow(unused_imports)]
use crate::error::{ErrorCategory, LazyImageError};
use crate::ops::{Operation, OutputFormat};
use image::{DynamicImage, GenericImageView, ImageFormat};
use std::borrow::Cow;
use std::sync::Arc;

/// Convert an `ImageFormat` to the lowercase label used by metrics and
/// downstream APIs.
///
/// Returns a `&'static str` (every arm, including `to_mime_type`, is static),
/// so the `String` allocation is deferred to the single point that actually
/// needs an owned value (metrics population).
pub(super) fn format_to_string(fmt: ImageFormat) -> &'static str {
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
}

/// Detect the input format from a byte slice, returning the lowercase label
/// used by metrics (e.g. "jpeg", "png").
pub(super) fn detect_input_format(bytes: &[u8]) -> Option<&'static str> {
    detect_format(bytes).map(format_to_string)
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

// ============================================================================================
// TaskContext — common fields shared by all single-image encode tasks
// ============================================================================================

/// Common fields shared by all single-image encode/write tasks.
///
/// Consolidates source, decoded image, operations, format, ICC/EXIF data,
/// auto-orient flag, metadata policy, and firewall config into a single struct.
/// Each task struct embeds a `TaskContext` instead of repeating these fields.
pub struct TaskContext {
    pub source: Option<Source>,
    /// Decoded image wrapped in Arc. decode() returns Cow::Borrowed pointing here,
    /// enabling true Copy-on-Write in apply_ops (no deep copy for format-only conversion).
    pub decoded: Option<Arc<DynamicImage>>,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    pub icc_state: IccSourceState,
    /// Raw EXIF data extracted from source image (for preservation)
    pub exif_data: Option<Arc<Vec<u8>>>,
    pub auto_orient: bool,
    /// Single source of truth for metadata preservation decisions
    pub metadata_policy: MetadataPolicy,
    pub firewall: FirewallConfig,
    /// Last error that occurred during compute (for use in reject)
    #[cfg(feature = "napi")]
    pub(crate) last_error: Option<LazyImageError>,
}

/// Decode an image from either a pre-decoded `Arc<DynamicImage>` or a byte source.
///
/// Returns `Cow::Borrowed` when the pre-decoded image is reused (no copy) and
/// `Cow::Owned` when the bytes are decoded fresh. Base safety limits and the
/// firewall are enforced on both paths.
pub(super) fn decode_internal_from_parts<'a>(
    source: Option<&'a Source>,
    decoded: Option<&'a Arc<DynamicImage>>,
    firewall: &FirewallConfig,
) -> std::result::Result<Cow<'a, DynamicImage>, LazyImageError> {
    // Prefer already decoded image (already validated)
    // Return borrowed reference - no deep copy until mutation is needed
    if let Some(img_arc) = decoded {
        // Always-on base safety limits (even without sanitize())
        firewall.enforce_base_limits(img_arc.width(), img_arc.height())?;
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

    // Always-on base metadata size check (even without sanitize())
    firewall.scan_metadata_base(bytes)?;

    firewall.enforce_source_len(bytes.len())?;
    firewall.scan_metadata(bytes)?;

    ensure_dimensions_safe(bytes)?;

    let (img, _detected_format) = decode_image(bytes)?;

    // Security check: reject decompression bombs
    let (w, h) = img.dimensions();
    // Always-on base safety limits (even without sanitize())
    firewall.enforce_base_limits(w, h)?;
    check_dimensions(w, h)?;
    firewall.enforce_pixels(w, h)?;

    Ok(Cow::Owned(img))
}

impl TaskContext {
    /// Build a borrowed [`EncodeContext`] view over this task's fields.
    ///
    /// Every per-task field (source, ops, format, ICC/EXIF, policy, firewall)
    /// is exposed as a borrow; the resulting context can be passed by reference
    /// to [`process_and_encode_from_parts`] and [`encode_prepared`](super::processing::encode_prepared).
    pub(super) fn encode_context(&self) -> EncodeContext<'_> {
        EncodeContext {
            source: self.source.as_ref(),
            decoded: self.decoded.as_ref(),
            ops: &self.ops,
            format: &self.format,
            icc_profile: self.icc_profile.as_ref(),
            icc_state: self.icc_state,
            exif_data: self.exif_data.as_ref(),
            auto_orient: self.auto_orient,
            policy: &self.metadata_policy,
            firewall: &self.firewall,
        }
    }

    /// Process image: decode → apply ops → encode
    /// This is the core processing pipeline shared by toBuffer and toFile.
    /// Returns LazyImageError directly (not wrapped in napi::Error) so that
    /// Task::reject can properly create error objects with code/category.
    pub(crate) fn process_and_encode(
        &self,
        metrics: Option<&mut crate::ProcessingMetrics>,
    ) -> std::result::Result<Vec<u8>, LazyImageError> {
        process_and_encode_from_parts(&self.encode_context(), metrics)
    }

    /// Store an error for later use in reject(), returning a napi::Error for compute().
    #[cfg(feature = "napi")]
    pub(super) fn store_and_convert_error(&mut self, lazy_err: LazyImageError) -> napi::Error {
        self.last_error = Some(lazy_err.clone());
        napi::Error::from(lazy_err)
    }

    /// Retrieve stored error or create a fallback from the napi::Error message.
    #[cfg(feature = "napi")]
    pub(super) fn take_stored_error(&mut self, err: napi::Error) -> LazyImageError {
        self.last_error
            .take()
            .unwrap_or_else(|| LazyImageError::generic(err.to_string()))
    }
}

// Test-only helper for decode assertions in engine.rs tests.
#[cfg(test)]
impl TaskContext {
    pub(crate) fn decode(&self) -> std::result::Result<Cow<'_, DynamicImage>, LazyImageError> {
        decode_internal_from_parts(self.source.as_ref(), self.decoded.as_ref(), &self.firewall)
    }
}
