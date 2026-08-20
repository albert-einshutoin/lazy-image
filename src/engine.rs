// src/engine.rs
//
// The core of lazy-image. A lazy pipeline that:
// 1. Queues operations without executing
// 2. Runs everything in a single pass on compute()
// 3. Uses NAPI AsyncTask to not block Node.js main thread
//
// This file is now a facade that delegates to the decomposed modules in engine/

// =============================================================================
// SECURITY LIMITS
// =============================================================================

/// Maximum allowed image dimension (width or height).
/// Images larger than 32768x32768 are rejected to prevent decompression bombs.
/// This is the same limit used by libvips/sharp.
pub const MAX_DIMENSION: u32 = 32768;

/// Maximum allowed total pixels (width * height).
/// 100 megapixels = 400MB uncompressed RGBA. Beyond this is likely malicious.
pub const MAX_PIXELS: u64 = 100_000_000;

/// When built with `fuzzing` feature, decode paths use these stricter limits so that
/// decode_from_buffer stays under CI's 2GB RSS (ASan + corpus overhead). This tests
/// lazy-image's "bounded memory" property: we reject inputs that would exceed a decode budget.
#[cfg(feature = "fuzzing")]
pub const FUZZ_MAX_DIMENSION: u32 = 1024;
#[cfg(feature = "fuzzing")]
pub const FUZZ_MAX_PIXELS: u64 = 1_000_000; // ~4MB RGBA; keeps fuzz run under 2GB with ASan

// =============================================================================
// MODULE DECOMPOSITION
// =============================================================================

// Import decomposed modules
mod api;
mod common;
mod decoder;
mod encoder;
mod firewall;
mod io;
mod memory;
pub(crate) mod metadata;
mod pipeline;
mod platform;
mod pool;
pub(crate) mod resize;
mod stress;
mod tasks;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod validation;

// -----------------------------------------------------------------------------
// Public API surface
// -----------------------------------------------------------------------------
// These items form the intentional public API of lazy-image as an `rlib`.
// External consumers may rely on these — they are subject to semver.
pub use api::ImageEngine;
pub use encoder::QualitySettings;
pub use firewall::FirewallConfig;
pub use io::Source;

// Crate-internal coordination points shared with header inspection. Re-export
// only the functions needed by the sibling module so the engine's internal
// module tree does not become part of the public API.
#[cfg(any(feature = "napi", feature = "fuzzing", test))]
pub(crate) use common::run_with_panic_policy;
#[cfg(any(feature = "napi", feature = "fuzzing", test))]
pub(crate) use decoder::detect_exif_orientation_bounded_from_reader;

// -----------------------------------------------------------------------------
// Internal items exposed to integration tests and fuzz targets
// -----------------------------------------------------------------------------
// Integration tests in `tests/` and fuzz targets in `fuzz/` compile as
// separate crates and can only see `pub` items. The codec/pipeline helpers
// below are NOT part of the supported public API — they are exposed to enable
// coverage of internal behavior, and may change at any time without a semver
// bump.
//
// `#[doc(hidden)]` keeps them out of rustdoc and signals to downstream users
// that depending on them is unsupported.
#[doc(hidden)]
pub use decoder::{
    check_dimensions, decode_image, decode_jpeg_mozjpeg, decode_with_image_crate,
    detect_exif_orientation, detect_format, ensure_dimensions_safe,
};
#[doc(hidden)]
pub use encoder::{encode_avif, encode_jpeg, encode_png, encode_webp};
#[doc(hidden)]
pub use io::{extract_exif_raw, extract_icc_profile};
#[doc(hidden)]
pub use pipeline::{apply_ops, optimize_ops};
#[doc(hidden)]
pub use resize::{calc_resize_dimensions, fast_resize, fast_resize_owned};

// Note: the remaining helpers used elsewhere inside the crate (e.g.
// `embed_icc_*`, `extract_icc_profile_lossy`, `ResizeError`, pool helpers)
// intentionally have no re-export here. They are reached via module-relative
// paths from sibling modules within `engine`, and the surrounding `mod`
// declarations are private so the items cannot leak through the rlib's
// public API.

// Re-export types from api.rs and tasks.rs
#[cfg(feature = "napi")]
pub use api::{Dimensions, PresetResult};
#[cfg(feature = "napi")]
pub use tasks::BatchResult;

// Re-export stress test function
#[cfg(feature = "stress")]
pub use stress::run_stress_iteration;
// =============================================================================
// UTILITY FUNCTIONS
// =============================================================================

// Removed duplicate functions - they are now in decomposed modules:
// - calc_resize_dimensions -> engine/pipeline.rs
// - check_dimensions -> engine/decoder.rs
// - extract_icc_profile and related functions -> engine/io.rs

// Removed duplicate fast_resize functions - they are now in engine/pipeline.rs

// Tests that exercise the decode path through `EncodeTask`/`TaskContext` plus
// `MetadataPolicy`/`FirewallConfig` remain inline because they depend on
// `pub(crate)` internals only reachable from within this crate.  ICC extraction
// tests live next to the code under `src/engine/io/icc_tests.rs`; all other
// engine tests have been extracted to `tests/engine_tests.rs`.

#[cfg(test)]
mod tests {
    use crate::engine::api::MetadataPolicy;
    use crate::engine::firewall::FirewallConfig;
    use crate::engine::io::IccSourceState;
    use crate::engine::tasks::{EncodeTask, TaskContext};
    use crate::engine::test_support::create_test_image;
    use crate::error::LazyImageError;
    use crate::ops::OutputFormat;
    use image::GenericImageView;
    use std::sync::Arc;

    fn create_png(width: u32, height: u32) -> Vec<u8> {
        let img = create_test_image(width, height);
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    fn create_minimal_png() -> Vec<u8> {
        create_png(1, 1)
    }

    mod decode_tests {
        use super::*;

        #[test]
        fn test_decode_with_image_crate() {
            let png_data = create_minimal_png();
            use crate::engine::io::Source;
            let task = EncodeTask {
                ctx: TaskContext {
                    source: Some(Source::from_vec(png_data)),
                    decoded: None,
                    ops: vec![],
                    format: OutputFormat::Png,
                    icc_profile: None,
                    icc_state: IccSourceState::Absent,
                    exif_data: None,
                    auto_orient: true,
                    metadata_policy: MetadataPolicy::default_policy(),
                    firewall: FirewallConfig::disabled(),
                    #[cfg(feature = "napi")]
                    last_error: None,
                },
            };
            let result = task.decode();
            assert!(result.is_ok());
            let img = result.unwrap();
            assert!(img.dimensions().0 > 0);
            assert!(img.dimensions().1 > 0);
        }

        #[test]
        fn test_decode_already_decoded() {
            let img = create_test_image(100, 100);
            let task = EncodeTask {
                ctx: TaskContext {
                    source: None,
                    decoded: Some(Arc::new(img.clone())),
                    ops: vec![],
                    format: OutputFormat::Png,
                    icc_profile: None,
                    icc_state: IccSourceState::Absent,
                    exif_data: None,
                    auto_orient: true,
                    metadata_policy: MetadataPolicy::default_policy(),
                    firewall: FirewallConfig::disabled(),
                    #[cfg(feature = "napi")]
                    last_error: None,
                },
            };
            let result = task.decode();
            assert!(result.is_ok());
            let decoded_img = result.unwrap();
            assert_eq!(decoded_img.dimensions(), img.dimensions());
        }

        #[test]
        fn test_decode_no_source() {
            let task = EncodeTask {
                ctx: TaskContext {
                    source: None,
                    decoded: None,
                    ops: vec![],
                    format: OutputFormat::Png,
                    icc_profile: None,
                    icc_state: IccSourceState::Absent,
                    exif_data: None,
                    auto_orient: true,
                    metadata_policy: MetadataPolicy::default_policy(),
                    firewall: FirewallConfig::disabled(),
                    #[cfg(feature = "napi")]
                    last_error: None,
                },
            };
            let result = task.decode();
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Image source already consumed"));
        }

        #[test]
        fn test_firewall_blocks_large_input_bytes() {
            use crate::engine::io::Source;
            let png_data = create_minimal_png();
            let mut firewall = FirewallConfig::custom();
            firewall.max_bytes = Some(1);

            let task = EncodeTask {
                ctx: TaskContext {
                    source: Some(Source::from_vec(png_data)),
                    decoded: None,
                    ops: vec![],
                    format: OutputFormat::Png,
                    icc_profile: None,
                    icc_state: IccSourceState::Absent,
                    exif_data: None,
                    auto_orient: true,
                    metadata_policy: MetadataPolicy::default_policy(),
                    firewall,
                    #[cfg(feature = "napi")]
                    last_error: None,
                },
            };
            let err = task.decode().unwrap_err();
            assert!(matches!(err, LazyImageError::FirewallViolation { .. }));
        }

        #[test]
        fn test_firewall_blocks_large_pixel_count() {
            use crate::engine::io::Source;
            let png_data = create_png(2, 2);
            let mut firewall = FirewallConfig::custom();
            firewall.max_pixels = Some(1);

            let task = EncodeTask {
                ctx: TaskContext {
                    source: Some(Source::from_vec(png_data)),
                    decoded: None,
                    ops: vec![],
                    format: OutputFormat::Png,
                    icc_profile: None,
                    icc_state: IccSourceState::Absent,
                    exif_data: None,
                    auto_orient: true,
                    metadata_policy: MetadataPolicy::default_policy(),
                    firewall,
                    #[cfg(feature = "napi")]
                    last_error: None,
                },
            };
            let err = task.decode().unwrap_err();
            assert!(matches!(err, LazyImageError::FirewallViolation { .. }));
        }
    }
}
