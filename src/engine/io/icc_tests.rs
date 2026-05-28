// src/engine/io/icc_tests.rs
//
// Test module body for `io::icc`, referenced via `#[path]` from icc.rs so the
// production module stays under the 800-line ceiling defined in CLAUDE.md.

use super::super::test_helpers::{
    create_minimal_jpeg, create_minimal_png, create_minimal_webp, create_test_image,
};
use super::*;
#[cfg(feature = "avif")]
use crate::engine::encoder::encode_avif;
use crate::engine::encoder::{encode_jpeg, encode_png, encode_webp};

fn extract_icc_ok(data: &[u8]) -> Option<Vec<u8>> {
    extract_icc_profile(data).unwrap()
}

#[test]
fn test_validate_icc_profile_too_small() {
    let data = vec![0u8; 127]; // Less than 128 bytes
    assert!(!validate_icc_profile(&data));
}

#[test]
fn test_validate_icc_profile_minimal_valid() {
    // Minimal valid ICC profile (128 bytes)
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
    let result = JpegIccExtractor.extract_icc(&jpeg_data);
    assert!(result.is_none());
}

#[test]
fn test_extract_icc_from_png_no_profile() {
    // PNG without ICC profile
    let png_data = create_minimal_png();
    let result = PngIccExtractor.extract_icc(&png_data);
    assert!(result.is_none());
}

#[test]
fn test_extract_icc_from_webp_no_profile() {
    // WebP without ICC profile
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
fn test_extract_icc_profile_large_input_skips_icc() {
    // Ensure inputs larger than the fuzz safety cap do not hard-fail.
    let huge = vec![0u8; MAX_ICC_SOURCE_BYTES + 1];
    let result = extract_icc_profile(&huge);
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
        // PNG ICC extraction should succeed for valid iCCP chunks.
        // The direct parser fallback keeps this deterministic across environments.
        let icc = create_minimal_srgb_icc();
        let png = create_png_with_icc(&icc);
        let extracted = extract_icc_ok(&png);
        // PNG ICC extraction should return Some when ICC profile is embedded correctly
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
        // Truncated in the middle
        let truncated = &icc[..50];
        assert!(!validate_icc_profile(truncated));
    }

    #[test]
    fn test_validate_wrong_size_field() {
        let mut icc = create_minimal_srgb_icc();
        // Set size field (first 4 bytes) to invalid value
        icc[0] = 0xFF;
        icc[1] = 0xFF;
        icc[2] = 0xFF;
        icc[3] = 0xFF;
        assert!(!validate_icc_profile(&icc));
    }

    #[test]
    fn test_validate_too_short() {
        assert!(!validate_icc_profile(&[0; 100])); // Less than 128 bytes
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
        // Use direct parsing to assert the iCCP chunk deterministically
        let icc = create_minimal_srgb_icc();
        let png = create_png_with_icc(&icc);

        // Extract ICC from PNG using direct parsing
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

#[cfg(feature = "avif")]
mod avif_icc_tests {
    use super::*;

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
