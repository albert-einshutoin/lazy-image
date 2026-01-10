// tests/napi_tests.rs
//
// NAPI機能を有効化した状態でのテスト
// このテストは`cargo test --features napi`で実行される
//
// 注意: このテストファイルはNAPI機能が有効化されている場合のみコンパイルされる
//
// CIでの実行について:
// - このテストはNode.jsランタイムが必要なため、通常のcargo testでは実行できません
// - NAPI機能はJavaScript統合テスト（test/integration/*.test.js）でカバーされています
// - ローカル開発環境でNode.jsがインストールされている場合のみ実行可能です

#[cfg(feature = "napi")]
mod napi_feature_tests {
    use lazy_image::engine::ImageEngine;
    use lazy_image::inspect_header_from_bytes;

    // テスト用の最小JPEGデータを作成
    fn create_minimal_jpeg() -> Vec<u8> {
        use image::RgbImage;
        use mozjpeg::ColorSpace;
        use mozjpeg::Compress;

        let img = RgbImage::from_fn(10, 10, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });

        let mut comp = Compress::new(ColorSpace::JCS_RGB);
        comp.set_size(10, 10);
        comp.set_quality(80.0);
        comp.set_color_space(ColorSpace::JCS_YCbCr);
        comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));

        let mut output = Vec::new();
        {
            let mut writer = comp.start_compress(&mut output).unwrap();
            let stride = 10 * 3;
            for row in img.as_raw().chunks(stride) {
                writer.write_scanlines(row).unwrap();
            }
            writer.finish().unwrap();
        }
        output
    }

    #[test]
    fn test_image_engine_from_buffer() {
        let jpeg_data = create_minimal_jpeg();
        let _engine = ImageEngine::from(napi::bindgen_prelude::Buffer::from(jpeg_data));
        // エンジンが正常に作成されることを確認
        // 実際の処理は非同期なので、ここでは作成のみをテスト
    }

    #[test]
    fn test_inspect_header_from_bytes() {
        let jpeg_data = create_minimal_jpeg();
        let result = inspect_header_from_bytes(&jpeg_data);
        assert!(result.is_ok());
        let metadata = result.unwrap();
        assert_eq!(metadata.width, 10);
        assert_eq!(metadata.height, 10);
        assert!(metadata.format.is_some());
    }

    #[test]
    fn test_inspect_header_invalid_data() {
        let invalid_data = vec![0u8; 10];
        let result = inspect_header_from_bytes(&invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_image_engine_clone() {
        let jpeg_data = create_minimal_jpeg();
        let engine = ImageEngine::from(napi::bindgen_prelude::Buffer::from(jpeg_data.clone()));
        let cloned = engine.clone_engine();
        assert!(cloned.is_ok());
    }
}
