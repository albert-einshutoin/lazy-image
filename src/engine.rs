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
pub(crate) mod validation;

// Re-export commonly used types and functions
pub use api::ImageEngine;
pub use decoder::{
    check_dimensions, decode_image, decode_jpeg_mozjpeg, decode_with_image_crate,
    detect_exif_orientation, detect_format, ensure_dimensions_safe,
};
pub use encoder::{
    embed_icc_jpeg, embed_icc_png, embed_icc_webp, encode_avif, encode_jpeg, encode_png,
    encode_webp, QualitySettings,
};
pub use firewall::FirewallConfig;
pub use io::{extract_exif_raw, extract_icc_profile, extract_icc_profile_lossy, Source};
pub use pipeline::{apply_ops, optimize_ops};
pub use resize::{
    calc_resize_dimensions, fast_resize, fast_resize_internal, fast_resize_owned, ResizeError,
};

// Re-export pool constants for tasks.rs
#[cfg(feature = "napi")]
pub use pool::{get_pool, MAX_CONCURRENCY};

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

// Tests that depend on pub(crate) internals (ICC helpers, EncodeTask,
// MetadataPolicy, FirewallConfig) remain inline.  All other engine tests
// have been extracted to tests/engine_tests.rs.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::MetadataPolicy;
    use crate::engine::firewall::FirewallConfig;
    use crate::engine::tasks::{EncodeTask, TaskContext};
    use crate::error::LazyImageError;
    use crate::ops::OutputFormat;
    use image::{DynamicImage, GenericImageView, RgbImage};
    use std::sync::Arc;

    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        }))
    }

    fn extract_icc_ok(data: &[u8]) -> Option<Vec<u8>> {
        extract_icc_profile(data).unwrap()
    }

    fn create_minimal_jpeg() -> Vec<u8> {
        let img = create_test_image(1, 1);
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let pixels = rgb.into_raw();

        let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
        comp.set_size(w as usize, h as usize);
        comp.set_quality(80.0);
        comp.set_color_space(mozjpeg::ColorSpace::JCS_YCbCr);
        comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));

        let mut output = Vec::new();
        {
            let mut writer = comp.start_compress(&mut output).unwrap();
            let stride = w as usize * 3;
            for row in pixels.chunks(stride) {
                writer.write_scanlines(row).unwrap();
            }
            writer.finish().unwrap();
        }
        output
    }

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

    fn create_minimal_webp() -> Vec<u8> {
        let img = create_test_image(10, 10);
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let encoder = webp::Encoder::from_rgb(&rgb, w, h);
        let config = webp::WebPConfig::new().unwrap();
        let mem = encoder.encode_advanced(&config).unwrap();
        mem.to_vec()
    }

    mod icc_tests {
        use super::*;
        use crate::engine::io::{
            extract_icc_from_png_direct, validate_icc_profile, IccExtractor, JpegIccExtractor,
            PngIccExtractor, WebpIccExtractor,
        };

        #[test]
        fn test_validate_icc_profile_too_small() {
            let data = vec![0u8; 127];
            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_minimal_valid() {
            let mut data = vec![0u8; 128];
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80;
            data[4] = b'A';
            data[5] = b'D';
            data[6] = b'B';
            data[7] = b'E';
            data[8] = 2;
            data[12] = b'm';
            data[13] = b'n';
            data[14] = b't';
            data[15] = b'r';
            data[16] = b'R';
            data[17] = b'G';
            data[18] = b'B';
            data[19] = b' ';
            data[20] = b'X';
            data[21] = b'Y';
            data[22] = b'Z';
            data[23] = b' ';

            assert!(validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_size_mismatch() {
            let mut data = vec![0u8; 200];
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0xC8;
            data[3] = 0x00;
            data[3] = 0xFF;

            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_invalid_version() {
            let mut data = vec![0u8; 128];
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80;
            data[8] = 20;

            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_extract_icc_from_jpeg_no_profile() {
            let jpeg_data = create_minimal_jpeg();
            let result = JpegIccExtractor.extract_icc(&jpeg_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_from_png_no_profile() {
            let png_data = create_png(2, 2);
            let result = PngIccExtractor.extract_icc(&png_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_from_webp_no_profile() {
            let webp_data = create_minimal_webp();
            let result = WebpIccExtractor.extract_icc(&webp_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_profile_invalid_data() {
            let invalid_data = vec![0u8; 10];
            let result = extract_icc_profile(&invalid_data);
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
        }

        #[test]
        fn test_extract_icc_profile_jpeg() {
            let jpeg_data = create_minimal_jpeg();
            let result = extract_icc_profile(&jpeg_data);
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
        }

        fn create_minimal_srgb_icc() -> Vec<u8> {
            let mut data = vec![0u8; 128];
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80;
            data[4] = b'A';
            data[5] = b'D';
            data[6] = b'B';
            data[7] = b'E';
            data[8] = 2;
            data[12] = b'm';
            data[13] = b'n';
            data[14] = b't';
            data[15] = b'r';
            data[16] = b'R';
            data[17] = b'G';
            data[18] = b'B';
            data[19] = b' ';
            data[20] = b'X';
            data[21] = b'Y';
            data[22] = b'Z';
            data[23] = b' ';
            data
        }

        fn create_jpeg_with_icc(icc: &[u8]) -> Vec<u8> {
            let img = create_test_image(100, 100);
            encode_jpeg(&img, 80, Some(icc)).unwrap()
        }

        fn create_png_with_icc(icc: &[u8]) -> Vec<u8> {
            let img = create_test_image(100, 100);
            encode_png(&img, Some(icc)).unwrap()
        }

        fn create_webp_with_icc(icc: &[u8]) -> Vec<u8> {
            let img = create_test_image(100, 100);
            encode_webp(&img, 80, Some(icc)).unwrap()
        }

        mod extraction_tests {
            use super::*;

            #[test]
            fn test_extract_icc_from_jpeg_with_profile() {
                let icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&icc);
                let extracted = extract_icc_ok(&jpeg);
                assert!(extracted.is_some());
                let extracted = extracted.unwrap();
                assert!(extracted.len() >= 128);
            }

            #[test]
            fn test_extract_icc_from_png_with_profile() {
                let icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&icc);
                let extracted = extract_icc_ok(&png);
                assert!(
                    extracted.is_some(),
                    "PNG ICC extraction should return Some when ICC profile is embedded correctly"
                );
                let extracted = extracted.unwrap();
                assert!(extracted.len() >= 128);
                assert_eq!(icc, extracted, "Extracted ICC should match original");
            }

            #[test]
            fn test_extract_icc_from_webp_with_profile() {
                let icc = create_minimal_srgb_icc();
                let webp = create_webp_with_icc(&icc);
                let extracted = extract_icc_ok(&webp);
                assert!(extracted.is_some());
            }

            #[test]
            fn test_extract_icc_returns_none_for_no_icc() {
                let jpeg = create_minimal_jpeg();
                let icc = extract_icc_ok(&jpeg);
                assert!(icc.is_none());
            }

            #[test]
            fn test_extract_icc_returns_none_for_non_image() {
                let icc = extract_icc_ok(b"not an image");
                assert!(icc.is_none());
            }

            #[test]
            fn test_extract_icc_returns_none_for_empty() {
                let icc = extract_icc_ok(&[]);
                assert!(icc.is_none());
            }
        }

        mod validation_tests {
            use super::*;

            #[test]
            fn test_validate_valid_icc() {
                let icc = create_minimal_srgb_icc();
                assert!(validate_icc_profile(&icc));
            }

            #[test]
            fn test_validate_truncated_icc() {
                let icc = create_minimal_srgb_icc();
                let truncated = &icc[..50];
                assert!(!validate_icc_profile(truncated));
            }

            #[test]
            fn test_validate_wrong_size_field() {
                let mut icc = create_minimal_srgb_icc();
                icc[0] = 0xFF;
                icc[1] = 0xFF;
                icc[2] = 0xFF;
                icc[3] = 0xFF;
                assert!(!validate_icc_profile(&icc));
            }

            #[test]
            fn test_validate_too_short() {
                assert!(!validate_icc_profile(&[0; 100]));
            }

            #[test]
            fn test_validate_empty() {
                assert!(!validate_icc_profile(&[]));
            }
        }

        mod roundtrip_tests {
            use super::*;

            #[test]
            fn test_jpeg_roundtrip() {
                let original_icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&original_icc);
                let extracted_icc = extract_icc_ok(&jpeg).unwrap();

                let img = image::load_from_memory(&jpeg).unwrap();
                let encoded = encode_jpeg(&img, 80, Some(&extracted_icc)).unwrap();
                let re_extracted_icc = extract_icc_ok(&encoded).unwrap();

                assert_eq!(extracted_icc, re_extracted_icc);
            }

            #[test]
            fn test_png_roundtrip() {
                let original_icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&original_icc);

                let extracted_icc = extract_icc_from_png_direct(&png);
                assert!(
                    extracted_icc.is_some(),
                    "PNG should contain iCCP chunk with ICC profile"
                );
                let extracted_icc = extracted_icc.unwrap();
                assert_eq!(
                    original_icc, extracted_icc,
                    "Extracted ICC should match original"
                );

                let img = image::load_from_memory(&png).unwrap();
                let encoded = encode_png(&img, Some(&extracted_icc)).unwrap();

                let re_extracted_icc = extract_icc_from_png_direct(&encoded);
                assert!(
                    re_extracted_icc.is_some(),
                    "Re-encoded PNG should also contain iCCP chunk"
                );
                assert_eq!(
                    extracted_icc,
                    re_extracted_icc.unwrap(),
                    "Re-extracted ICC should match original"
                );
            }

            #[test]
            fn test_webp_roundtrip() {
                let original_icc = create_minimal_srgb_icc();
                let webp = create_webp_with_icc(&original_icc);
                let extracted_icc = extract_icc_ok(&webp).unwrap();

                let img = image::load_from_memory(&webp).unwrap();
                let encoded = encode_webp(&img, 80, Some(&extracted_icc)).unwrap();
                let re_extracted_icc = extract_icc_ok(&encoded).unwrap();

                assert_eq!(extracted_icc, re_extracted_icc);
            }

            #[test]
            fn test_cross_format_roundtrip_jpeg_to_png() {
                let icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&icc);
                let extracted_icc = extract_icc_ok(&jpeg).unwrap();

                let img = image::load_from_memory(&jpeg).unwrap();
                let png = encode_png(&img, Some(&extracted_icc)).unwrap();

                let re_extracted = extract_icc_from_png_direct(&png);
                assert!(
                    re_extracted.is_some(),
                    "PNG should contain iCCP chunk with ICC profile from JPEG"
                );
                assert_eq!(
                    extracted_icc,
                    re_extracted.unwrap(),
                    "ICC profile should be preserved in JPEG to PNG conversion"
                );
            }

            #[test]
            fn test_cross_format_roundtrip_png_to_webp() {
                let icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&icc);

                let extracted_icc = extract_icc_from_png_direct(&png);
                assert!(
                    extracted_icc.is_some(),
                    "PNG should contain iCCP chunk with ICC profile"
                );
                let extracted_icc = extracted_icc.unwrap();
                assert_eq!(
                    icc, extracted_icc,
                    "Extracted ICC from PNG should match original"
                );

                let img = image::load_from_memory(&png).unwrap();
                let webp = encode_webp(&img, 80, Some(&extracted_icc)).unwrap();

                let re_extracted = extract_icc_ok(&webp).unwrap();
                assert_eq!(
                    extracted_icc, re_extracted,
                    "ICC profile should be preserved in PNG to WebP conversion"
                );
            }
        }

        #[cfg(feature = "avif")]
        mod avif_icc_tests {
            use super::*;
            use crate::engine::io::is_avif_data;

            #[test]
            fn test_avif_preserves_icc_profile() {
                let icc = create_minimal_srgb_icc();
                let img = create_test_image(100, 100);
                let avif = encode_avif(&img, 60, Some(&icc)).unwrap();

                assert!(is_avif_data(&avif), "Output should be valid AVIF");

                let extracted = extract_icc_ok(&avif);
                assert!(
                    extracted.is_some(),
                    "AVIF should now preserve ICC profile with libavif"
                );

                let extracted_icc = extracted.unwrap();
                assert_eq!(
                    extracted_icc.len(),
                    icc.len(),
                    "Extracted ICC size should match original"
                );
                assert_eq!(
                    &extracted_icc[..],
                    &icc[..],
                    "Extracted ICC data should match original"
                );
            }

            #[test]
            fn test_avif_encoding_with_icc_does_not_crash() {
                let icc = create_minimal_srgb_icc();
                let img = create_test_image(100, 100);
                let result = encode_avif(&img, 60, Some(&icc));
                assert!(result.is_ok(), "AVIF encoding with ICC should succeed");
            }

            #[test]
            fn test_avif_encoding_without_icc() {
                let img = create_test_image(100, 100);
                let avif = encode_avif(&img, 60, None).unwrap();

                assert!(is_avif_data(&avif), "Output should be valid AVIF");

                let extracted = extract_icc_ok(&avif);
                assert!(
                    extracted.is_none(),
                    "AVIF without ICC should not have ICC profile"
                );
            }
        }
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
                    icc_present: false,
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
                    icc_present: false,
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
                    icc_present: false,
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
                    icc_present: false,
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
                    icc_present: false,
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
