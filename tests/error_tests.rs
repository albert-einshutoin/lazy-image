//! Tests for the error module — extracted from src/error.rs inline tests.
//!
//! These tests cover ErrorCode, ErrorCategory, LazyImageError constructors,
//! recovery hints, clone behavior, and repr values.

use lazy_image::error::*;
use std::sync::Arc;

#[test]
fn test_error_display() {
    let err = LazyImageError::file_not_found("/path/to/file.jpg");
    assert!(err.to_string().contains("/path/to/file.jpg"));
}

#[test]
fn test_error_recoverable() {
    assert!(LazyImageError::file_not_found("test.jpg").is_recoverable());
    assert!(LazyImageError::invalid_crop_bounds(0, 0, 100, 100, 50, 50).is_recoverable());
    assert!(LazyImageError::invalid_crop_dimensions(0, 100).is_recoverable());
    assert!(!LazyImageError::decode_failed("test").is_recoverable());
    assert!(!LazyImageError::internal_panic("test").is_recoverable());
}

#[test]
fn test_all_error_constructors() {
    let _ = LazyImageError::file_not_found("test.jpg");
    let _ = LazyImageError::file_read_failed(
        "test.jpg",
        std::io::Error::from(std::io::ErrorKind::NotFound),
    );
    let _ = LazyImageError::file_write_failed(
        "test.jpg",
        std::io::Error::from(std::io::ErrorKind::PermissionDenied),
    );
    let _ = LazyImageError::unsupported_format("gif");
    let _ = LazyImageError::decode_failed("test");
    let _ = LazyImageError::corrupted_image();
    let _ = LazyImageError::dimension_exceeds_limit(10000, 8000);
    let _ = LazyImageError::pixel_count_exceeds_limit(1000000000, 100000000);
    let _ = LazyImageError::invalid_crop_bounds(100, 100, 500, 500, 200, 200);
    let _ = LazyImageError::invalid_crop_dimensions(0, 0);
    let _ = LazyImageError::invalid_rotation_angle(45);
    let _ = LazyImageError::invalid_resize_dimensions(None, None);
    let _ = LazyImageError::resize_failed((100, 100), (50, 50), "test");
    let _ = LazyImageError::unsupported_color_space("CMYK");
    let _ = LazyImageError::encode_failed("jpeg", "test");
    let _ = LazyImageError::invalid_preset("unknown");
    let _ = LazyImageError::invalid_argument("width", "0", "must be positive");
    let _ = LazyImageError::source_consumed();
    let _ = LazyImageError::internal_panic("test");
    let _ = LazyImageError::generic("test");
}

#[test]
fn test_error_category_user_error() {
    assert_eq!(
        LazyImageError::file_not_found("test.jpg").category(),
        ErrorCategory::UserError
    );
    assert_eq!(
        LazyImageError::invalid_crop_bounds(0, 0, 100, 100, 50, 50).category(),
        ErrorCategory::UserError
    );
    assert_eq!(
        LazyImageError::invalid_crop_dimensions(0, 100).category(),
        ErrorCategory::UserError
    );
    assert_eq!(
        LazyImageError::invalid_rotation_angle(45).category(),
        ErrorCategory::UserError
    );
    assert_eq!(
        LazyImageError::invalid_resize_dimensions(None, None).category(),
        ErrorCategory::UserError
    );
    assert_eq!(
        LazyImageError::invalid_preset("unknown").category(),
        ErrorCategory::UserError
    );
    assert_eq!(
        LazyImageError::source_consumed().category(),
        ErrorCategory::UserError
    );
}

#[test]
fn test_error_category_codec_error() {
    assert_eq!(
        LazyImageError::unsupported_format("gif").category(),
        ErrorCategory::CodecError
    );
    assert_eq!(
        LazyImageError::decode_failed("test").category(),
        ErrorCategory::CodecError
    );
    assert_eq!(
        LazyImageError::corrupted_image().category(),
        ErrorCategory::CodecError
    );
    assert_eq!(
        LazyImageError::encode_failed("jpeg", "test").category(),
        ErrorCategory::CodecError
    );
    assert_eq!(
        LazyImageError::unsupported_color_space("CMYK").category(),
        ErrorCategory::CodecError
    );
    assert_eq!(
        LazyImageError::resize_failed((100, 100), (50, 50), "test").category(),
        ErrorCategory::CodecError
    );
}

#[test]
fn test_error_category_resource_limit() {
    assert_eq!(
        LazyImageError::dimension_exceeds_limit(10000, 8000).category(),
        ErrorCategory::ResourceLimit
    );
    assert_eq!(
        LazyImageError::pixel_count_exceeds_limit(1000000000, 100000000).category(),
        ErrorCategory::ResourceLimit
    );
    assert_eq!(
        LazyImageError::file_read_failed(
            "test.jpg",
            std::io::Error::from(std::io::ErrorKind::NotFound)
        )
        .category(),
        ErrorCategory::ResourceLimit
    );
    assert_eq!(
        LazyImageError::mmap_failed(
            "test.jpg",
            std::io::Error::from(std::io::ErrorKind::NotFound)
        )
        .category(),
        ErrorCategory::ResourceLimit
    );
    assert_eq!(
        LazyImageError::file_write_failed(
            "test.jpg",
            std::io::Error::from(std::io::ErrorKind::PermissionDenied)
        )
        .category(),
        ErrorCategory::ResourceLimit
    );
}

#[test]
fn test_error_category_internal_bug() {
    assert_eq!(
        LazyImageError::internal_panic("test").category(),
        ErrorCategory::InternalBug
    );
    assert_eq!(
        LazyImageError::generic("test").category(),
        ErrorCategory::InternalBug
    );
}

#[test]
fn test_error_code_mapping() {
    assert_eq!(
        LazyImageError::file_not_found("x").code(),
        ErrorCode::FileNotFound
    );
    assert_eq!(
        LazyImageError::invalid_resize_fit("foo").code(),
        ErrorCode::InvalidResizeFit
    );
    assert_eq!(
        LazyImageError::encode_failed("jpeg", "oops").code(),
        ErrorCode::EncodeFailed
    );
}

#[test]
fn test_recovery_hint_present() {
    let err = LazyImageError::invalid_rotation_angle(45);
    let hint = err.recovery_hint();
    assert!(!hint.is_empty());
    assert!(
        hint.contains("90"),
        "recovery hint should mention allowed angles"
    );
}

#[test]
fn test_clone_preserves_io_error_chain() {
    use std::error::Error as StdError;

    // Build an io::Error with a chained source
    let inner = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "inner cause");
    let outer = std::io::Error::new(inner.kind(), inner);

    let err = LazyImageError::file_read_failed("/tmp/test.jpg", outer);
    let cloned = err.clone();

    // Display messages must be identical
    assert_eq!(err.to_string(), cloned.to_string());

    // The source() chain must survive cloning
    let orig_source = err.source().expect("original should have a source");
    let clone_source = cloned.source().expect("clone should have a source");
    assert_eq!(orig_source.to_string(), clone_source.to_string());

    // The inner source of the io::Error should also survive
    let orig_inner = orig_source.source();
    let clone_inner = clone_source.source();
    assert_eq!(orig_inner.is_some(), clone_inner.is_some());
    if let (Some(a), Some(b)) = (orig_inner, clone_inner) {
        assert_eq!(a.to_string(), b.to_string());
    }
}

#[test]
fn test_clone_preserves_io_error_kind_for_all_io_variants() {
    let kinds = [
        std::io::ErrorKind::NotFound,
        std::io::ErrorKind::PermissionDenied,
        std::io::ErrorKind::Other,
    ];

    for kind in &kinds {
        let io_err = std::io::Error::new(*kind, "test message");
        let err = LazyImageError::file_read_failed("a.jpg", io_err);
        let cloned = err.clone();
        // Verify display is identical (error kind + message preserved)
        assert_eq!(err.to_string(), cloned.to_string());

        let io_err = std::io::Error::new(*kind, "test message");
        let err = LazyImageError::mmap_failed("b.jpg", io_err);
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());

        let io_err = std::io::Error::new(*kind, "test message");
        let err = LazyImageError::file_write_failed("c.jpg", io_err);
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }
}

#[test]
fn test_clone_shares_arc_identity() {
    let err = LazyImageError::file_read_failed(
        "test.jpg",
        std::io::Error::new(std::io::ErrorKind::NotFound, "gone"),
    );
    let cloned = err.clone();

    // Both should point to the same Arc allocation
    if let (
        LazyImageError::FileReadFailed { source: ref s1, .. },
        LazyImageError::FileReadFailed { source: ref s2, .. },
    ) = (&err, &cloned)
    {
        assert!(Arc::ptr_eq(s1, s2), "clone should share the same Arc");
    } else {
        panic!("expected FileReadFailed variants");
    }
}

#[cfg(feature = "napi")]
#[test]
fn test_error_category_as_str() {
    assert_eq!(ErrorCategory::UserError.as_str(), "UserError");
    assert_eq!(ErrorCategory::CodecError.as_str(), "CodecError");
    assert_eq!(ErrorCategory::ResourceLimit.as_str(), "ResourceLimit");
    assert_eq!(ErrorCategory::InternalBug.as_str(), "InternalBug");
}

// ---------------------------------------------------------------
// Comprehensive per-variant tests: code, as_str, category,
// is_recoverable, recovery_hint, Display
// ---------------------------------------------------------------

/// Helper to run all standard assertions for an error variant.
fn assert_error_variant(
    err: &LazyImageError,
    expected_code: ErrorCode,
    expected_str: &str,
    expected_category: ErrorCategory,
    expected_recoverable: bool,
    display_contains: &[&str],
) {
    assert_eq!(err.code(), expected_code);
    assert_eq!(err.code().as_str(), expected_str);
    assert_eq!(err.code().category(), expected_category);
    assert_eq!(err.category(), expected_category);
    assert_eq!(err.code().is_recoverable(), expected_recoverable);
    assert_eq!(err.is_recoverable(), expected_recoverable);
    let hint = err.recovery_hint();
    assert!(
        !hint.is_empty(),
        "recovery_hint must be non-empty for {expected_str}"
    );
    let display = err.to_string();
    for substr in display_contains {
        assert!(
            display.contains(substr),
            "Display of {expected_str} should contain '{substr}', got: {display}"
        );
    }
}

// --- E1xx Input Errors ---

#[test]
fn test_e100_file_not_found() {
    let err = LazyImageError::file_not_found("/missing/file.jpg");
    assert_error_variant(
        &err,
        ErrorCode::FileNotFound,
        "E100",
        ErrorCategory::UserError,
        true,
        &["File not found", "/missing/file.jpg"],
    );
}

#[test]
fn test_e101_file_read_failed() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let err = LazyImageError::file_read_failed("/locked/file.jpg", io_err);
    assert_error_variant(
        &err,
        ErrorCode::FileReadFailed,
        "E101",
        ErrorCategory::ResourceLimit,
        true,
        &["Failed to read file", "/locked/file.jpg"],
    );
}

#[test]
fn test_e102_mmap_failed() {
    let io_err = std::io::Error::other("mmap error");
    let err = LazyImageError::mmap_failed("/big/file.jpg", io_err);
    assert_error_variant(
        &err,
        ErrorCode::MmapFailed,
        "E102",
        ErrorCategory::ResourceLimit,
        true,
        &["memory-map", "/big/file.jpg"],
    );
}

#[test]
fn test_e111_unsupported_format() {
    let err = LazyImageError::unsupported_format("gif");
    assert_error_variant(
        &err,
        ErrorCode::UnsupportedFormat,
        "E111",
        ErrorCategory::CodecError,
        false,
        &["Unsupported image format", "gif"],
    );
}

#[test]
fn test_e121_dimension_exceeds_limit() {
    let err = LazyImageError::dimension_exceeds_limit(50000, 16384);
    assert_error_variant(
        &err,
        ErrorCode::DimensionExceedsLimit,
        "E121",
        ErrorCategory::ResourceLimit,
        true,
        &["dimension", "50000", "16384"],
    );
}

#[test]
fn test_e122_pixel_count_exceeds_limit() {
    let err = LazyImageError::pixel_count_exceeds_limit(500_000_000, 100_000_000);
    assert_error_variant(
        &err,
        ErrorCode::PixelCountExceedsLimit,
        "E122",
        ErrorCategory::ResourceLimit,
        true,
        &["pixel count", "500000000", "100000000"],
    );
}

#[test]
fn test_e123_firewall_violation() {
    let err = LazyImageError::firewall_violation("file exceeds byte limit");
    assert_error_variant(
        &err,
        ErrorCode::FirewallViolation,
        "E123",
        ErrorCategory::ResourceLimit,
        true,
        &["Firewall", "file exceeds byte limit"],
    );
}

#[test]
fn test_e130_corrupted_image() {
    let err = LazyImageError::corrupted_image();
    assert_error_variant(
        &err,
        ErrorCode::CorruptedImage,
        "E130",
        ErrorCategory::CodecError,
        false,
        &["Corrupted image data"],
    );
}

#[test]
fn test_e131_decode_failed() {
    let err = LazyImageError::decode_failed("invalid JPEG marker");
    assert_error_variant(
        &err,
        ErrorCode::DecodeFailed,
        "E131",
        ErrorCategory::CodecError,
        false,
        &["Failed to decode", "invalid JPEG marker"],
    );
}

// --- E2xx Processing Errors ---

#[test]
fn test_e200_invalid_crop_bounds() {
    let err = LazyImageError::invalid_crop_bounds(10, 20, 500, 500, 100, 100);
    assert_error_variant(
        &err,
        ErrorCode::InvalidCropBounds,
        "E200",
        ErrorCategory::UserError,
        true,
        &["Crop bounds", "100x100"],
    );
}

#[test]
fn test_e201_invalid_crop_dimensions() {
    let err = LazyImageError::invalid_crop_dimensions(0, 0);
    assert_error_variant(
        &err,
        ErrorCode::InvalidCropDimensions,
        "E201",
        ErrorCategory::UserError,
        true,
        &["Invalid crop dimensions", "width=0", "height=0"],
    );
}

#[test]
fn test_e202_invalid_rotation_angle() {
    let err = LazyImageError::invalid_rotation_angle(45);
    assert_error_variant(
        &err,
        ErrorCode::InvalidRotationAngle,
        "E202",
        ErrorCategory::UserError,
        true,
        &["rotation angle", "45"],
    );
}

#[test]
fn test_e203_invalid_resize_dimensions() {
    let err = LazyImageError::invalid_resize_dimensions(None, None);
    assert_error_variant(
        &err,
        ErrorCode::InvalidResizeDimensions,
        "E203",
        ErrorCategory::UserError,
        true,
        &["Invalid resize dimensions"],
    );
}

#[test]
fn test_e204_invalid_resize_fit() {
    let err = LazyImageError::invalid_resize_fit("stretch");
    assert_error_variant(
        &err,
        ErrorCode::InvalidResizeFit,
        "E204",
        ErrorCategory::UserError,
        true,
        &["Invalid resize fit", "stretch"],
    );
}

#[test]
fn test_e210_unsupported_color_space() {
    let err = LazyImageError::unsupported_color_space("CMYK");
    assert_error_variant(
        &err,
        ErrorCode::UnsupportedColorSpace,
        "E210",
        ErrorCategory::CodecError,
        false,
        &["Unsupported color space", "CMYK"],
    );
}

#[test]
fn test_e299_resize_failed() {
    let err = LazyImageError::resize_failed((1000, 800), (50, 40), "fir error");
    assert_error_variant(
        &err,
        ErrorCode::ResizeFailed,
        "E299",
        ErrorCategory::CodecError,
        false,
        &["Resize failed", "1000x800", "50x40", "fir error"],
    );
}

// --- E3xx Output Errors ---

#[test]
fn test_e300_encode_failed() {
    let err = LazyImageError::encode_failed("avif", "encoder crashed");
    assert_error_variant(
        &err,
        ErrorCode::EncodeFailed,
        "E300",
        ErrorCategory::CodecError,
        false,
        &["Failed to encode", "avif", "encoder crashed"],
    );
}

#[test]
fn test_e301_file_write_failed() {
    let io_err = std::io::Error::other("disk full");
    let err = LazyImageError::file_write_failed("/out/image.webp", io_err);
    assert_error_variant(
        &err,
        ErrorCode::FileWriteFailed,
        "E301",
        ErrorCategory::ResourceLimit,
        true,
        &["Failed to write file", "/out/image.webp"],
    );
}

// --- E4xx Configuration Errors ---

#[test]
fn test_e400_invalid_argument() {
    let err = LazyImageError::invalid_argument("quality", "200", "must be 1-100");
    assert_error_variant(
        &err,
        ErrorCode::InvalidArgument,
        "E400",
        ErrorCategory::UserError,
        true,
        &["Invalid value", "quality", "200", "must be 1-100"],
    );
}

#[test]
fn test_e401_invalid_preset() {
    let err = LazyImageError::invalid_preset("gigantic");
    assert_error_variant(
        &err,
        ErrorCode::InvalidPreset,
        "E401",
        ErrorCategory::UserError,
        true,
        &["Unknown preset", "gigantic"],
    );
}

#[test]
fn test_e402_invalid_firewall_policy() {
    let err = LazyImageError::invalid_firewall_policy("paranoid");
    assert_error_variant(
        &err,
        ErrorCode::InvalidFirewallPolicy,
        "E402",
        ErrorCategory::UserError,
        true,
        &["Unknown firewall policy", "paranoid"],
    );
}

// --- E9xx Internal Errors ---

#[test]
fn test_e900_source_consumed() {
    let err = LazyImageError::source_consumed();
    assert_error_variant(
        &err,
        ErrorCode::SourceConsumed,
        "E900",
        ErrorCategory::UserError,
        true,
        &["source already consumed"],
    );
}

#[test]
fn test_e901_internal_panic() {
    let err = LazyImageError::internal_panic("segfault in codec");
    assert_error_variant(
        &err,
        ErrorCode::InternalPanic,
        "E901",
        ErrorCategory::InternalBug,
        false,
        &["Internal error", "segfault in codec"],
    );
}

#[test]
fn test_e999_generic() {
    let err = LazyImageError::generic("something unexpected");
    assert_error_variant(
        &err,
        ErrorCode::Generic,
        "E999",
        ErrorCategory::InternalBug,
        false,
        &["something unexpected"],
    );
}

// --- ErrorCode enum-level exhaustive tests ---

#[test]
fn test_error_code_as_str_exhaustive() {
    // Verify every variant has the correct E-code string
    let cases: Vec<(ErrorCode, &str)> = vec![
        (ErrorCode::FileNotFound, "E100"),
        (ErrorCode::FileReadFailed, "E101"),
        (ErrorCode::MmapFailed, "E102"),
        (ErrorCode::UnsupportedFormat, "E111"),
        (ErrorCode::DimensionExceedsLimit, "E121"),
        (ErrorCode::PixelCountExceedsLimit, "E122"),
        (ErrorCode::FirewallViolation, "E123"),
        (ErrorCode::CorruptedImage, "E130"),
        (ErrorCode::DecodeFailed, "E131"),
        (ErrorCode::InvalidCropBounds, "E200"),
        (ErrorCode::InvalidCropDimensions, "E201"),
        (ErrorCode::InvalidRotationAngle, "E202"),
        (ErrorCode::InvalidResizeDimensions, "E203"),
        (ErrorCode::InvalidResizeFit, "E204"),
        (ErrorCode::UnsupportedColorSpace, "E210"),
        (ErrorCode::ResizeFailed, "E299"),
        (ErrorCode::EncodeFailed, "E300"),
        (ErrorCode::FileWriteFailed, "E301"),
        (ErrorCode::InvalidArgument, "E400"),
        (ErrorCode::InvalidPreset, "E401"),
        (ErrorCode::InvalidFirewallPolicy, "E402"),
        (ErrorCode::SourceConsumed, "E900"),
        (ErrorCode::InternalPanic, "E901"),
        (ErrorCode::Generic, "E999"),
    ];
    for (code, expected) in &cases {
        assert_eq!(
            code.as_str(),
            *expected,
            "ErrorCode::{code:?} should map to {expected}"
        );
    }
}

#[test]
fn test_error_code_category_exhaustive() {
    use ErrorCategory::*;
    let cases: Vec<(ErrorCode, ErrorCategory)> = vec![
        (ErrorCode::FileNotFound, UserError),
        (ErrorCode::FileReadFailed, ResourceLimit),
        (ErrorCode::MmapFailed, ResourceLimit),
        (ErrorCode::UnsupportedFormat, CodecError),
        (ErrorCode::CorruptedImage, CodecError),
        (ErrorCode::DecodeFailed, CodecError),
        (ErrorCode::DimensionExceedsLimit, ResourceLimit),
        (ErrorCode::PixelCountExceedsLimit, ResourceLimit),
        (ErrorCode::FirewallViolation, ResourceLimit),
        (ErrorCode::InvalidCropBounds, UserError),
        (ErrorCode::InvalidCropDimensions, UserError),
        (ErrorCode::InvalidRotationAngle, UserError),
        (ErrorCode::InvalidResizeDimensions, UserError),
        (ErrorCode::InvalidResizeFit, UserError),
        (ErrorCode::UnsupportedColorSpace, CodecError),
        (ErrorCode::ResizeFailed, CodecError),
        (ErrorCode::EncodeFailed, CodecError),
        (ErrorCode::FileWriteFailed, ResourceLimit),
        (ErrorCode::InvalidArgument, UserError),
        (ErrorCode::InvalidPreset, UserError),
        (ErrorCode::InvalidFirewallPolicy, UserError),
        (ErrorCode::SourceConsumed, UserError),
        (ErrorCode::InternalPanic, InternalBug),
        (ErrorCode::Generic, InternalBug),
    ];
    for (code, expected_cat) in &cases {
        assert_eq!(
            code.category(),
            *expected_cat,
            "ErrorCode::{code:?} should have category {expected_cat:?}"
        );
    }
}

#[test]
fn test_error_code_is_recoverable_exhaustive() {
    let recoverable_codes = vec![
        ErrorCode::FileNotFound,
        ErrorCode::FileReadFailed,
        ErrorCode::MmapFailed,
        ErrorCode::DimensionExceedsLimit,
        ErrorCode::PixelCountExceedsLimit,
        ErrorCode::FirewallViolation,
        ErrorCode::InvalidCropBounds,
        ErrorCode::InvalidCropDimensions,
        ErrorCode::InvalidRotationAngle,
        ErrorCode::InvalidResizeDimensions,
        ErrorCode::InvalidResizeFit,
        ErrorCode::InvalidArgument,
        ErrorCode::InvalidPreset,
        ErrorCode::InvalidFirewallPolicy,
        ErrorCode::FileWriteFailed,
        ErrorCode::SourceConsumed,
    ];
    let non_recoverable_codes = vec![
        ErrorCode::UnsupportedFormat,
        ErrorCode::CorruptedImage,
        ErrorCode::DecodeFailed,
        ErrorCode::UnsupportedColorSpace,
        ErrorCode::ResizeFailed,
        ErrorCode::EncodeFailed,
        ErrorCode::InternalPanic,
        ErrorCode::Generic,
    ];
    for code in &recoverable_codes {
        assert!(
            code.is_recoverable(),
            "ErrorCode::{code:?} should be recoverable"
        );
    }
    for code in &non_recoverable_codes {
        assert!(
            !code.is_recoverable(),
            "ErrorCode::{code:?} should NOT be recoverable"
        );
    }
}

#[test]
fn test_recovery_hint_non_empty_for_all_variants() {
    // Build one instance of every LazyImageError variant and verify its hint
    let io_err = || std::io::Error::other("test");
    let all_errors: Vec<LazyImageError> = vec![
        LazyImageError::file_not_found("x"),
        LazyImageError::file_read_failed("x", io_err()),
        LazyImageError::mmap_failed("x", io_err()),
        LazyImageError::file_write_failed("x", io_err()),
        LazyImageError::unsupported_format("bmp"),
        LazyImageError::decode_failed("bad"),
        LazyImageError::corrupted_image(),
        LazyImageError::dimension_exceeds_limit(1, 1),
        LazyImageError::pixel_count_exceeds_limit(1, 1),
        LazyImageError::firewall_violation("blocked"),
        LazyImageError::invalid_crop_bounds(0, 0, 1, 1, 1, 1),
        LazyImageError::invalid_crop_dimensions(0, 0),
        LazyImageError::invalid_rotation_angle(45),
        LazyImageError::invalid_resize_dimensions(None, None),
        LazyImageError::invalid_resize_fit("bad"),
        LazyImageError::unsupported_color_space("Lab"),
        LazyImageError::resize_failed((1, 1), (1, 1), "fail"),
        LazyImageError::encode_failed("jpeg", "fail"),
        LazyImageError::invalid_argument("x", "y", "z"),
        LazyImageError::invalid_preset("none"),
        LazyImageError::invalid_firewall_policy("none"),
        LazyImageError::source_consumed(),
        LazyImageError::internal_panic("boom"),
        LazyImageError::generic("oops"),
    ];
    for err in &all_errors {
        let hint = err.recovery_hint();
        assert!(
            !hint.is_empty(),
            "recovery_hint() must be non-empty for {:?}",
            err.code()
        );
    }
}

#[test]
fn test_error_code_consistency_with_error_variant() {
    // Verify that LazyImageError.code().category() matches LazyImageError.category()
    // for every variant (ensures the two parallel match arms stay in sync).
    let io_err = || std::io::Error::other("test");
    let all_errors: Vec<LazyImageError> = vec![
        LazyImageError::file_not_found("x"),
        LazyImageError::file_read_failed("x", io_err()),
        LazyImageError::mmap_failed("x", io_err()),
        LazyImageError::file_write_failed("x", io_err()),
        LazyImageError::unsupported_format("bmp"),
        LazyImageError::decode_failed("bad"),
        LazyImageError::corrupted_image(),
        LazyImageError::dimension_exceeds_limit(1, 1),
        LazyImageError::pixel_count_exceeds_limit(1, 1),
        LazyImageError::firewall_violation("blocked"),
        LazyImageError::invalid_crop_bounds(0, 0, 1, 1, 1, 1),
        LazyImageError::invalid_crop_dimensions(0, 0),
        LazyImageError::invalid_rotation_angle(45),
        LazyImageError::invalid_resize_dimensions(None, None),
        LazyImageError::invalid_resize_fit("bad"),
        LazyImageError::unsupported_color_space("Lab"),
        LazyImageError::resize_failed((1, 1), (1, 1), "fail"),
        LazyImageError::encode_failed("jpeg", "fail"),
        LazyImageError::invalid_argument("x", "y", "z"),
        LazyImageError::invalid_preset("none"),
        LazyImageError::invalid_firewall_policy("none"),
        LazyImageError::source_consumed(),
        LazyImageError::internal_panic("boom"),
        LazyImageError::generic("oops"),
    ];
    for err in &all_errors {
        assert_eq!(
            err.code().category(),
            err.category(),
            "code().category() and category() must agree for {:?}",
            err.code()
        );
        assert_eq!(
            err.code().is_recoverable(),
            err.is_recoverable(),
            "code().is_recoverable() and is_recoverable() must agree for {:?}",
            err.code()
        );
    }
}

#[test]
fn test_error_code_repr_values() {
    // Verify #[repr(u16)] discriminant values match the E-code numbers
    assert_eq!(ErrorCode::FileNotFound as u16, 100);
    assert_eq!(ErrorCode::FileReadFailed as u16, 101);
    assert_eq!(ErrorCode::MmapFailed as u16, 102);
    assert_eq!(ErrorCode::UnsupportedFormat as u16, 111);
    assert_eq!(ErrorCode::DimensionExceedsLimit as u16, 121);
    assert_eq!(ErrorCode::PixelCountExceedsLimit as u16, 122);
    assert_eq!(ErrorCode::FirewallViolation as u16, 123);
    assert_eq!(ErrorCode::CorruptedImage as u16, 130);
    assert_eq!(ErrorCode::DecodeFailed as u16, 131);
    assert_eq!(ErrorCode::InvalidCropBounds as u16, 200);
    assert_eq!(ErrorCode::InvalidCropDimensions as u16, 201);
    assert_eq!(ErrorCode::InvalidRotationAngle as u16, 202);
    assert_eq!(ErrorCode::InvalidResizeDimensions as u16, 203);
    assert_eq!(ErrorCode::InvalidResizeFit as u16, 204);
    assert_eq!(ErrorCode::UnsupportedColorSpace as u16, 210);
    assert_eq!(ErrorCode::ResizeFailed as u16, 299);
    assert_eq!(ErrorCode::EncodeFailed as u16, 300);
    assert_eq!(ErrorCode::FileWriteFailed as u16, 301);
    assert_eq!(ErrorCode::InvalidArgument as u16, 400);
    assert_eq!(ErrorCode::InvalidPreset as u16, 401);
    assert_eq!(ErrorCode::InvalidFirewallPolicy as u16, 402);
    assert_eq!(ErrorCode::SourceConsumed as u16, 900);
    assert_eq!(ErrorCode::InternalPanic as u16, 901);
    assert_eq!(ErrorCode::Generic as u16, 999);
}

#[test]
fn test_error_category_repr_values() {
    assert_eq!(ErrorCategory::UserError as u32, 0);
    assert_eq!(ErrorCategory::CodecError as u32, 1);
    assert_eq!(ErrorCategory::ResourceLimit as u32, 2);
    assert_eq!(ErrorCategory::InternalBug as u32, 3);
}
