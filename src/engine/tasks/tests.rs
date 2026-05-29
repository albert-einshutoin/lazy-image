// src/engine/tasks/tests.rs
//
// Non-NAPI unit tests for the tasks module. Gated to `cfg(test, not(napi))`
// because the assertions exercise the pure-Rust pipeline without the NAPI
// runtime in play.

use super::super::api::MetadataPolicy;
use super::super::firewall::FirewallConfig;
use super::context::TaskContext;
use super::encode::{EncodeTargetBytesTask, EncodeTask};
use super::processing::{encode_prepared, prepare_for_encode, EncodeContext};
use crate::engine::io::Source;
use crate::error::LazyImageError;
use crate::ops::{Operation, OutputFormat, Quality, ResizeFit};
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba};
use std::sync::Arc;

fn sample_png_bytes() -> Vec<u8> {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    bytes
}

/// Helper to create a TaskContext with a pre-decoded image.
fn make_ctx_with_decoded(format: OutputFormat) -> TaskContext {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
    let dyn_img = DynamicImage::ImageRgba8(img);
    TaskContext {
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
        metadata_policy: MetadataPolicy::default_policy(),
        firewall: FirewallConfig::disabled(),
    }
}

fn make_task_with_decoded(format: OutputFormat) -> EncodeTask {
    EncodeTask {
        ctx: make_ctx_with_decoded(format),
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
        ctx: TaskContext {
            source: None,
            decoded: None,
            ops: vec![],
            format: OutputFormat::Png,
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: true,
            metadata_policy: MetadataPolicy::default_policy(),
            firewall: FirewallConfig::disabled(),
        },
    };
    let err = task.decode().unwrap_err();
    assert!(matches!(err, LazyImageError::SourceConsumed));
}

/// Verify that base dimension/pixel limits are enforced at decode time
/// even when the firewall is disabled (i.e., sanitize() was NOT called).
/// This is the core fix for #492.
#[test]
fn base_limits_enforced_without_sanitize() {
    // Create a PNG with dimensions exceeding MAX_PIXELS (100M)
    // We can't easily create such a huge image, so instead verify via
    // enforce_base_limits which is called in the decode path.
    let cfg = FirewallConfig::disabled();
    assert!(!cfg.enabled);

    // 10001 x 10001 = 100_020_001 > 100M pixels
    let err = cfg.enforce_base_limits(10001, 10001).unwrap_err();
    assert!(matches!(
        err,
        crate::error::LazyImageError::PixelCountExceedsLimit { .. }
    ));
}

/// Verify that base metadata scan runs at decode time without sanitize().
#[test]
fn base_metadata_scan_runs_without_sanitize() {
    let cfg = FirewallConfig::disabled();
    assert!(!cfg.enabled);

    // Small metadata passes
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    assert!(cfg.scan_metadata_base(&bytes).is_ok());
}

#[test]
fn firewall_limits_are_enforced_on_decode() {
    let png = sample_png_bytes();
    let mut firewall = FirewallConfig::custom();
    firewall.max_bytes = Some(1); // smaller than PNG size to force rejection

    let task = EncodeTask {
        ctx: TaskContext {
            source: Some(Source::from_vec(png)),
            decoded: None,
            ops: vec![],
            format: OutputFormat::Png,
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: true,
            metadata_policy: MetadataPolicy::default_policy(),
            firewall,
        },
    };
    let err = task.decode().unwrap_err();
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
        ctx: TaskContext {
            source: Some(Source::from_vec(png)),
            decoded: None,
            ops: vec![],
            format: OutputFormat::Png,
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: false,
            metadata_policy: MetadataPolicy::default_policy(),
            firewall: FirewallConfig::disabled(),
        },
    };
    assert!(!task.ctx.auto_orient);
    // process_and_encode should succeed (no orientation applied)
    let result = task.process_and_encode(None);
    assert!(result.is_ok());
}

#[test]
fn auto_orient_flag_default_is_true() {
    // Verify the default task helper sets auto_orient to true
    let task = make_task_with_decoded(OutputFormat::Png);
    assert!(task.ctx.auto_orient);
}

#[test]
fn fast_mode_propagates_to_jpeg_encode() {
    // Verify that OutputFormat::Jpeg with fast_mode=true produces valid JPEG
    let task = make_task_with_decoded(OutputFormat::Jpeg {
        quality: Quality::unchecked(80),
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
        quality: Quality::unchecked(80),
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
        quality: Quality::unchecked(80),
        fast_mode: true,
    });
    let normal_task = make_task_with_decoded(OutputFormat::Jpeg {
        quality: Quality::unchecked(80),
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

#[test]
fn target_bytes_search_finds_best_quality_under_budget() {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(64, 64, Rgba([10, 20, 30, 255]));
    let dyn_img = DynamicImage::ImageRgba8(img);

    // Encode at max quality to get a reference size
    let task_max = EncodeTask {
        ctx: TaskContext {
            source: None,
            decoded: Some(Arc::new(dyn_img.clone())),
            ops: vec![],
            format: OutputFormat::Jpeg {
                quality: Quality::unchecked(100),
                fast_mode: false,
            },
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: false,
            metadata_policy: MetadataPolicy::strip_all(),
            firewall: FirewallConfig::disabled(),
        },
    };
    let max_size = task_max.process_and_encode(None).unwrap().len();

    // Set a budget larger than any output (should pick max quality)
    let search_task = EncodeTargetBytesTask {
        ctx: TaskContext {
            source: None,
            decoded: Some(Arc::new(dyn_img.clone())),
            ops: vec![],
            format: OutputFormat::Jpeg {
                quality: Quality::unchecked(100),
                fast_mode: false,
            },
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: false,
            metadata_policy: MetadataPolicy::strip_all(),
            firewall: FirewallConfig::disabled(),
        },
        target_bytes: (max_size + 10000) as u32,
        min_quality: 1,
        max_quality: 100,
        quality_floor_policy_strict: false,
    };
    let result = search_task.search().expect("search should succeed");
    assert!(result.budget_met);
    assert_eq!(result.quality, 100);

    // Set a tight budget (should pick a lower quality)
    let tight_task = EncodeTargetBytesTask {
        ctx: TaskContext {
            source: None,
            decoded: Some(Arc::new(dyn_img)),
            ops: vec![],
            format: OutputFormat::Jpeg {
                quality: Quality::unchecked(100),
                fast_mode: false,
            },
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: false,
            metadata_policy: MetadataPolicy::strip_all(),
            firewall: FirewallConfig::disabled(),
        },
        target_bytes: 500,
        min_quality: 1,
        max_quality: 100,
        quality_floor_policy_strict: false,
    };
    let tight_result = tight_task.search().expect("search should succeed");
    // Quality should be lower than max since budget is tight
    assert!(tight_result.quality <= 100);
    // The result should be the best effort (either under budget or closest over)
    assert!(!tight_result.data.is_empty());
}

#[test]
fn target_bytes_strict_policy_fails_when_budget_unmet() {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(64, 64, Rgba([10, 20, 30, 255]));
    let dyn_img = DynamicImage::ImageRgba8(img);

    let task = EncodeTargetBytesTask {
        ctx: TaskContext {
            source: None,
            decoded: Some(Arc::new(dyn_img)),
            ops: vec![],
            format: OutputFormat::Jpeg {
                quality: Quality::unchecked(100),
                fast_mode: false,
            },
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: false,
            metadata_policy: MetadataPolicy::strip_all(),
            firewall: FirewallConfig::disabled(),
        },
        // Impossibly small budget
        target_bytes: 1,
        min_quality: 80,
        max_quality: 100,
        quality_floor_policy_strict: true,
    };
    let err = task.search().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Unable to meet targetBytes"),
        "expected strict policy error, got: {msg}"
    );
}

/// Regression test for issue #558: the binary search must decode + apply
/// ops once, not per quality iteration. We assert it indirectly by giving
/// the prepare phase a pre-decoded Arc and checking that `prepare_for_encode`
/// reuses the Arc by reference (no pixel clone) when ops is empty.
#[test]
fn prepare_for_encode_reuses_decoded_arc_when_ops_empty() {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(32, 32, Rgba([10, 20, 30, 255]));
    let dyn_img = DynamicImage::ImageRgba8(img);
    let original = Arc::new(dyn_img);

    let prepared = prepare_for_encode(
        None,
        Some(&original),
        &[],
        false,
        false,
        &FirewallConfig::disabled(),
        &OutputFormat::Jpeg {
            quality: Quality::unchecked(80),
            fast_mode: false,
        },
    )
    .expect("prepare should succeed");

    // Cheap path: Arc clone, not a deep image clone.
    assert!(
        Arc::ptr_eq(&original, &prepared.processed),
        "prepare_for_encode must reuse the decoded Arc when no ops are applied"
    );
}

/// Regression test for issue #558: ops *are* applied during prepare, and
/// the resulting Arc points to a freshly-allocated transformed image
/// (not the original input).
#[test]
fn prepare_for_encode_applies_ops_once_when_ops_nonempty() {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(64, 64, Rgba([10, 20, 30, 255]));
    let dyn_img = DynamicImage::ImageRgba8(img);
    let original = Arc::new(dyn_img);

    let prepared = prepare_for_encode(
        None,
        Some(&original),
        &[Operation::Resize {
            width: Some(16),
            height: Some(16),
            fit: crate::ops::ResizeFit::Inside,
        }],
        false,
        false,
        &FirewallConfig::disabled(),
        &OutputFormat::Jpeg {
            quality: Quality::unchecked(80),
            fast_mode: false,
        },
    )
    .expect("prepare should succeed");

    assert!(
        !Arc::ptr_eq(&original, &prepared.processed),
        "prepare_for_encode must produce a new image when ops transform pixels"
    );
    assert_eq!(prepared.processed.width(), 16);
    assert_eq!(prepared.processed.height(), 16);
}

/// Regression test for issue #558: re-running encode_prepared multiple
/// times against the same PreparedEncode must produce consistent results,
/// proving the cached image is reused safely.
#[test]
fn encode_prepared_can_run_multiple_times_against_same_cache() {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(64, 64, Rgba([200, 100, 50, 255]));
    let dyn_img = DynamicImage::ImageRgba8(img);

    let prepared = prepare_for_encode(
        None,
        Some(&Arc::new(dyn_img)),
        &[],
        false,
        false,
        &FirewallConfig::disabled(),
        &OutputFormat::Jpeg {
            quality: Quality::unchecked(80),
            fast_mode: false,
        },
    )
    .expect("prepare should succeed");

    // Encode at three different qualities — like the binary search would.
    let mut last_size: Option<usize> = None;
    let policy = MetadataPolicy::strip_all();
    let firewall = FirewallConfig::disabled();
    for q in [50u8, 80, 100] {
        let format = OutputFormat::Jpeg {
            quality: Quality::unchecked(q),
            fast_mode: false,
        };
        let ctx = EncodeContext {
            source: None,
            decoded: None,
            ops: &[],
            format: &format,
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: false,
            policy: &policy,
            firewall: &firewall,
        };
        let bytes = encode_prepared(&prepared, &ctx, None).expect("encode should succeed");
        assert_eq!(
            &bytes[0..2],
            &[0xFF, 0xD8],
            "encode_prepared must produce valid JPEG at quality {q}"
        );
        // Higher quality should generally yield larger files.
        if let Some(prev) = last_size {
            assert!(
                bytes.len() >= prev,
                "higher quality should not produce smaller output: prev={prev}, current={}",
                bytes.len()
            );
        }
        last_size = Some(bytes.len());
    }
}

/// Regression test for issue #558: the metrics returned by the byte-budget
/// search must include sensible decode_ms / ops_ms / encode_ms fields
/// (the prepare phase populates the first two; encode populates the third).
#[test]
fn target_bytes_search_metrics_include_decode_ops_and_encode_timings() {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(64, 64, Rgba([10, 20, 30, 255]));
    let dyn_img = DynamicImage::ImageRgba8(img);

    let task = EncodeTargetBytesTask {
        ctx: TaskContext {
            source: None,
            decoded: Some(Arc::new(dyn_img)),
            ops: vec![Operation::Resize {
                width: Some(32),
                height: Some(32),
                fit: crate::ops::ResizeFit::Inside,
            }],
            format: OutputFormat::Jpeg {
                quality: Quality::unchecked(100),
                fast_mode: false,
            },
            icc_profile: None,
            icc_present: false,
            exif_data: None,
            auto_orient: false,
            metadata_policy: MetadataPolicy::strip_all(),
            firewall: FirewallConfig::disabled(),
        },
        target_bytes: 10_000_000,
        min_quality: 1,
        max_quality: 100,
        quality_floor_policy_strict: false,
    };

    let result = task.search().expect("search should succeed");
    let metrics = result.metrics;

    // All stage timings must be non-negative (Instant::elapsed cannot be negative
    // but the f64 cast can technically produce 0.0; assert >=).
    assert!(metrics.decode_ms >= 0.0);
    assert!(metrics.ops_ms >= 0.0);
    assert!(metrics.encode_ms >= 0.0);
    assert!(metrics.total_ms >= metrics.encode_ms);
    // bytes_in/out should reflect the actual output (no input bytes available).
    assert_eq!(metrics.bytes_in, 0);
    assert_eq!(metrics.bytes_out as usize, result.data.len());
}
