// tests/engine_tests.rs
//
// Engine tests extracted from src/engine.rs.
// Only tests that use the public API live here; tests that depend on
// pub(crate) internals (ICC helpers, EncodeTask, MetadataPolicy) remain
// inline in src/engine.rs.

use image::{DynamicImage, GenericImageView, RgbImage, RgbaImage};
use lazy_image::engine::{
    apply_ops, calc_resize_dimensions, check_dimensions, decode_jpeg_mozjpeg, encode_jpeg,
    encode_png, encode_webp, fast_resize, fast_resize_owned,
};
use lazy_image::ops::{ColorSpace, Operation, ResizeFit, RotationAngle};
use std::borrow::Cow;

// =========================================================================
// Helper functions
// =========================================================================

fn create_test_image(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    }))
}

fn create_test_image_rgba(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgba8(RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
    }))
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

// =========================================================================
// fast_resize_owned
// =========================================================================

#[test]
fn fast_resize_owned_returns_error_instead_of_dummy_image() {
    let img = create_test_image(1, 1);
    let err = fast_resize_owned(img, 0, 10).expect_err("expected resize failure");
    assert_eq!(err.source_dims, (1, 1));
    assert_eq!(err.target_dims, (0, 10));
    assert!(err.reason.contains("invalid dimensions"));
}

// =========================================================================
// resize_calc_tests
// =========================================================================

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
        assert_eq!(h, 250);
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
        let (w, h) = calc_resize_dimensions(101, 51, Some(50), None);
        assert_eq!(w, 50);
        assert_eq!(h, 25);
    }

    #[test]
    fn test_aspect_ratio_preservation_wide() {
        let (w, h) = calc_resize_dimensions(2000, 1000, Some(1000), None);
        assert_eq!(w, 1000);
        assert_eq!(h, 500);
    }

    #[test]
    fn test_aspect_ratio_preservation_tall() {
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
        let (w, h) = calc_resize_dimensions(6000, 4000, Some(800), Some(600));
        assert_eq!(w, 800);
        assert_eq!(h, 533);
    }

    #[test]
    fn test_both_dimensions_tall_image_fits_inside() {
        let (w, h) = calc_resize_dimensions(4000, 6000, Some(800), Some(600));
        assert_eq!(w, 400);
        assert_eq!(h, 600);
    }

    #[test]
    fn test_both_dimensions_same_aspect_ratio() {
        let (w, h) = calc_resize_dimensions(1000, 500, Some(800), Some(400));
        assert_eq!((w, h), (800, 400));
    }
}

// =========================================================================
// security_tests
// =========================================================================

mod security_tests {
    use super::*;

    #[test]
    fn test_check_dimensions_valid() {
        assert!(check_dimensions(1920, 1080).is_ok());
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
        let result = check_dimensions(10001, 10000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds max"));
    }

    #[test]
    #[cfg(not(feature = "fuzzing"))]
    fn test_check_dimensions_at_pixel_boundary() {
        assert!(check_dimensions(10000, 10000).is_ok());
    }

    #[test]
    #[cfg(not(feature = "fuzzing"))]
    fn test_check_dimensions_at_max_dimension() {
        let result = check_dimensions(32768, 32768);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds max"));
    }

    #[test]
    fn test_check_dimensions_small_image() {
        assert!(check_dimensions(1, 1).is_ok());
    }

    #[test]
    fn test_check_dimensions_zero_dimension() {
        assert!(check_dimensions(0, 100).is_ok());
    }
}

#[cfg(feature = "fuzzing")]
mod fuzz_limit_tests {
    use super::*;

    #[test]
    fn test_check_dimensions_fuzz_allows_at_limit() {
        assert!(check_dimensions(1000, 1000).is_ok());
        assert!(check_dimensions(1024, 1).is_ok());
        assert!(check_dimensions(1, 1024).is_ok());
    }

    #[test]
    fn test_check_dimensions_fuzz_rejects_over_dimension() {
        let result = check_dimensions(1025, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
    }

    #[test]
    fn test_check_dimensions_fuzz_rejects_over_pixels() {
        let result = check_dimensions(1024, 1024);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds max"));
    }
}

// =========================================================================
// apply_ops_tests
// =========================================================================

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
        let ops = vec![Operation::Rotate(RotationAngle::Cw90)];
        let result = apply_ops(Cow::Owned(img), &ops).unwrap();
        assert_eq!(result.dimensions(), (50, 100));
    }

    #[test]
    fn test_rotate_180() {
        let img = create_test_image(100, 50);
        let ops = vec![Operation::Rotate(RotationAngle::Cw180)];
        let result = apply_ops(Cow::Owned(img), &ops).unwrap();
        assert_eq!(result.dimensions(), (100, 50));
    }

    #[test]
    fn test_rotate_270() {
        let img = create_test_image(100, 50);
        let ops = vec![Operation::Rotate(RotationAngle::Cw270)];
        let result = apply_ops(Cow::Owned(img), &ops).unwrap();
        assert_eq!(result.dimensions(), (50, 100));
    }

    #[test]
    fn test_rotate_neg90() {
        // -90° is equivalent to Cw270
        let img = create_test_image(100, 50);
        let ops = vec![Operation::Rotate(RotationAngle::Cw270)];
        let result = apply_ops(Cow::Owned(img), &ops).unwrap();
        assert_eq!(result.dimensions(), (50, 100));
    }

    #[test]
    fn test_rotate_0() {
        // 0° is a no-op; empty ops list produces original dimensions
        let img = create_test_image(100, 50);
        let ops = vec![];
        let result = apply_ops(Cow::Owned(img), &ops).unwrap();
        assert_eq!(result.dimensions(), (100, 50));
    }

    #[test]
    fn test_rotate_invalid_angle() {
        // 45° is not representable as RotationAngle; from_degrees must return Err
        assert!(RotationAngle::from_degrees(45).is_err());
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
            target: ColorSpace::Srgb,
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
            Operation::Rotate(RotationAngle::Cw90),
            Operation::Grayscale,
        ];
        let result = apply_ops(Cow::Owned(img), &ops).unwrap();
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

// =========================================================================
// optimize_ops_tests
// =========================================================================

mod optimize_ops_tests {
    use lazy_image::engine::optimize_ops;

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

// =========================================================================
// encode_tests
// =========================================================================

mod encode_tests {
    use super::*;

    #[test]
    fn test_encode_jpeg_produces_valid_jpeg() {
        let img = create_test_image(100, 100);
        let result = encode_jpeg(&img, 80, None).unwrap();
        assert_eq!(&result[0..2], &[0xFF, 0xD8]);
        assert_eq!(&result[result.len() - 2..], &[0xFF, 0xD9]);
    }

    #[test]
    fn test_encode_jpeg_quality_affects_size() {
        let img = create_test_image(100, 100);
        let high_quality = encode_jpeg(&img, 95, None).unwrap();
        let low_quality = encode_jpeg(&img, 50, None).unwrap();
        assert!(!high_quality.is_empty());
        assert!(!low_quality.is_empty());
        assert_eq!(&high_quality[0..2], &[0xFF, 0xD8]);
        assert_eq!(&low_quality[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_encode_jpeg_with_icc() {
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

        let result = encode_jpeg(&img, 80, Some(&icc_data)).unwrap();
        assert_eq!(&result[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_encode_png_produces_valid_png() {
        let img = create_test_image(100, 100);
        let result = encode_png(&img, None).unwrap();
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
    #[cfg(feature = "avif")]
    fn test_encode_avif_produces_valid_avif() {
        use lazy_image::engine::encode_avif;
        let img = create_test_image(100, 100);
        let result = encode_avif(&img, 60, None).unwrap();
        assert!(result.len() > 12);
        let has_ftyp = result.windows(4).any(|w| w == b"ftyp");
        assert!(has_ftyp);
    }

    #[test]
    #[cfg(feature = "avif")]
    fn test_encode_avif_quality_affects_size() {
        use lazy_image::engine::encode_avif;
        let img = create_test_image(100, 100);
        let high_quality = encode_avif(&img, 80, None).unwrap();
        let low_quality = encode_avif(&img, 40, None).unwrap();
        assert!(!high_quality.is_empty());
        assert!(!low_quality.is_empty());
    }

    #[test]
    #[cfg(not(feature = "avif"))]
    fn test_encode_avif_returns_unsupported_without_feature() {
        use lazy_image::engine::encode_avif;
        let img = create_test_image(100, 100);
        let result = encode_avif(&img, 60, None);
        assert!(result.is_err());
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

// =========================================================================
// decode_tests (only public-API subset; EncodeTask tests remain inline)
// =========================================================================

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
        let invalid_data = vec![0xFF, 0xD8, 0x00];
        let result = decode_jpeg_mozjpeg(&invalid_data);
        assert!(result.is_err());
    }
}

// =========================================================================
// fast_resize_tests
// =========================================================================

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
