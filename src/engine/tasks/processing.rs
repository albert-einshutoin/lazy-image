// src/engine/tasks/processing.rs
//
// The two-phase encode pipeline:
//   `prepare_for_encode` (decode + ops) → `encode_prepared` (encode + EXIF embed)
//
// The byte-budget binary search reuses one prepared image across many encode
// passes; `process_and_encode_from_parts` is the single-shot convenience that
// chains both phases.

use super::super::api::MetadataPolicy;
use super::super::firewall::FirewallConfig;
use super::super::platform;
use super::context::{decode_internal_from_parts, detect_input_format};
use crate::engine::encoder::{
    embed_exif_jpeg, embed_exif_png, embed_exif_webp, encode_avif, encode_jpeg_with_settings,
    encode_png, encode_webp,
};
#[allow(unused_imports)]
use crate::engine::io::{exif_has_gps, has_exif, Source};
use crate::engine::memory;
use crate::engine::pipeline::{apply_ops_tracked, ColorState, IccState};
use crate::error::LazyImageError;
use crate::ops::{Operation, OutputFormat};
use crate::PROCESSING_METRICS_VERSION;
use image::{DynamicImage, GenericImageView};
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;

type ResourceUsage = platform::ResourceUsage;

fn get_resource_usage() -> Option<ResourceUsage> {
    platform::get_resource_usage()
}

/// Borrowed parameter bundle for the process + encode pipeline.
///
/// Replaces a 10-argument signature on `process_and_encode_from_parts` and an
/// 8-argument signature on `encode_prepared` with a single struct. Every field
/// is `Copy`, so the struct itself is `Copy` — that lets callers reuse a base
/// context and vary only one field with struct-update syntax (used by the
/// byte-budget binary search, which keeps every field constant except the
/// per-iteration `OutputFormat`).
///
/// `encode_prepared` ignores the prepare-only fields (`source`, `decoded`,
/// `ops`, `icc_present`); they live here rather than in a separate struct so
/// every callsite builds exactly one context object.
#[derive(Clone, Copy)]
pub(super) struct EncodeContext<'a> {
    pub(super) source: Option<&'a Source>,
    pub(super) decoded: Option<&'a Arc<DynamicImage>>,
    pub(super) ops: &'a [Operation],
    pub(super) format: &'a OutputFormat,
    pub(super) icc_profile: Option<&'a Arc<Vec<u8>>>,
    pub(super) icc_present: bool,
    pub(super) exif_data: Option<&'a Arc<Vec<u8>>>,
    pub(super) auto_orient: bool,
    pub(super) policy: &'a MetadataPolicy,
    pub(super) firewall: &'a FirewallConfig,
}

/// Cached state from decode + ops, reusable across multiple encode passes.
///
/// Used by [`super::encode::EncodeTargetBytesTask`] to avoid re-decoding and
/// re-applying ops for every quality candidate in the binary search. The memory
/// permit is held across all encode iterations so we do not repeatedly
/// acquire/release.
pub(super) struct PreparedEncode {
    /// Image after decode + ops applied. Stored as `Arc` so the cheap path
    /// (no ops, pre-decoded input) avoids a deep image clone.
    pub(super) processed: Arc<DynamicImage>,
    pub(super) final_color_state: ColorState,
    pub(super) input_format: Option<String>,
    /// Whether the source originally had an EXIF segment. Detected once via
    /// `has_exif` on the input bytes (not gated by `MAX_EXIF_SOURCE_BYTES`).
    pub(super) exif_present_in_source: bool,
    pub(super) decode_ms: f64,
    pub(super) ops_ms: f64,
    pub(super) usage_start: Option<ResourceUsage>,
    pub(super) input_size: u64,
    pub(super) start_total: Instant,
    /// Hold the memory permit until the prepared image is dropped so the
    /// reservation stays valid across multiple `encode_prepared` calls.
    pub(super) _permit_guard: memory::MemoryPermit,
}

/// Phase 1 of the pipeline: decode the input and apply all ops once.
///
/// Returns a [`PreparedEncode`] that can be fed to [`encode_prepared`] one or
/// more times — useful for the byte-budget binary search where only the encode
/// quality varies between iterations.
pub(super) fn prepare_for_encode(
    source: Option<&Source>,
    decoded: Option<&Arc<DynamicImage>>,
    ops: &[Operation],
    icc_present: bool,
    auto_orient: bool,
    firewall: &FirewallConfig,
    format_for_memory_estimate: &OutputFormat,
) -> std::result::Result<PreparedEncode, LazyImageError> {
    let input_size = source.map(|s| s.len() as u64).unwrap_or(0);
    let input_bytes = source.and_then(|s| s.as_bytes());
    let input_format = input_bytes.and_then(detect_input_format);

    // Memory backpressure: estimate before decode and acquire weighted permit.
    // Use file-size-based fallback instead of hard-coded 100MB when header parsing fails.
    let estimated_memory = source
        .and_then(|s| s.as_bytes())
        .and_then(|bytes| {
            memory::estimate_memory_from_header(bytes, ops, Some(format_for_memory_estimate))
        })
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

    let start_total = Instant::now();
    let usage_start = get_resource_usage();
    let mut stage_start = start_total;

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

    // EXIF presence detection: `has_exif` is not gated by `MAX_EXIF_SOURCE_BYTES`,
    // so JPEGs larger than 8 MiB are still detected. Done here so the encode
    // phase does not need to re-scan input bytes on every iteration.
    let exif_present_in_source = input_bytes.map(has_exif).unwrap_or(false);

    // 1. Decode
    let img = decode_internal_from_parts(source, decoded, firewall)?;
    firewall.enforce_timeout(start_total, "decode")?;
    let decode_ms = stage_start.elapsed().as_secs_f64() * 1000.0;
    stage_start = Instant::now();

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

    // Materialize the processed image into an Arc:
    // - Cow::Owned (ops applied OR decoded fresh from bytes) → wrap newly allocated buffer.
    // - Cow::Borrowed (only possible when `decoded.is_some()` AND ops is effectively empty)
    //   → clone the original Arc cheaply (atomic increment, no pixel copy).
    let processed: Arc<DynamicImage> = match tracked.image {
        Cow::Owned(img) => Arc::new(img),
        Cow::Borrowed(_) => Arc::clone(
            decoded
                .expect("Cow::Borrowed implies decoded.is_some(); see decode_internal_from_parts"),
        ),
    };
    firewall.enforce_timeout(start_total, "process")?;
    let ops_ms = stage_start.elapsed().as_secs_f64() * 1000.0;

    Ok(PreparedEncode {
        processed,
        final_color_state,
        input_format,
        exif_present_in_source,
        decode_ms,
        ops_ms,
        usage_start,
        input_size,
        start_total,
        _permit_guard: permit,
    })
}

/// Phase 2 of the pipeline: encode the prepared image to the given format.
///
/// May be called multiple times against the same [`PreparedEncode`] — the
/// byte-budget binary search uses this to vary only quality. Per-call timing
/// (encode_ms, total_ms) is recorded into `metrics` if provided; decode_ms /
/// ops_ms come from the cached prepare phase.
pub(super) fn encode_prepared(
    prepared: &PreparedEncode,
    ctx: &EncodeContext<'_>,
    metrics: Option<&mut crate::ProcessingMetrics>,
) -> std::result::Result<Vec<u8>, LazyImageError> {
    let EncodeContext {
        format,
        icc_profile,
        exif_data,
        auto_orient,
        policy,
        firewall,
        ..
    } = *ctx;
    let keep_icc = policy.effective_icc();
    let keep_exif = policy.effective_exif();
    let strip_gps = policy.strip_gps();

    let encode_start = Instant::now();

    // 3. Encode - only preserve ICC profile if keep_icc is true
    let icc = if keep_icc {
        icc_profile.map(|v| v.as_slice())
    } else {
        None // Strip metadata by default for security & smaller files
    };

    let processed: &DynamicImage = prepared.processed.as_ref();

    // 4. Encode image to target format
    let mut result = match format {
        OutputFormat::Jpeg { quality, fast_mode } => {
            encode_jpeg_with_settings(processed, *quality, icc, *fast_mode)
        }
        OutputFormat::Png => encode_png(processed, icc),
        OutputFormat::WebP { quality } => encode_webp(processed, *quality, icc),
        OutputFormat::Avif { quality } => encode_avif(processed, *quality, icc),
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
    firewall.enforce_timeout(prepared.start_total, "encode")?;

    let encode_ms = encode_start.elapsed().as_secs_f64() * 1000.0;

    if let Some(m) = metrics {
        // Get final resource usage & finalize metrics
        let final_usage = get_resource_usage();
        // Use tracked color state to reason about ICC preservation.
        let icc_present = matches!(prepared.final_color_state.icc, IccState::Present);
        let icc_preserved = keep_icc && icc_present;
        // EXIF presence detection: prefer the in-memory copy that the API layer may
        // have already extracted, otherwise fall back to a lightweight presence scan
        // recorded during the prepare phase.
        let exif_present = exif_data.is_some() || prepared.exif_present_in_source;
        let exif_preserved =
            keep_exif && exif_present && !matches!(format, OutputFormat::Avif { .. });
        // GPS is only "stripped" if it was actually present in the source. When
        // `keep_exif` is true, the API layer extracted EXIF eagerly, so we can
        // parse it directly. When EXIF is being dropped by default policy,
        // `exif_preserved` is false and `gps_stripped_from_exif` does not affect
        // the final flag.
        let gps_present_in_exif = exif_data
            .map(|exif| exif_has_gps(exif.as_slice()))
            .unwrap_or(false);
        let gps_stripped_from_exif = exif_preserved && strip_gps && gps_present_in_exif;
        let metadata_stripped = (icc_present && !icc_preserved)
            || (exif_present && !exif_preserved)
            || gps_stripped_from_exif;
        let metadata_blocked_by_policy =
            (keep_icc || keep_exif) && firewall.reject_metadata && (icc_present || exif_present);
        let mut policy_violations = Vec::new();
        if metadata_blocked_by_policy {
            policy_violations.push("firewall_rejected_metadata".to_string());
        }

        m.decode_ms = prepared.decode_ms;
        m.ops_ms = prepared.ops_ms;
        m.encode_ms = encode_ms;
        m.total_ms = prepared.start_total.elapsed().as_secs_f64() * 1000.0;
        m.processing_time = m.total_ms / 1000.0;
        m.version = PROCESSING_METRICS_VERSION.to_string();

        if let (Some(start), Some(end)) = (prepared.usage_start.as_ref(), final_usage.as_ref()) {
            m.cpu_time = (end.cpu_time - start.cpu_time).max(0.0);
            m.peak_rss = end.memory_rss.min(u32::MAX as u64) as u32;
        } else {
            let (w, h) = processed.dimensions();
            m.peak_rss =
                ((w as u64 * h as u64 * 4) + result.len() as u64).min(u32::MAX as u64) as u32;
        }

        m.bytes_in = prepared.input_size.min(u32::MAX as u64) as u32;
        m.bytes_out = (result.len() as u64).min(u32::MAX as u64) as u32;
        m.compression_ratio = if m.bytes_in > 0 {
            m.bytes_out as f64 / m.bytes_in as f64
        } else {
            0.0
        };

        m.format_in = prepared.input_format.clone();
        m.format_out = format.as_str().to_string();
        m.icc_preserved = icc_preserved;
        m.metadata_stripped = metadata_stripped;
        m.policy_violations = policy_violations;
    }

    Ok(result)
}

/// Single-shot helper that chains [`prepare_for_encode`] and [`encode_prepared`].
///
/// Most tasks call this exactly once. The byte-budget binary search bypasses
/// this and runs `prepare_for_encode` separately so it can reuse the prepared
/// image across encode iterations.
pub(super) fn process_and_encode_from_parts(
    ctx: &EncodeContext<'_>,
    metrics: Option<&mut crate::ProcessingMetrics>,
) -> std::result::Result<Vec<u8>, LazyImageError> {
    let prepared = prepare_for_encode(
        ctx.source,
        ctx.decoded,
        ctx.ops,
        ctx.icc_present,
        ctx.auto_orient,
        ctx.firewall,
        ctx.format,
    )?;
    encode_prepared(&prepared, ctx, metrics)
}
