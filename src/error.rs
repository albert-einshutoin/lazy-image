// src/error.rs
//
// Unified error handling for lazy-image
// Uses thiserror for simple, type-safe error handling

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
use std::borrow::Cow;
use thiserror::Error;

/// lazy-image error types
///
/// All errors are type-safe and provide clear, actionable messages.
/// No numeric error codes - just clear error variants.
#[derive(Debug, Error)]
pub enum LazyImageError {
    // File I/O Errors
    #[error("File not found: {path}")]
    FileNotFound { path: Cow<'static, str> },

    #[error("Failed to read file '{path}': {source}")]
    FileReadFailed {
        path: Cow<'static, str>,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write file '{path}': {source}")]
    FileWriteFailed {
        path: Cow<'static, str>,
        #[source]
        source: std::io::Error,
    },

    // Decode Errors
    #[error("Unsupported image format: {format}")]
    UnsupportedFormat { format: Cow<'static, str> },

    #[error("Failed to decode image: {message}")]
    DecodeFailed { message: Cow<'static, str> },

    #[error("Corrupted image data")]
    CorruptedImage,

    // Size Limit Errors
    #[error("Image dimension {dimension} exceeds maximum {max}")]
    DimensionExceedsLimit { dimension: u32, max: u32 },

    #[error("Image pixel count {pixels} exceeds maximum {max}")]
    PixelCountExceedsLimit { pixels: u64, max: u64 },

    // Operation Errors
    #[error("Crop bounds ({x}+{width}, {y}+{height}) exceed image dimensions ({img_width}x{img_height})")]
    InvalidCropBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        img_width: u32,
        img_height: u32,
    },

    #[error(
        "Unsupported rotation angle: {degrees}. Only 0, 90, 180, 270 (and negatives) are supported"
    )]
    InvalidRotationAngle { degrees: i32 },

    #[error("Invalid resize dimensions: width={width:?}, height={height:?}")]
    InvalidResizeDimensions {
        width: Option<u32>,
        height: Option<u32>,
    },

    #[error("Resize failed ({source_width}x{source_height} -> {target_width}x{target_height}): {message}")]
    ResizeFailed {
        source_width: u32,
        source_height: u32,
        target_width: u32,
        target_height: u32,
        message: Cow<'static, str>,
    },

    #[error("Unsupported color space: {color_space}")]
    UnsupportedColorSpace { color_space: Cow<'static, str> },

    // Encode Errors
    #[error("Failed to encode as {format}: {message}")]
    EncodeFailed {
        format: Cow<'static, str>,
        message: Cow<'static, str>,
    },

    // Configuration Errors
    #[error("Unknown preset: '{name}'. Available: thumbnail, avatar, hero, social")]
    InvalidPreset { name: Cow<'static, str> },

    // State Errors
    #[error("Image source already consumed. Use clone() for multi-output scenarios")]
    SourceConsumed,

    // Internal Errors
    #[error("Internal error: {message}")]
    InternalPanic { message: Cow<'static, str> },

    // Generic Error
    #[error("{message}")]
    Generic { message: Cow<'static, str> },
}

// Constructor Helpers
impl LazyImageError {
    pub fn file_not_found(path: impl Into<Cow<'static, str>>) -> Self {
        Self::FileNotFound {
            path: path.into(),
        }
    }

    pub fn file_read_failed(path: impl Into<Cow<'static, str>>, source: std::io::Error) -> Self {
        Self::FileReadFailed {
            path: path.into(),
            source,
        }
    }

    pub fn file_write_failed(path: impl Into<Cow<'static, str>>, source: std::io::Error) -> Self {
        Self::FileWriteFailed {
            path: path.into(),
            source,
        }
    }

    pub fn unsupported_format(format: impl Into<Cow<'static, str>>) -> Self {
        Self::UnsupportedFormat {
            format: format.into(),
        }
    }

    pub fn decode_failed(message: impl Into<Cow<'static, str>>) -> Self {
        Self::DecodeFailed {
            message: message.into(),
        }
    }

    pub fn corrupted_image() -> Self {
        Self::CorruptedImage
    }

    pub fn dimension_exceeds_limit(dimension: u32, max: u32) -> Self {
        Self::DimensionExceedsLimit { dimension, max }
    }

    pub fn pixel_count_exceeds_limit(pixels: u64, max: u64) -> Self {
        Self::PixelCountExceedsLimit { pixels, max }
    }

    pub fn invalid_crop_bounds(
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        img_width: u32,
        img_height: u32,
    ) -> Self {
        Self::InvalidCropBounds {
            x,
            y,
            width,
            height,
            img_width,
            img_height,
        }
    }

    pub fn invalid_rotation_angle(degrees: i32) -> Self {
        Self::InvalidRotationAngle { degrees }
    }

    pub fn invalid_resize_dimensions(width: Option<u32>, height: Option<u32>) -> Self {
        Self::InvalidResizeDimensions { width, height }
    }

    pub fn resize_failed(
        source_dims: (u32, u32),
        target_dims: (u32, u32),
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::ResizeFailed {
            source_width: source_dims.0,
            source_height: source_dims.1,
            target_width: target_dims.0,
            target_height: target_dims.1,
            message: message.into(),
        }
    }

    pub fn unsupported_color_space(color_space: impl Into<Cow<'static, str>>) -> Self {
        Self::UnsupportedColorSpace {
            color_space: color_space.into(),
        }
    }

    pub fn encode_failed(
        format: impl Into<Cow<'static, str>>,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::EncodeFailed {
            format: format.into(),
            message: message.into(),
        }
    }

    pub fn invalid_preset(name: impl Into<Cow<'static, str>>) -> Self {
        Self::InvalidPreset {
            name: name.into(),
        }
    }

    pub fn source_consumed() -> Self {
        Self::SourceConsumed
    }

    pub fn internal_panic(message: impl Into<Cow<'static, str>>) -> Self {
        Self::InternalPanic {
            message: message.into(),
        }
    }

    pub fn generic(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Generic {
            message: message.into(),
        }
    }

    // Static string literal helpers (zero-allocation for static strings)
    // These are convenience methods for when you have a static string literal.
    // They are optional since impl Into<Cow<'static, str>> already handles &'static str,
    // but they make the intent explicit and can help with code clarity.

    pub fn decode_failed_static(msg: &'static str) -> Self {
        Self::DecodeFailed {
            message: Cow::Borrowed(msg),
        }
    }

    pub fn internal_panic_static(msg: &'static str) -> Self {
        Self::InternalPanic {
            message: Cow::Borrowed(msg),
        }
    }

    pub fn encode_failed_static(format: &'static str, message: &'static str) -> Self {
        Self::EncodeFailed {
            format: Cow::Borrowed(format),
            message: Cow::Borrowed(message),
        }
    }

    /// Check if this error is recoverable (user can fix it)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::FileNotFound { .. }
                | Self::FileReadFailed { .. }
                | Self::FileWriteFailed { .. }
                | Self::DimensionExceedsLimit { .. }
                | Self::PixelCountExceedsLimit { .. }
                | Self::InvalidCropBounds { .. }
                | Self::InvalidRotationAngle { .. }
                | Self::InvalidResizeDimensions { .. }
                | Self::InvalidPreset { .. }
                | Self::SourceConsumed
        )
    }
}

// Conversion to NAPI Error
#[cfg(feature = "napi")]
impl From<LazyImageError> for napi::Error {
    fn from(err: LazyImageError) -> Self {
        let status = match &err {
            // Input/Argument Errors -> InvalidArg
            LazyImageError::UnsupportedFormat { .. }
            | LazyImageError::DimensionExceedsLimit { .. }
            | LazyImageError::PixelCountExceedsLimit { .. }
            | LazyImageError::InvalidCropBounds { .. }
            | LazyImageError::InvalidRotationAngle { .. }
            | LazyImageError::InvalidResizeDimensions { .. }
            | LazyImageError::UnsupportedColorSpace { .. }
            | LazyImageError::InvalidPreset { .. } => Status::InvalidArg,

            // All other errors -> GenericFailure
            _ => Status::GenericFailure,
        };

        napi::Error::new(status, err.to_string())
    }
}

// Result type alias
pub type Result<T> = std::result::Result<T, LazyImageError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = LazyImageError::file_not_found("/path/to/file.jpg");
        assert!(err.to_string().contains("/path/to/file.jpg"));
    }

    #[test]
    fn test_error_recoverable() {
        assert!(LazyImageError::file_not_found("test.jpg").is_recoverable());
        assert!(LazyImageError::invalid_crop_bounds(0, 0, 100, 100, 50, 50).is_recoverable());
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
        let _ = LazyImageError::invalid_rotation_angle(45);
        let _ = LazyImageError::invalid_resize_dimensions(None, None);
        let _ = LazyImageError::resize_failed((100, 100), (50, 50), "test");
        let _ = LazyImageError::unsupported_color_space("CMYK");
        let _ = LazyImageError::encode_failed("jpeg", "test");
        let _ = LazyImageError::invalid_preset("unknown");
        let _ = LazyImageError::source_consumed();
        let _ = LazyImageError::internal_panic("test");
        let _ = LazyImageError::generic("test");
    }
}
