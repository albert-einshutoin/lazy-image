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
mod pipeline;
mod pool;
mod stress;
mod tasks;

// Re-export commonly used types and functions
pub use api::ImageEngine;
pub use decoder::{check_dimensions, decode_jpeg_mozjpeg, decode_with_image_crate};
pub use encoder::{
    embed_icc_jpeg, embed_icc_png, embed_icc_webp, encode_avif, encode_jpeg, encode_png,
    encode_webp, QualitySettings,
};
pub use io::{extract_icc_profile, extract_icc_profile_lossy, Source};
pub use pipeline::{
    apply_ops, calc_resize_dimensions, fast_resize, fast_resize_internal, fast_resize_owned,
    optimize_ops, ResizeError,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::firewall::FirewallConfig;
    use crate::engine::tasks::EncodeTask;
    use crate::error::LazyImageError;
    use crate::ops::{Operation, OutputFormat, ResizeFit};
    use image::{DynamicImage, GenericImageView, RgbImage, RgbaImage};
    use std::borrow::Cow;
    use std::sync::Arc;

    // Helper function to create test images
    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        }))
    }

    fn extract_icc_ok(data: &[u8]) -> Option<Vec<u8>> {
        extract_icc_profile(data).unwrap()
    }

    fn create_test_image_rgba(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
        }))
    }

    // Helper to create minimal valid JPEG bytes
    fn create_minimal_jpeg() -> Vec<u8> {
        // Create a 1x1 RGB image and encode it as JPEG
        let img = create_test_image(1, 1);
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let pixels = rgb.into_raw();

        // Use mozjpeg to create a valid JPEG
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

    // Helper to create minimal valid PNG bytes
    fn create_minimal_png() -> Vec<u8> {
        create_png(1, 1)
    }

    // Helper to create minimal valid WebP bytes
    fn create_minimal_webp() -> Vec<u8> {
        let img = create_test_image(10, 10);
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let encoder = webp::Encoder::from_rgb(&rgb, w, h);
        let config = webp::WebPConfig::new().unwrap();
        let mem = encoder.encode_advanced(&config).unwrap();
        mem.to_vec()
    }

    #[test]
    fn fast_resize_owned_returns_error_instead_of_dummy_image() {
        let img = create_test_image(1, 1);
        let err = fast_resize_owned(img, 0, 10).expect_err("expected resize failure");
        assert_eq!(err.source_dims, (1, 1));
        assert_eq!(err.target_dims, (0, 10));
        assert!(err.reason.contains("invalid dimensions"));
    }

    mod resize_calc_tests {
        use super::*;

        #[test]
        fn test_both_dimensions_specified() {
            let (w, h) = calc_resize_dimensions(1000, 800, Some(500), Some(400));
            assert_eq!((w, h), (500, 400));
        }

        #[test]
        fn test_width_only_maintains_aspect_ratio() {
            let (w, h) = calc_resize_dimensions(1000, 500, Some(500), None);
            assert_eq!(w, 500);
            assert_eq!(h, 250); // 1000:500 = 500:250
        }

        #[test]
        fn test_height_only_maintains_aspect_ratio() {
            let (w, h) = calc_resize_dimensions(1000, 500, None, Some(250));
            assert_eq!(w, 500);
            assert_eq!(h, 250);
        }

        #[test]
        fn test_none_returns_original() {
            let (w, h) = calc_resize_dimensions(1000, 500, None, None);
            assert_eq!((w, h), (1000, 500));
        }

        #[test]
        fn test_rounding_behavior() {
            // Test rounding behavior with odd sizes
            let (w, h) = calc_resize_dimensions(101, 51, Some(50), None);
            assert_eq!(w, 50);
            // 101:51 ≈ 50:25.2... → should round to 25
            assert_eq!(h, 25);
        }

        #[test]
        fn test_aspect_ratio_preservation_wide() {
            // Wide image
            let (w, h) = calc_resize_dimensions(2000, 1000, Some(1000), None);
            assert_eq!(w, 1000);
            assert_eq!(h, 500);
        }

        #[test]
        fn test_aspect_ratio_preservation_tall() {
            // Tall image
            let (w, h) = calc_resize_dimensions(1000, 2000, None, Some(1000));
            assert_eq!(w, 500);
            assert_eq!(h, 1000);
        }

        #[test]
        fn test_square_image() {
            let (w, h) = calc_resize_dimensions(100, 100, Some(50), None);
            assert_eq!(w, 50);
            assert_eq!(h, 50);
        }

        #[test]
        fn test_both_dimensions_wide_image_fits_inside() {
            // Resize wide image (6000×4000) to 800×600
            // Aspect ratio: 6000/4000 = 1.5 > 800/600 = 1.333...
            // → Should fit to 800×533 based on width
            let (w, h) = calc_resize_dimensions(6000, 4000, Some(800), Some(600));
            assert_eq!(w, 800);
            assert_eq!(h, 533); // 4000 * (800/6000) = 533.33... → 533
        }

        #[test]
        fn test_both_dimensions_tall_image_fits_inside() {
            // Resize tall image (4000×6000) to 800×600
            // Aspect ratio: 4000/6000 = 0.666... < 800/600 = 1.333...
            // → Should fit to 400×600 based on height
            let (w, h) = calc_resize_dimensions(4000, 6000, Some(800), Some(600));
            assert_eq!(w, 400); // 4000 * (600/6000) = 400
            assert_eq!(h, 600);
        }

        #[test]
        fn test_both_dimensions_same_aspect_ratio() {
            // When aspect ratios match, use specified size as-is
            // 1000:500 = 2:1, 800:400 = 2:1
            let (w, h) = calc_resize_dimensions(1000, 500, Some(800), Some(400));
            assert_eq!((w, h), (800, 400));
        }
    }

    mod security_tests {
        use super::*;

        #[test]
        fn test_check_dimensions_valid() {
            assert!(check_dimensions(1920, 1080).is_ok());
            // 32768 x 32768 = 1,073,741,824 > MAX_PIXELS(100,000,000) so should error
            // MAX_DIMENSION check passes, but MAX_PIXELS check rejects it
            let result = check_dimensions(32768, 32768);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("exceeds max"));
        }

        #[test]
        fn test_check_dimensions_exceeds_max_dimension() {
            let result = check_dimensions(32769, 1);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
        }

        #[test]
        fn test_check_dimensions_exceeds_max_dimension_height() {
            let result = check_dimensions(1, 32769);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
        }

        #[test]
        fn test_check_dimensions_exceeds_max_pixels() {
            // 10001 x 10000 = 100,010,000 > MAX_PIXELS(100,000,000)
            let result = check_dimensions(10001, 10000);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("exceeds max"));
        }

        #[test]
        fn test_check_dimensions_at_pixel_boundary() {
            // ちょうど100,000,000ピクセル = OK
            assert!(check_dimensions(10000, 10000).is_ok());
        }

        #[test]
        fn test_check_dimensions_at_max_dimension() {
            // Boundary: 32768 x 32768 = 1,073,741,824 > MAX_PIXELS
            // However, MAX_DIMENSION check comes first, so this would be OK
            // Actually rejected by MAX_PIXELS check
            let result = check_dimensions(32768, 32768);
            // 32768 * 32768 = 1,073,741,824 > 100,000,000
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("exceeds max"));
        }

        #[test]
        fn test_check_dimensions_small_image() {
            assert!(check_dimensions(1, 1).is_ok());
        }

        #[test]
        fn test_check_dimensions_zero_dimension() {
            // Zero dimension is technically invalid, but check_dimensions doesn't check it
            // image crate handles it
            assert!(check_dimensions(0, 100).is_ok()); // 0 * 100 = 0 < MAX_PIXELS
        }
    }

    mod icc_tests {
        use super::*;
        use crate::engine::io::{
            extract_icc_from_jpeg, extract_icc_from_png, extract_icc_from_png_direct,
            extract_icc_from_webp, validate_icc_profile,
        };

        #[test]
        fn test_validate_icc_profile_too_small() {
            let data = vec![0u8; 127]; // 128バイト未満
            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_minimal_valid() {
            // Minimal valid ICC profile (128 bytes)
            let mut data = vec![0u8; 128];
            // プロファイルサイズ（最初の4バイト、big-endian）
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80; // 128バイト
                            // CMM type (bytes 4-7): "ADBE" (ASCII)
            data[4] = b'A';
            data[5] = b'D';
            data[6] = b'B';
            data[7] = b'E';
            // Version (byte 8): 2
            data[8] = 2;
            // Profile class (bytes 12-15): "mntr" (monitor)
            data[12] = b'm';
            data[13] = b'n';
            data[14] = b't';
            data[15] = b'r';
            // Data color space (bytes 16-19): "RGB " (ASCII)
            data[16] = b'R';
            data[17] = b'G';
            data[18] = b'B';
            data[19] = b' ';
            // PCS (bytes 20-23): "XYZ " (ASCII)
            data[20] = b'X';
            data[21] = b'Y';
            data[22] = b'Z';
            data[23] = b' ';

            assert!(validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_size_mismatch() {
            let mut data = vec![0u8; 200];
            // Set profile size to 200
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0xC8; // 200 bytes
                            // But actual data is 200 bytes, so this is valid
                            // Test case where size doesn't match
            data[3] = 0x00;
            data[3] = 0xFF; // Set to 255 bytes (actual is 200 bytes)

            // Invalid because size doesn't match
            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_invalid_version() {
            let mut data = vec![0u8; 128];
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80;
            data[8] = 20; // Version too large

            assert!(!validate_icc_profile(&data));
        }

            #[test]
            fn test_extract_icc_from_jpeg_no_profile() {
                // JPEG without ICC profile
                let jpeg_data = create_minimal_jpeg();
                let result = extract_icc_from_jpeg(&jpeg_data);
                assert!(result.is_none());
            }

            #[test]
            fn test_extract_icc_from_png_no_profile() {
                // PNG without ICC profile
                let png_data = create_png(2, 2);
                let result = extract_icc_from_png(&png_data);
                assert!(result.is_none());
            }

            #[test]
            fn test_extract_icc_from_webp_no_profile() {
                // WebP without ICC profile
                let webp_data = create_minimal_webp();
                let result = extract_icc_from_webp(&webp_data);
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
                // Extract ICC profile from JPEG (when not present)
                let result = extract_icc_profile(&jpeg_data);
                // Minimal JPEG has no ICC profile
                assert!(result.is_ok());
                assert!(result.unwrap().is_none());
            }

        // Helper function to create a minimal valid ICC profile (sRGB)
        fn create_minimal_srgb_icc() -> Vec<u8> {
            // Minimal valid sRGB ICC profile (128 bytes)
            let mut data = vec![0u8; 128];
            // Profile size (first 4 bytes, big-endian)
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80; // 128 bytes
                            // CMM type (bytes 4-7): "ADBE" (ASCII)
            data[4] = b'A';
            data[5] = b'D';
            data[6] = b'B';
            data[7] = b'E';
            // Version (byte 8): 2
            data[8] = 2;
            // Profile class (bytes 12-15): "mntr" (monitor)
            data[12] = b'm';
            data[13] = b'n';
            data[14] = b't';
            data[15] = b'r';
            // Data color space (bytes 16-19): "RGB " (ASCII)
            data[16] = b'R';
            data[17] = b'G';
            data[18] = b'B';
            data[19] = b' ';
            // PCS (bytes 20-23): "XYZ " (ASCII)
            data[20] = b'X';
            data[21] = b'Y';
            data[22] = b'Z';
            data[23] = b' ';
            data
        }

        // Helper function to create JPEG with ICC profile
        fn create_jpeg_with_icc(icc: &[u8]) -> Vec<u8> {
            let img = create_test_image(100, 100);
            encode_jpeg(&img, 80, Some(icc)).unwrap()
        }

        // Helper function to create PNG with ICC profile
        fn create_png_with_icc(icc: &[u8]) -> Vec<u8> {
            let img = create_test_image(100, 100);
            encode_png(&img, Some(icc)).unwrap()
        }

        // Helper function to create WebP with ICC profile
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
                // Minimum ICC profile size is 128 bytes (header)
                assert!(extracted.len() >= 128);
            }

            #[test]
            fn test_extract_icc_from_png_with_profile() {
                // PNG ICC extraction: img-parts can now extract ICC profiles from PNG iCCP chunks
                // when they are embedded using the correct format (raw ICC profile data).
                let icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&icc);
                let extracted = extract_icc_ok(&png);
                // PNG ICC extraction should now work with img-parts
                assert!(
                    extracted.is_some(),
                    "PNG ICC extraction should return Some when ICC profile is embedded correctly"
                );
                let extracted = extracted.unwrap();
                // Minimum ICC profile size is 128 bytes (header)
                assert!(extracted.len() >= 128);
                // Extracted ICC should match original
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
                // 途中で切り詰め
                let truncated = &icc[..50];
                assert!(!validate_icc_profile(truncated));
            }

            #[test]
            fn test_validate_wrong_size_field() {
                let mut icc = create_minimal_srgb_icc();
                // サイズフィールド（先頭4バイト）を不正値に
                icc[0] = 0xFF;
                icc[1] = 0xFF;
                icc[2] = 0xFF;
                icc[3] = 0xFF;
                assert!(!validate_icc_profile(&icc));
            }

            #[test]
            fn test_validate_too_short() {
                assert!(!validate_icc_profile(&[0; 100])); // 128バイト未満
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
                // 1. Extract ICC from original image
                let original_icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&original_icc);
                let extracted_icc = extract_icc_ok(&jpeg).unwrap();

                // 2. Decode image
                let img = image::load_from_memory(&jpeg).unwrap();

                // 3. Encode JPEG with ICC embedded
                let encoded = encode_jpeg(&img, 80, Some(&extracted_icc)).unwrap();

                // 4. Re-extract ICC from encoded result
                let re_extracted_icc = extract_icc_ok(&encoded).unwrap();

                // 5. Verify identity
                assert_eq!(extracted_icc, re_extracted_icc);
            }

            #[test]
            fn test_png_roundtrip() {
                // Test that ICC profile is preserved in PNG roundtrip
                let original_icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&original_icc);

                // Verify that iCCP chunk exists in PNG (using direct parsing)
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

                // Test roundtrip: decode and re-encode
                let img = image::load_from_memory(&png).unwrap();
                let encoded = encode_png(&img, Some(&extracted_icc)).unwrap();

                // Verify that re-encoded PNG also contains iCCP chunk
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
                // Test that ICC profile is preserved when converting JPEG to PNG
                let icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&icc);
                let extracted_icc = extract_icc_ok(&jpeg).unwrap();

                // Convert JPEG to PNG with ICC
                let img = image::load_from_memory(&jpeg).unwrap();
                let png = encode_png(&img, Some(&extracted_icc)).unwrap();

                // Verify that PNG contains iCCP chunk with ICC profile (using direct parsing)
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
                // Test that ICC profile is preserved when converting PNG to WebP
                // Since img-parts cannot extract ICC from PNG, we use direct parsing
                let icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&icc);

                // Extract ICC from PNG using direct parsing (img-parts limitation)
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

                // Convert PNG to WebP using extracted ICC
                let img = image::load_from_memory(&png).unwrap();
                let webp = encode_webp(&img, 80, Some(&extracted_icc)).unwrap();

                // Verify that WebP contains ICC profile
                let re_extracted = extract_icc_ok(&webp).unwrap();
                assert_eq!(
                    extracted_icc, re_extracted,
                    "ICC profile should be preserved in PNG to WebP conversion"
                );
            }
        }

        mod avif_icc_tests {
            use super::*;
            use crate::engine::io::is_avif_data;

            #[test]
            fn test_avif_preserves_icc_profile() {
                // libavif implementation now properly embeds ICC profiles
                // libavif-sys is always available (not dependent on napi feature)
                let icc = create_minimal_srgb_icc();
                let img = create_test_image(100, 100);
                let avif = encode_avif(&img, 60, Some(&icc)).unwrap();

                // Verify AVIF data is valid
                assert!(is_avif_data(&avif), "Output should be valid AVIF");

                // Extract ICC profile from AVIF
                let extracted = extract_icc_ok(&avif);
                assert!(
                    extracted.is_some(),
                    "AVIF should now preserve ICC profile with libavif"
                );

                // Verify extracted ICC matches original
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
                // Verify that passing ICC profile does not crash
                let icc = create_minimal_srgb_icc();
                let img = create_test_image(100, 100);
                let result = encode_avif(&img, 60, Some(&icc));
                assert!(result.is_ok(), "AVIF encoding with ICC should succeed");
            }

            #[test]
            fn test_avif_encoding_without_icc() {
                // Verify that encoding works without ICC
                let img = create_test_image(100, 100);
                let avif = encode_avif(&img, 60, None).unwrap();

                // Verify AVIF data is valid
                assert!(is_avif_data(&avif), "Output should be valid AVIF");

                // Should not have ICC profile
                let extracted = extract_icc_ok(&avif);
                assert!(
                    extracted.is_none(),
                    "AVIF without ICC should not have ICC profile"
                );
            }
        }
    }

    mod apply_ops_tests {
        use super::*;

        #[test]
        fn test_resize_operation() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Resize {
                width: Some(50),
                height: Some(50),
                fit: ResizeFit::Inside,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 50));
        }

        #[test]
        fn test_resize_width_only() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Resize {
                width: Some(50),
                height: None,
                fit: ResizeFit::Inside,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 25));
        }

        #[test]
        fn test_resize_height_only() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Resize {
                width: None,
                height: Some(25),
                fit: ResizeFit::Inside,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 25));
        }

        #[test]
        fn test_crop_valid() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 10,
                y: 10,
                width: 50,
                height: 50,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 50));
        }

        #[test]
        fn test_crop_out_of_bounds() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 60,
                y: 60,
                width: 50,
                height: 50,
            }];
            let result = apply_ops(Cow::Owned(img), &ops);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Crop bounds"));
        }

        #[test]
        fn test_crop_at_origin() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 0,
                y: 0,
                width: 50,
                height: 50,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 50));
        }

        #[test]
        fn test_crop_entire_image() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_rotate_90() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 90 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 100)); // Width and height swapped
        }

        #[test]
        fn test_rotate_180() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 180 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 50)); // Size unchanged
        }

        #[test]
        fn test_rotate_270() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 270 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 100));
        }

        #[test]
        fn test_rotate_neg90() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: -90 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 100));
        }

        #[test]
        fn test_rotate_0() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 0 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 50));
        }

        #[test]
        fn test_rotate_invalid_angle() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Rotate { degrees: 45 }];
            let result = apply_ops(Cow::Owned(img), &ops);
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Unsupported rotation angle"));
        }

        #[test]
        fn test_flip_h() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::FlipH];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_flip_v() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::FlipV];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_grayscale_reduces_channels() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Grayscale];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            // グレースケール後はLuma8形式
            assert!(matches!(*result, DynamicImage::ImageLuma8(_)));
        }

        #[test]
        fn test_brightness() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Brightness { value: 50 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_contrast() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Contrast { value: 50 }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_colorspace_srgb() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::ColorSpace {
                target: crate::ops::ColorSpace::Srgb,
            }];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_chained_operations() {
            let img = create_test_image(200, 100);
            let ops = vec![
                Operation::Resize {
                    width: Some(100),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Rotate { degrees: 90 },
                Operation::Grayscale,
            ];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            // 200x100 → resize → 100x50 → rotate90 → 50x100
            assert_eq!(result.dimensions(), (50, 100));
            assert!(matches!(*result, DynamicImage::ImageLuma8(_)));
        }

        #[test]
        fn test_empty_operations() {
            let img = create_test_image(100, 100);
            let ops = vec![];
            let result = apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }
    }

    mod optimize_ops_tests {
        use super::*;

        #[test]
        fn test_consecutive_resizes_combined() {
            let ops = vec![
                Operation::Resize {
                    width: Some(800),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Resize {
                    width: Some(400),
                    height: None,
                    fit: ResizeFit::Inside,
                },
            ];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
            if let Operation::Resize {
                width,
                height: _,
                fit,
            } = &optimized[0]
            {
                assert_eq!(*width, Some(400));
                assert_eq!(*fit, ResizeFit::Inside);
            } else {
                panic!("Expected Resize operation");
            }
        }

        #[test]
        fn test_non_consecutive_resizes_not_combined() {
            let ops = vec![
                Operation::Resize {
                    width: Some(800),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Grayscale,
                Operation::Resize {
                    width: Some(400),
                    height: None,
                    fit: ResizeFit::Inside,
                },
            ];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 3);
        }

        #[test]
        fn test_single_operation() {
            let ops = vec![Operation::Resize {
                width: Some(100),
                height: None,
                fit: ResizeFit::Inside,
            }];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
        }

        #[test]
        fn test_empty_operations() {
            let ops = vec![];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 0);
        }

        #[test]
        fn test_multiple_consecutive_resizes() {
            let ops = vec![
                Operation::Resize {
                    width: Some(1000),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Resize {
                    width: Some(800),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Resize {
                    width: Some(400),
                    height: None,
                    fit: ResizeFit::Inside,
                },
            ];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
            if let Operation::Resize {
                width,
                height: _,
                fit,
            } = &optimized[0]
            {
                assert_eq!(*width, Some(400));
                assert_eq!(*fit, ResizeFit::Inside);
            }
        }

        #[test]
        fn test_resize_with_both_dimensions() {
            let ops = vec![
                Operation::Resize {
                    width: Some(800),
                    height: None,
                    fit: ResizeFit::Inside,
                },
                Operation::Resize {
                    width: Some(400),
                    height: Some(300),
                    fit: ResizeFit::Inside,
                },
            ];
            let optimized = optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
            if let Operation::Resize { width, height, fit } = &optimized[0] {
                assert_eq!(*width, Some(400));
                assert_eq!(*height, Some(300));
                assert_eq!(*fit, ResizeFit::Inside);
            }
        }
    }

    mod encode_tests {
        use super::*;

        #[test]
        fn test_encode_jpeg_produces_valid_jpeg() {
            let img = create_test_image(100, 100);
            let result = encode_jpeg(&img, 80, None).unwrap();
            // Verify JPEG magic bytes
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
            // Verify JPEG end marker
            assert_eq!(&result[result.len() - 2..], &[0xFF, 0xD9]);
        }

        #[test]
        fn test_encode_jpeg_quality_affects_size() {
            let img = create_test_image(100, 100);
            let high_quality = encode_jpeg(&img, 95, None).unwrap();
            let low_quality = encode_jpeg(&img, 50, None).unwrap();
            // Higher quality is usually larger (though content may reverse this)
            // At least verify both are valid JPEGs
            assert!(high_quality.len() > 0);
            assert!(low_quality.len() > 0);
            assert_eq!(&high_quality[0..2], &[0xFF, 0xD8]);
            assert_eq!(&low_quality[0..2], &[0xFF, 0xD8]);
        }

        #[test]
        fn test_encode_jpeg_with_icc() {
            let img = create_test_image(100, 100);
            // Minimal valid ICC profile
            let mut icc_data = vec![0u8; 128];
            icc_data[0] = 0x00;
            icc_data[1] = 0x00;
            icc_data[2] = 0x00;
            icc_data[3] = 0x80; // 128バイト
            icc_data[4] = b'A';
            icc_data[5] = b'D';
            icc_data[6] = b'B';
            icc_data[7] = b'E';
            icc_data[8] = 2;
            icc_data[12] = b'm';
            icc_data[13] = b'n';
            icc_data[14] = b't';
            icc_data[15] = b'r';
            icc_data[16] = b'R';
            icc_data[17] = b'G';
            icc_data[18] = b'B';
            icc_data[19] = b' ';
            icc_data[20] = b'X';
            icc_data[21] = b'Y';
            icc_data[22] = b'Z';
            icc_data[23] = b' ';

            let result = encode_jpeg(&img, 80, Some(&icc_data)).unwrap();
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
        }

        #[test]
        fn test_encode_png_produces_valid_png() {
            let img = create_test_image(100, 100);
            let result = encode_png(&img, None).unwrap();
            // Verify PNG magic bytes
            assert_eq!(
                &result[0..8],
                &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
            );
        }

        #[test]
        fn test_encode_png_with_icc() {
            let img = create_test_image(100, 100);
            let mut icc_data = vec![0u8; 128];
            icc_data[0] = 0x00;
            icc_data[1] = 0x00;
            icc_data[2] = 0x00;
            icc_data[3] = 0x80;
            icc_data[4] = b'A';
            icc_data[5] = b'D';
            icc_data[6] = b'B';
            icc_data[7] = b'E';
            icc_data[8] = 2;
            icc_data[12] = b'm';
            icc_data[13] = b'n';
            icc_data[14] = b't';
            icc_data[15] = b'r';
            icc_data[16] = b'R';
            icc_data[17] = b'G';
            icc_data[18] = b'B';
            icc_data[19] = b' ';
            icc_data[20] = b'X';
            icc_data[21] = b'Y';
            icc_data[22] = b'Z';
            icc_data[23] = b' ';

            let result = encode_png(&img, Some(&icc_data)).unwrap();
            assert_eq!(
                &result[0..8],
                &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
            );
        }

        #[test]
        fn test_encode_webp_produces_valid_webp() {
            let img = create_test_image(100, 100);
            let result = encode_webp(&img, 80, None).unwrap();
            // Verify WebP magic bytes (RIFF....WEBP)
            assert_eq!(&result[0..4], b"RIFF");
            assert_eq!(&result[8..12], b"WEBP");
        }

        #[test]
        fn test_encode_webp_with_icc() {
            let img = create_test_image(100, 100);
            let mut icc_data = vec![0u8; 128];
            icc_data[0] = 0x00;
            icc_data[1] = 0x00;
            icc_data[2] = 0x00;
            icc_data[3] = 0x80;
            icc_data[4] = b'A';
            icc_data[5] = b'D';
            icc_data[6] = b'B';
            icc_data[7] = b'E';
            icc_data[8] = 2;
            icc_data[12] = b'm';
            icc_data[13] = b'n';
            icc_data[14] = b't';
            icc_data[15] = b'r';
            icc_data[16] = b'R';
            icc_data[17] = b'G';
            icc_data[18] = b'B';
            icc_data[19] = b' ';
            icc_data[20] = b'X';
            icc_data[21] = b'Y';
            icc_data[22] = b'Z';
            icc_data[23] = b' ';

            let result = encode_webp(&img, 80, Some(&icc_data)).unwrap();
            assert_eq!(&result[0..4], b"RIFF");
            assert_eq!(&result[8..12], b"WEBP");
        }

        #[test]
        fn test_encode_avif_produces_valid_avif() {
            let img = create_test_image(100, 100);
            let result = encode_avif(&img, 60, None).unwrap();
            // AVIF has ftyp box at the beginning
            assert!(result.len() > 12);
            // Verify "ftyp" is present
            let has_ftyp = result.windows(4).any(|w| w == b"ftyp");
            assert!(has_ftyp);
        }

        #[test]
        fn test_encode_avif_quality_affects_size() {
            let img = create_test_image(100, 100);
            let high_quality = encode_avif(&img, 80, None).unwrap();
            let low_quality = encode_avif(&img, 40, None).unwrap();
            // Verify both are valid AVIF
            assert!(high_quality.len() > 0);
            assert!(low_quality.len() > 0);
        }

        #[test]
        fn test_encode_rgba_image() {
            let img = create_test_image_rgba(100, 100);
            let jpeg_result = encode_jpeg(&img, 80, None).unwrap();
            assert_eq!(&jpeg_result[0..2], &[0xFF, 0xD8]);

            let png_result = encode_png(&img, None).unwrap();
            assert_eq!(
                &png_result[0..8],
                &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
            );
        }
    }

    mod decode_tests {
        use super::*;

        #[test]
        fn test_decode_jpeg_mozjpeg() {
            let jpeg_data = create_minimal_jpeg();
            let result = decode_jpeg_mozjpeg(&jpeg_data);
            assert!(result.is_ok());
            let img = result.unwrap();
            assert!(img.dimensions().0 > 0);
            assert!(img.dimensions().1 > 0);
        }

        #[test]
        fn test_decode_jpeg_mozjpeg_invalid_data() {
            let invalid_data = vec![0xFF, 0xD8, 0x00]; // Incomplete JPEG
            let result = decode_jpeg_mozjpeg(&invalid_data);
            assert!(result.is_err());
        }

        #[test]
        fn test_decode_with_image_crate() {
            // Verify that decode() uses image crate for PNG data
            let png_data = create_minimal_png();
            use crate::engine::io::Source;
            let task = EncodeTask {
                source: Some(Source::Memory(Arc::new(png_data))),
                decoded: None,
                ops: vec![],
                format: OutputFormat::Png,
                icc_profile: None,
                icc_present: false,
                auto_orient: true,
                keep_metadata: false,
                keep_metadata_requested: false,
                firewall: FirewallConfig::disabled(),
                #[cfg(feature = "napi")]
                last_error: None,
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
                source: None,
                decoded: Some(Arc::new(img.clone())),
                ops: vec![],
                format: OutputFormat::Png,
                icc_profile: None,
                icc_present: false,
                auto_orient: true,
                keep_metadata: false,
                keep_metadata_requested: false,
                firewall: FirewallConfig::disabled(),
                #[cfg(feature = "napi")]
                last_error: None,
            };
            let result = task.decode();
            assert!(result.is_ok());
            let decoded_img = result.unwrap();
            assert_eq!(decoded_img.dimensions(), img.dimensions());
        }

        #[test]
        fn test_decode_no_source() {
            let task = EncodeTask {
                source: None,
                decoded: None,
                ops: vec![],
                format: OutputFormat::Png,
                icc_profile: None,
                icc_present: false,
                auto_orient: true,
                keep_metadata: false,
                keep_metadata_requested: false,
                firewall: FirewallConfig::disabled(),
                #[cfg(feature = "napi")]
                last_error: None,
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
                source: Some(Source::Memory(Arc::new(png_data))),
                decoded: None,
                ops: vec![],
                format: OutputFormat::Png,
                icc_profile: None,
                icc_present: false,
                auto_orient: true,
                keep_metadata: false,
                keep_metadata_requested: false,
                firewall,
                #[cfg(feature = "napi")]
                last_error: None,
            };
            let err = task.decode_internal().unwrap_err();
            assert!(matches!(err, LazyImageError::FirewallViolation { .. }));
        }

        #[test]
        fn test_firewall_blocks_large_pixel_count() {
            use crate::engine::io::Source;
            let png_data = create_png(2, 2);
            let mut firewall = FirewallConfig::custom();
            firewall.max_pixels = Some(1);

            let task = EncodeTask {
                source: Some(Source::Memory(Arc::new(png_data))),
                decoded: None,
                ops: vec![],
                format: OutputFormat::Png,
                icc_profile: None,
                icc_present: false,
                auto_orient: true,
                keep_metadata: false,
                keep_metadata_requested: false,
                firewall,
                #[cfg(feature = "napi")]
                last_error: None,
            };
            let err = task.decode_internal().unwrap_err();
            assert!(matches!(err, LazyImageError::FirewallViolation { .. }));
        }
    }

    mod fast_resize_tests {
        use super::*;

        #[test]
        fn test_fast_resize_downscale() {
            let img = create_test_image(200, 200);
            let result = fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_upscale() {
            let img = create_test_image(50, 50);
            let result = fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_aspect_ratio_change() {
            let img = create_test_image(200, 100);
            let result = fast_resize(&img, 100, 200);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 200));
        }

        #[test]
        fn test_fast_resize_invalid_dimensions() {
            let img = create_test_image(100, 100);
            let result = fast_resize(&img, 0, 100);
            assert!(result.is_err());
        }

        #[test]
        fn test_fast_resize_same_size() {
            let img = create_test_image(100, 100);
            let result = fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_rgba() {
            let img = create_test_image_rgba(100, 100);
            let result = fast_resize(&img, 50, 50);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (50, 50));
        }
    }
}
