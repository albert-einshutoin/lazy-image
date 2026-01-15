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

    // Helper to create a dummy Env for exercising NAPI entry points in Rust tests.
    // The Env value is only used when an error occurs, so we can safely pass a null
    // pointer for success-path tests. This keeps coverage on the `dimensions` API
    // without requiring a real Node.js runtime in cargo test.
    fn dummy_env() -> Env {
        unsafe { Env::from_raw(std::ptr::null_mut()) }
    }

    #[test]
    fn test_image_engine_from_buffer() {
        let buffer = create_test_image_buffer();
        let engine = ImageEngine::from(buffer);

        // Verify engine is created successfully
        // Test through NAPI public API to ensure Env-based error handling works
        let mut engine_for_dims = engine;
        let env = dummy_env();
        let dims = engine_for_dims.dimensions(env).unwrap();
        assert_eq!(dims.width, 100);
        assert_eq!(dims.height, 100);
    }

    #[test]
    fn test_image_engine_dimensions() {
        let buffer = create_test_image_buffer();
        let mut engine = ImageEngine::from(buffer);

        // Get dimensions without full decode (header-only parsing)
        // Test through NAPI public API to ensure Env-based error handling works
        let env = dummy_env();
        let dims = engine.dimensions(env).unwrap();
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
        // Test through NAPI public API to ensure Env-based error handling works
        let mut cloned_for_dims = cloned;
        let env = dummy_env();
        let dims = cloned_for_dims.dimensions(env).unwrap();
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
        assert!(
            icc_size.is_none(),
            "PNG created from scratch should not have ICC profile"
        );
    }

    // Note: Methods requiring Reference (resize, crop, rotate, flip_h, flip_v,
    // grayscale, keep_metadata, contrast, ensure_rgb, preset) cannot be tested
    // here as they require NAPI JavaScript context. These are covered by
    // test/integration/*.test.js which runs in Node.js environment.
}
