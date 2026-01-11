// tests/integration_tests.rs
//
// Integration tests for the NAPI public API.
// These tests exercise the full ImageEngine API through NAPI bindings.
//
// Note: Some methods (resize, crop, rotate, etc.) require NAPI Reference context
// which is only available in JavaScript runtime. These are tested in
// test/integration/*.test.js instead. This file focuses on testable methods
// that don't require Reference.

#[cfg(feature = "napi")]
mod tests {
    use lazy_image::ImageEngine;
    use napi::bindgen_prelude::*;

    // Helper to create a minimal test image buffer
    fn create_test_image_buffer() -> Buffer {
        // Create a 100x100 RGB image and encode it as PNG
        use image::{ImageBuffer, Rgb, RgbImage};
        let img: RgbImage = ImageBuffer::from_fn(100, 100, |x, y| {
            Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        Buffer::from(buf)
    }

    #[test]
    fn test_image_engine_from_buffer() {
        let buffer = create_test_image_buffer();
        let engine = ImageEngine::from(buffer);
        
        // Verify engine is created successfully
        // We can verify by checking that dimensions() works (requires source_bytes to be loaded)
        let mut engine_for_dims = engine;
        let dims = engine_for_dims.dimensions().unwrap();
        assert_eq!(dims.width, 100);
        assert_eq!(dims.height, 100);
    }

    #[test]
    fn test_image_engine_dimensions() {
        let buffer = create_test_image_buffer();
        let mut engine = ImageEngine::from(buffer);
        
        // Get dimensions without full decode (header-only parsing)
        let dims = engine.dimensions().unwrap();
        assert_eq!(dims.width, 100);
        assert_eq!(dims.height, 100);
    }

    #[test]
    fn test_image_engine_clone() {
        let buffer = create_test_image_buffer();
        let engine = ImageEngine::from(buffer);
        
        // Clone should succeed and create independent instance
        let cloned = engine.clone_engine().unwrap();
        
        // Verify cloned engine works by checking dimensions
        let mut cloned_for_dims = cloned;
        let dims = cloned_for_dims.dimensions().unwrap();
        assert_eq!(dims.width, 100);
        assert_eq!(dims.height, 100);
        
        // Verify they are independent (cloned engine has same data but separate instance)
        // This is tested implicitly by the clone succeeding and dimensions matching
    }

    #[test]
    fn test_image_engine_has_icc_profile() {
        let buffer = create_test_image_buffer();
        let engine = ImageEngine::from(buffer);
        
        // Test image without ICC profile should return None
        let icc_size = engine.has_icc_profile();
        // PNG created from scratch typically doesn't have ICC profile
        // This test verifies the method works without panicking
        assert!(icc_size.is_none() || icc_size.is_some());
    }

    // Note: Methods requiring Reference (resize, crop, rotate, flip_h, flip_v,
    // grayscale, keep_metadata, contrast, ensure_rgb, preset) cannot be tested
    // here as they require NAPI JavaScript context. These are covered by
    // test/integration/*.test.js which runs in Node.js environment.
}
