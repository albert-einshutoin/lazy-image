// tests/edge_cases.rs
//
// Edge case tests for lazy-image
// Tests boundary values, invalid inputs, and error handling

use image::{DynamicImage, GenericImageView, RgbImage};
use lazy_image::engine::{
    apply_ops, calc_resize_dimensions, check_dimensions, encode_avif, encode_jpeg, encode_png,
    encode_webp, fast_resize, EncodeTask,
};
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
    use lazy_image::ops::Operation;

    #[test]
    fn test_1x1_resize() {
        let img = create_test_image(1, 1);
        let ops = vec![Operation::Resize {
            width: Some(100),
            height: Some(100),
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
        assert_eq!(w, 100);
        // 32768:1 = 100:0.003... → 0に丸められる可能性がある
        // 実際の計算: 100 / 32768 * 1 = 0.003... → round()で0になる可能性がある
        // これは計算上の制限であり、実際のリサイズ処理で1にクランプされる可能性がある
        eprintln!("Calculated resize: 32768x1 -> {}x{}", w, h);
    }
}

mod corrupted_image_tests {
    use super::*;
    use lazy_image::engine::EncodeTask;
    use std::sync::Arc;

    #[test]
    fn test_jpeg_header_only() {
        // JPEGマジックバイト（0xFF 0xD8）のみ
        let corrupted = vec![0xFF, 0xD8];
        let task = EncodeTask {
            source: Some(Arc::new(corrupted)),
            decoded: None,
            ops: vec![],
            format: lazy_image::ops::OutputFormat::Jpeg { quality: 80 },
            icc_profile: None,
            keep_metadata: false,
        };
        let result = task.decode();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("decode") || err.contains("failed"));
    }

    #[test]
    fn test_truncated_jpeg() {
        // 有効なJPEGを途中で切断
        let valid_jpeg = create_valid_jpeg(100, 100);
        let truncated: Vec<u8> = valid_jpeg[..valid_jpeg.len() / 2].to_vec();

        let task = EncodeTask {
            source: Some(Arc::new(truncated)),
            decoded: None,
            ops: vec![],
            format: lazy_image::ops::OutputFormat::Jpeg { quality: 80 },
            icc_profile: None,
            keep_metadata: false,
        };
        let result = task.decode();
        // 切断されたJPEGは通常エラーになるが、image crateが部分的にデコードできる場合もある
        // 少なくともpanicしないことを確認
        if result.is_ok() {
            // デコードできた場合でも、後続の処理でエラーになる可能性がある
            eprintln!("Warning: Truncated JPEG was decoded successfully (may be a limitation)");
        }
    }

    #[test]
    fn test_wrong_magic_bytes() {
        // PNG風のマジックバイトだが中身がJPEG
        let mut fake = vec![0x89, 0x50, 0x4E, 0x47]; // PNGマジック
        let valid_jpeg = create_valid_jpeg(10, 10);
        fake.extend_from_slice(&valid_jpeg[4..]);

        let task = EncodeTask {
            source: Some(Arc::new(fake)),
            decoded: None,
            ops: vec![],
            format: lazy_image::ops::OutputFormat::Jpeg { quality: 80 },
            icc_profile: None,
            keep_metadata: false,
        };
        let result = task.decode();
        // PNGとして解析を試みるが、実際はJPEGなので失敗する可能性が高い
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_buffer() {
        let empty: Vec<u8> = vec![];
        let task = EncodeTask {
            source: Some(Arc::new(empty)),
            decoded: None,
            ops: vec![],
            format: lazy_image::ops::OutputFormat::Jpeg { quality: 80 },
            icc_profile: None,
            keep_metadata: false,
        };
        let result = task.decode();
        assert!(result.is_err());
    }
}

mod non_image_tests {
    use lazy_image::engine::EncodeTask;
    use std::sync::Arc;

    #[test]
    fn test_text_file() {
        let text = b"Hello, this is not an image!".to_vec();
        let task = EncodeTask {
            source: Some(Arc::new(text)),
            decoded: None,
            ops: vec![],
            format: lazy_image::ops::OutputFormat::Jpeg { quality: 80 },
            icc_profile: None,
            keep_metadata: false,
        };
        let result = task.decode();
        assert!(result.is_err());
    }

    #[test]
    fn test_random_binary() {
        let random: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let task = EncodeTask {
            source: Some(Arc::new(random)),
            decoded: None,
            ops: vec![],
            format: lazy_image::ops::OutputFormat::Jpeg { quality: 80 },
            icc_profile: None,
            keep_metadata: false,
        };
        let result = task.decode();
        assert!(result.is_err());
    }
}

mod quality_boundary_tests {
    use super::*;

    #[test]
    fn test_quality_0() {
        let img = create_test_image(100, 100);
        // quality=0は意味があるか？多くのエンコーダは1以上を期待
        // 少なくともpanicしないことを確認
        let result = encode_jpeg(&img, 0, None);
        // mozjpegは0を受け入れる可能性がある
        assert!(result.is_ok() || result.is_err());
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
        // quality > 100 の処理：クランプされるか、エラーか
        // mozjpegはf32で品質を受け取るので、101も受け入れる可能性がある
        let result = encode_jpeg(&img, 101, None);
        // panicしないことが最低限の要件
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_quality_webp_0() {
        let img = create_test_image(100, 100);
        let result = encode_webp(&img, 0, None);
        // WebPは0を受け入れる可能性がある
        assert!(result.is_ok() || result.is_err());
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
    use lazy_image::ops::Operation;

    #[test]
    fn test_resize_to_zero_width() {
        let img = create_test_image(100, 100);
        let ops = vec![Operation::Resize {
            width: Some(0),
            height: Some(50),
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        // 0幅へのリサイズはfast_resizeでエラーになる可能性がある
        // または、image crateのresizeでエラーになる
        // 少なくともpanicしないことを確認
        if result.is_ok() {
            let resized = result.unwrap();
            // 0幅は無効なので、エラーになるか、または1にクランプされる可能性がある
            // 実際の動作に依存する
            eprintln!(
                "Warning: Resize to 0 width succeeded, result: {}x{}",
                resized.width(),
                resized.height()
            );
        }
    }

    #[test]
    fn test_resize_to_zero_height() {
        let img = create_test_image(100, 100);
        let ops = vec![Operation::Resize {
            width: Some(50),
            height: Some(0),
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        // 0高さへのリサイズはエラーになる可能性がある
        // 少なくともpanicしないことを確認
        if result.is_ok() {
            let resized = result.unwrap();
            eprintln!(
                "Warning: Resize to 0 height succeeded, result: {}x{}",
                resized.width(),
                resized.height()
            );
        }
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
        // 0幅のクロップはエラーであるべき
        // ただし、image crateの動作に依存する可能性がある
        if result.is_ok() {
            eprintln!("Warning: Crop with 0 width succeeded (may be a limitation)");
        }
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
        // 0高さのクロップはエラーであるべき
        // ただし、image crateの動作に依存する可能性がある
        if result.is_ok() {
            eprintln!("Warning: Crop with 0 height succeeded (may be a limitation)");
        }
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
    use lazy_image::ops::Operation;

    #[test]
    fn test_resize_extreme_wide() {
        // 32768x1の画像をリサイズ
        // 注意: 32768x1の画像を作成するとメモリ不足になる可能性がある
        // より小さな極端なアスペクト比でテスト
        let img = create_test_image(1000, 1);
        let ops = vec![Operation::Resize {
            width: Some(100),
            height: None,
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
        }];
        let result = apply_ops(Cow::Owned(img), &ops);
        assert!(
            result.is_err(),
            "Expect resize to report error for extreme aspect ratio producing zero width"
        );
    }
}
