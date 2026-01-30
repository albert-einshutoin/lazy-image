// tests/edge_cases.rs
//
// Edge case tests for lazy-image
// Tests boundary values, invalid inputs, and error handling

use image::{DynamicImage, GenericImageView, RgbImage};
use lazy_image::engine::{
    apply_ops, calc_resize_dimensions, check_dimensions, encode_avif, encode_jpeg, encode_png,
    encode_webp, fast_resize,
};
use lazy_image::error::LazyImageError;
use std::borrow::Cow;

// Helper function to create test images
fn create_test_image(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    }))
}

// Helper to create valid JPEG of specified size
fn create_valid_jpeg(width: u32, height: u32) -> Vec<u8> {
    let img = create_test_image(width, height);
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let pixels = rgb.into_raw();

    use mozjpeg::ColorSpace;
    use mozjpeg::Compress;

    let mut comp = Compress::new(ColorSpace::JCS_RGB);
    comp.set_size(w as usize, h as usize);
    comp.set_quality(80.0);
    comp.set_color_space(ColorSpace::JCS_YCbCr);
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

mod minimal_image_tests {
    use super::*;
    use lazy_image::ops::{Operation, ResizeFit};

    #[test]
    fn test_1x1_resize() {
        let img = create_test_image(1, 1);
        let ops = vec![Operation::Resize {
            width: Some(100),
            height: Some(100),
            fit: ResizeFit::Inside,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(result.is_ok());
        let resized = result.unwrap();
        assert_eq!(resized.dimensions(), (100, 100));
    }

    #[test]
    fn test_1x1_rotate() {
        let img = create_test_image(1, 1);
        let ops = vec![Operation::Rotate { degrees: 90 }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(result.is_ok());
        // 1x1の回転はサイズが変わらない
        let rotated = result.unwrap();
        assert_eq!(rotated.dimensions(), (1, 1));
    }

    #[test]
    fn test_1x1_grayscale() {
        let img = create_test_image(1, 1);
        let ops = vec![Operation::Grayscale];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(result.is_ok());
    }

    #[test]
    fn test_1x1_encode_jpeg() {
        let img = create_test_image(1, 1);
        let result = encode_jpeg(&img, 80, None);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        // JPEGマジックバイト確認
        assert_eq!(&encoded[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_1x1_encode_png() {
        let img = create_test_image(1, 1);
        let result = encode_png(&img, None);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        // PNGマジックバイト確認
        assert_eq!(
            &encoded[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    #[test]
    fn test_1x1_encode_webp() {
        let img = create_test_image(1, 1);
        let result = encode_webp(&img, 80, None);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert_eq!(&encoded[0..4], b"RIFF");
    }
}

mod large_image_tests {
    use super::*;

    #[test]
    fn test_max_dimension_boundary() {
        // 32768x32768はMAX_PIXELSを超えるのでエラーになる
        // ただし、MAX_DIMENSIONチェックは通る
        let result = check_dimensions(32768, 32768);
        // 32768 * 32768 = 1,073,741,824 > 100,000,000 (MAX_PIXELS)
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds max"));
    }

    #[test]
    fn test_exceed_max_dimension_width() {
        // 32769はNG
        let result = check_dimensions(32769, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
    }

    #[test]
    fn test_exceed_max_dimension_height() {
        // 32769はNG
        let result = check_dimensions(1, 32769);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
    }

    #[test]
    fn test_max_pixels_boundary() {
        // 10000x10000 = 100,000,000 はOK
        let result = check_dimensions(10000, 10000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_exceed_max_pixels() {
        // 10001x10000 = 100,010,000 はNG
        let result = check_dimensions(10001, 10000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds max"));
    }

    #[test]
    fn test_extreme_aspect_ratio_wide() {
        // 32768x1 - MAX_DIMENSION内、MAX_PIXELS内
        let result = check_dimensions(32768, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extreme_aspect_ratio_tall() {
        // 1x32768 - MAX_DIMENSION内、MAX_PIXELS内
        let result = check_dimensions(1, 32768);
        assert!(result.is_ok());
    }

    #[test]
    fn test_calc_resize_extreme_aspect_ratio() {
        // 極端なアスペクト比でのリサイズ計算
        let (w, h) = calc_resize_dimensions(32768, 1, Some(100), None);
        assert_eq!((w, h), (100, 0));
    }
}

mod corrupted_image_tests {
    use super::*;
    use lazy_image::engine::decode_jpeg_mozjpeg;

    #[test]
    fn test_jpeg_header_only() {
        // JPEGマジックバイト（0xFF 0xD8）のみ
        let corrupted = vec![0xFF, 0xD8];
        let result = decode_jpeg_mozjpeg(&corrupted);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("EOI") || err.contains("decode") || err.contains("failed"));
    }

    #[test]
    fn test_truncated_jpeg() {
        // 有効なJPEGを途中で切断
        let valid_jpeg = create_valid_jpeg(100, 100);
        let truncated: Vec<u8> = valid_jpeg[..valid_jpeg.len() / 2].to_vec();

        let result = decode_jpeg_mozjpeg(&truncated);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("EOI"),
            "expected EOI error for truncated JPEG, got: {}",
            err
        );
    }

    #[test]
    fn test_wrong_magic_bytes() {
        // PNG風のマジックバイトだが中身がJPEG
        let mut fake = vec![0x89, 0x50, 0x4E, 0x47]; // PNGマジック
        let valid_jpeg = create_valid_jpeg(10, 10);
        fake.extend_from_slice(&valid_jpeg[4..]);

        // PNGマジックバイトなので、decode_jpeg_mozjpegは呼ばれず、image crateが処理する
        let result = image::load_from_memory(&fake);
        // PNGとして解析を試みるが、実際はJPEGなので失敗する
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_buffer() {
        let empty: Vec<u8> = vec![];
        let result = decode_jpeg_mozjpeg(&empty);
        assert!(result.is_err());
    }
}

mod non_image_tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_text_file() {
        let text = b"Hello, this is not an image!".to_vec();
        // image crateでデコードを試みる
        let result = image::load_from_memory(&text);
        assert!(result.is_err());
    }

    #[test]
    fn test_random_binary() {
        let random: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        // image crateでデコードを試みる
        let result = image::load_from_memory(&random);
        assert!(result.is_err());
    }
}

mod quality_boundary_tests {
    use super::*;

    #[test]
    fn test_quality_0() {
        let img = create_test_image(100, 100);
        let result = encode_jpeg(&img, 0, None);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert_eq!(&encoded[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_quality_1() {
        let img = create_test_image(100, 100);
        let result = encode_jpeg(&img, 1, None);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert_eq!(&encoded[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_quality_100() {
        let img = create_test_image(100, 100);
        let result = encode_jpeg(&img, 100, None);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert_eq!(&encoded[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_quality_over_100() {
        let img = create_test_image(100, 100);
        let result = encode_jpeg(&img, 101, None);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert_eq!(&encoded[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_quality_webp_0() {
        let img = create_test_image(100, 100);
        let result = encode_webp(&img, 0, None);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert_eq!(&encoded[0..4], b"RIFF");
    }

    #[test]
    fn test_quality_webp_100() {
        let img = create_test_image(100, 100);
        let result = encode_webp(&img, 100, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_quality_avif_0() {
        let img = create_test_image(100, 100);
        // AVIFはquality=0も受け入れる（rav1eの実装では品質0も有効）
        // 品質0は最低品質（最大圧縮）を意味する
        let result = encode_avif(&img, 0, None);
        assert!(
            result.is_ok(),
            "AVIF encoding with quality=0 should succeed"
        );
    }

    #[test]
    fn test_quality_avif_100() {
        let img = create_test_image(100, 100);
        let result = encode_avif(&img, 100, None);
        assert!(result.is_ok());
    }
}

mod zero_dimension_tests {
    use super::*;
    use lazy_image::ops::{Operation, ResizeFit};

    #[test]
    fn test_resize_to_zero_width() {
        let img = create_test_image(100, 100);
        let ops = vec![Operation::Resize {
            width: Some(0),
            height: Some(50),
            fit: ResizeFit::Inside,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(matches!(
            result,
            Err(LazyImageError::InvalidResizeDimensions { .. })
        ));
    }

    #[test]
    fn test_resize_to_zero_height() {
        let img = create_test_image(100, 100);
        let ops = vec![Operation::Resize {
            width: Some(50),
            height: Some(0),
            fit: ResizeFit::Inside,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(matches!(
            result,
            Err(LazyImageError::InvalidResizeDimensions { .. })
        ));
    }

    #[test]
    fn test_crop_zero_width() {
        let img = create_test_image(100, 100);
        let ops = vec![Operation::Crop {
            x: 10,
            y: 10,
            width: 0,
            height: 50,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(matches!(
            result,
            Err(LazyImageError::InvalidCropDimensions { .. })
        ));
    }

    #[test]
    fn test_crop_zero_height() {
        let img = create_test_image(100, 100);
        let ops = vec![Operation::Crop {
            x: 10,
            y: 10,
            width: 50,
            height: 0,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(matches!(
            result,
            Err(LazyImageError::InvalidCropDimensions { .. })
        ));
    }

    #[test]
    fn test_fast_resize_zero_dimensions() {
        let img = create_test_image(100, 100);
        let result = fast_resize(&img, 0, 100);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid dimensions"));

        let result = fast_resize(&img, 100, 0);
        assert!(result.is_err());
    }
}

mod extreme_aspect_ratio_tests {
    use super::*;
    use lazy_image::ops::{Operation, ResizeFit};

    #[test]
    fn test_resize_extreme_wide() {
        // 32768x1の画像をリサイズ
        // 注意: 32768x1の画像を作成するとメモリ不足になる可能性がある
        // より小さな極端なアスペクト比でテスト
        let img = create_test_image(1000, 1);
        let ops = vec![Operation::Resize {
            width: Some(100),
            height: None,
            fit: ResizeFit::Inside,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(
            result.is_err(),
            "Expect resize to report error for extreme aspect ratio producing zero height"
        );
    }

    #[test]
    fn test_resize_extreme_tall() {
        // 1x32768の画像をリサイズ
        // 注意: 1x32768の画像を作成するとメモリ不足になる可能性がある
        // より小さな極端なアスペクト比でテスト
        let img = create_test_image(1, 1000);
        let ops = vec![Operation::Resize {
            width: None,
            height: Some(100),
            fit: ResizeFit::Inside,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(
            result.is_err(),
            "Expect resize to report error for extreme aspect ratio producing zero width"
        );
    }
}

mod decoder_error_tests {
    #[allow(unused_imports)]
    use super::*;
    use lazy_image::engine::Source;
    use std::sync::Arc;

    #[test]
    fn test_source_as_bytes_memory() {
        // Memoryソースのas_bytes()は成功する（zero-copy）
        let data = vec![0xFF, 0xD8, 0x00, 0x01];
        let memory_source = Source::Memory(Arc::new(data.clone()));
        let bytes = memory_source.as_bytes();
        assert!(bytes.is_some());
        assert_eq!(bytes.unwrap(), data.as_slice());
    }

    #[test]
    fn test_source_as_bytes() {
        let data = vec![0xFF, 0xD8];
        let memory_source = Source::Memory(Arc::new(data.clone()));
        assert_eq!(memory_source.as_bytes(), Some(data.as_slice()));
    }

    #[test]
    fn test_source_len() {
        let data = vec![0u8; 100];
        let memory_source = Source::Memory(Arc::new(data));
        assert_eq!(memory_source.len(), 100);
    }
}

mod pipeline_error_tests {
    use super::*;
    use lazy_image::engine::fast_resize_owned;
    use lazy_image::ops::Operation;

    #[test]
    fn test_fast_resize_owned_invalid_dimensions() {
        let img = create_test_image(100, 100);
        // 0幅は無効
        let result = fast_resize_owned(img.clone(), 0, 100);
        assert!(result.is_err());

        // 0高さは無効
        let result = fast_resize_owned(img, 100, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_crop_bounds_exceed_image() {
        let img = create_test_image(100, 100);
        let ops = vec![Operation::Crop {
            x: 50,
            y: 50,
            width: 60, // 50 + 60 = 110 > 100
            height: 50,
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("exceed") || err.to_string().contains("bounds"),
            "Error should mention bounds or exceed"
        );
    }

    #[test]
    fn test_crop_bounds_exceed_height() {
        let img = create_test_image(100, 100);
        let ops = vec![Operation::Crop {
            x: 50,
            y: 50,
            width: 50,
            height: 60, // 50 + 60 = 110 > 100
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_rotation_angle() {
        let img = create_test_image(100, 100);
        let ops = vec![Operation::Rotate { degrees: 45 }]; // 45度は無効
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("rotation") || err.to_string().contains("angle"),
            "Error should mention rotation or angle"
        );
    }

    #[test]
    fn test_negative_rotation_angles() {
        let img = create_test_image(100, 100);
        // -90, -180, -270は有効
        let ops1 = vec![Operation::Rotate { degrees: -90 }];
        assert!(apply_ops(Cow::Owned(img.clone()), &ops1).is_ok());

        let ops2 = vec![Operation::Rotate { degrees: -180 }];
        assert!(apply_ops(Cow::Owned(img.clone()), &ops2).is_ok());

        let ops3 = vec![Operation::Rotate { degrees: -270 }];
        assert!(apply_ops(Cow::Owned(img), &ops3).is_ok());
    }

    #[test]
    fn test_rotation_0_degrees() {
        let img = create_test_image(100, 100);
        let ops = vec![Operation::Rotate { degrees: 0 }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(result.is_ok()); // 0度は有効（no-op）
    }
}

mod encoder_error_tests {
    use super::*;

    #[test]
    fn test_encode_jpeg_invalid_quality() {
        let img = create_test_image(100, 100);
        // quality > 100 はクランプされ、エンコードは成功する
        let result = encode_jpeg(&img, 150, None);
        assert!(result.is_ok(), "encode_jpeg should accept quality > 100");
    }

    #[test]
    fn test_encode_webp_invalid_quality() {
        let img = create_test_image(100, 100);
        // quality > 100 はクランプされ、エンコードは成功する
        let result = encode_webp(&img, 150, None);
        assert!(result.is_ok(), "encode_webp should accept quality > 100");
        let encoded = result.unwrap();
        assert_eq!(&encoded[0..4], b"RIFF");
    }

    #[test]
    fn test_encode_avif_invalid_quality() {
        let img = create_test_image(100, 100);
        // quality > 100 はクランプされ、エンコードは成功する
        let result = encode_avif(&img, 150, None);
        assert!(result.is_ok(), "encode_avif should accept quality > 100");
    }
}
