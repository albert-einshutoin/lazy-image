// src/error.rs
//
// Unified error handling for lazy-image
// Uses thiserror for type-safe error handling with error codes

use thiserror::Error;
#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
use napi_derive::napi;

/// lazy-image Error Codes
///
/// Error codes are organized by category:
/// - **E1xx**: Input Errors - Issues with input files or data
/// - **E2xx**: Processing Errors - Issues during image processing operations
/// - **E3xx**: Output Errors - Issues when writing or encoding output
/// - **E4xx**: Configuration Errors - Invalid parameters or settings
/// - **E9xx**: Internal Errors - Unexpected internal state or bugs
///
/// Each error code is type-safe and can be used programmatically.
#[cfg(feature = "napi")]
#[napi]
#[derive(Debug, PartialEq, Eq)]
pub enum ErrorCode {
    // Input Errors (E1xx)
    /// **E100**: File not found
    ///
    /// The specified file path does not exist.
    /// **Recoverable**: Yes - Check the file path and permissions.
    FileNotFound = 100,

    /// **E101**: Failed to read file
    ///
    /// An I/O error occurred while reading the file.
    /// **Recoverable**: Yes - Check file permissions and disk space.
    FileReadFailed = 101,

    /// **E110**: Invalid image format
    ///
    /// The file format is not recognized or is invalid.
    /// **Recoverable**: No - The file is corrupted or not an image.
    InvalidImageFormat = 110,

    /// **E111**: Unsupported image format
    ///
    /// The image format is recognized but not supported by lazy-image.
    /// **Recoverable**: No - Convert to a supported format (JPEG, PNG, WebP).
    UnsupportedFormat = 111,

    /// **E120**: Image too large
    ///
    /// The image exceeds size limits (file size or memory constraints).
    /// **Recoverable**: No - Resize or compress the image before processing.
    ImageTooLarge = 120,

    /// **E121**: Dimension exceeds limit
    ///
    /// Image width or height exceeds the maximum allowed dimension.
    /// **Recoverable**: Yes - Resize the image to fit within limits.
    DimensionExceedsLimit = 121,

    /// **E122**: Pixel count exceeds limit
    ///
    /// Total pixel count (width × height) exceeds the maximum allowed.
    /// **Recoverable**: Yes - Resize the image to reduce pixel count.
    PixelCountExceedsLimit = 122,

    /// **E130**: Corrupted image data
    ///
    /// The image file is corrupted or contains invalid data.
    /// **Recoverable**: No - The file needs to be repaired or recreated.
    CorruptedImage = 130,

    /// **E131**: Failed to decode image
    ///
    /// An error occurred during image decoding (format-specific issue).
    /// **Recoverable**: No - Check if the file is a valid image.
    DecodeFailed = 131,

    // Processing Errors (E2xx)
    /// **E200**: Invalid crop bounds
    ///
    /// Crop coordinates exceed image dimensions.
    /// **Recoverable**: Yes - Adjust crop coordinates to fit within image bounds.
    InvalidCropBounds = 200,

    /// **E201**: Invalid rotation angle
    ///
    /// Rotation angle is not a multiple of 90 degrees.
    /// **Recoverable**: Yes - Use 0, 90, 180, or 270 degrees (or negatives).
    InvalidRotationAngle = 201,

    /// **E202**: Invalid resize dimensions
    ///
    /// Resize dimensions are invalid (e.g., both width and height are None).
    /// **Recoverable**: Yes - Provide at least one valid dimension.
    InvalidResizeDimensions = 202,

    /// **E210**: Unsupported color space
    ///
    /// The requested color space conversion is not supported.
    /// **Recoverable**: No - Use a supported color space.
    UnsupportedColorSpace = 210,

    /// **E299**: Operation failed
    ///
    /// A general processing operation failed.
    /// **Recoverable**: Depends on the specific operation.
    OperationFailed = 299,

    // Output Errors (E3xx)
    /// **E300**: Failed to encode image
    ///
    /// An error occurred during image encoding (format-specific issue).
    /// **Recoverable**: No - Check encoding parameters and try a different format.
    EncodeFailed = 300,

    /// **E301**: Failed to write file
    ///
    /// An I/O error occurred while writing the output file.
    /// **Recoverable**: Yes - Check disk space and write permissions.
    FileWriteFailed = 301,

    /// **E302**: Output path invalid
    ///
    /// The output file path is invalid or inaccessible.
    /// **Recoverable**: Yes - Provide a valid output path.
    OutputPathInvalid = 302,

    // Configuration Errors (E4xx)
    /// **E400**: Invalid quality value
    ///
    /// Quality parameter is out of valid range (typically 1-100).
    /// **Recoverable**: Yes - Use a quality value within the valid range.
    InvalidQuality = 400,

    /// **E401**: Invalid preset name
    ///
    /// The specified preset name is not recognized.
    /// **Recoverable**: Yes - Use a valid preset: thumbnail, avatar, hero, or social.
    InvalidPreset = 401,

    // Internal Errors (E9xx)
    /// **E900**: Source already consumed
    ///
    /// Image source has already been consumed and cannot be reused.
    /// **Recoverable**: Yes - Use `clone()` for multi-output scenarios.
    SourceConsumed = 900,

    /// **E901**: Internal panic
    ///
    /// An unexpected internal error occurred (likely a bug).
    /// **Recoverable**: No - Report this as a bug.
    InternalPanic = 901,

    /// **E999**: Unexpected state
    ///
    /// The library is in an unexpected internal state.
    /// **Recoverable**: No - Report this as a bug.
    UnexpectedState = 999,
}

#[cfg(not(feature = "napi"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // Input Errors (E1xx)
    /// **E100**: File not found
    FileNotFound = 100,
    /// **E101**: Failed to read file
    FileReadFailed = 101,
    /// **E110**: Invalid image format
    InvalidImageFormat = 110,
    /// **E111**: Unsupported image format
    UnsupportedFormat = 111,
    /// **E120**: Image too large
    ImageTooLarge = 120,
    /// **E121**: Dimension exceeds limit
    DimensionExceedsLimit = 121,
    /// **E122**: Pixel count exceeds limit
    PixelCountExceedsLimit = 122,
    /// **E130**: Corrupted image data
    CorruptedImage = 130,
    /// **E131**: Failed to decode image
    DecodeFailed = 131,

    // Processing Errors (E2xx)
    /// **E200**: Invalid crop bounds
    InvalidCropBounds = 200,
    /// **E201**: Invalid rotation angle
    InvalidRotationAngle = 201,
    /// **E202**: Invalid resize dimensions
    InvalidResizeDimensions = 202,
    /// **E210**: Unsupported color space
    UnsupportedColorSpace = 210,
    /// **E299**: Operation failed
    OperationFailed = 299,

    // Output Errors (E3xx)
    /// **E300**: Failed to encode image
    EncodeFailed = 300,
    /// **E301**: Failed to write file
    FileWriteFailed = 301,
    /// **E302**: Output path invalid
    OutputPathInvalid = 302,

    // Configuration Errors (E4xx)
    /// **E400**: Invalid quality value
    InvalidQuality = 400,
    /// **E401**: Invalid preset name
    InvalidPreset = 401,

    // Internal Errors (E9xx)
    /// **E900**: Source already consumed
    SourceConsumed = 900,
    /// **E901**: Internal panic
    InternalPanic = 901,
    /// **E999**: Unexpected state
    UnexpectedState = 999,
}

impl ErrorCode {
    /// Get error code as string (e.g., "E100")
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FileNotFound => "E100",
            Self::FileReadFailed => "E101",
            Self::InvalidImageFormat => "E110",
            Self::UnsupportedFormat => "E111",
            Self::ImageTooLarge => "E120",
            Self::DimensionExceedsLimit => "E121",
            Self::PixelCountExceedsLimit => "E122",
            Self::CorruptedImage => "E130",
            Self::DecodeFailed => "E131",
            Self::InvalidCropBounds => "E200",
            Self::InvalidRotationAngle => "E201",
            Self::InvalidResizeDimensions => "E202",
            Self::UnsupportedColorSpace => "E210",
            Self::OperationFailed => "E299",
            Self::EncodeFailed => "E300",
            Self::FileWriteFailed => "E301",
            Self::OutputPathInvalid => "E302",
            Self::InvalidQuality => "E400",
            Self::InvalidPreset => "E401",
            Self::SourceConsumed => "E900",
            Self::InternalPanic => "E901",
            Self::UnexpectedState => "E999",
        }
    }

    /// Get numeric error code value
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }

    /// Returns `true` if this error is recoverable (user can fix it)
    ///
    /// Recoverable errors are those that users can address by:
    /// - Fixing input parameters
    /// - Adjusting configuration
    /// - Correcting file paths or permissions
    ///
    /// Non-recoverable errors typically indicate:
    /// - Corrupted data
    /// - Internal bugs
    /// - Unsupported formats
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::FileNotFound
                | Self::FileReadFailed
                | Self::FileWriteFailed
                | Self::DimensionExceedsLimit
                | Self::PixelCountExceedsLimit
                | Self::InvalidCropBounds
                | Self::InvalidRotationAngle
                | Self::InvalidResizeDimensions
                | Self::InvalidPreset
                | Self::InvalidQuality
                | Self::OutputPathInvalid
                | Self::SourceConsumed
        )
    }

    /// Get the error category as a string
    ///
    /// Returns the category prefix (E1xx, E2xx, etc.) for this error code.
    pub fn category(&self) -> &'static str {
        match self.as_u32() {
            100..=199 => "E1xx: Input Errors",
            200..=299 => "E2xx: Processing Errors",
            300..=399 => "E3xx: Output Errors",
            400..=499 => "E4xx: Configuration Errors",
            900..=999 => "E9xx: Internal Errors",
            _ => "Unknown",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Error)]
pub enum LazyImageError {
    // File I/O Errors
    #[error("[{code}] File not found: {path}")]
    FileNotFound {
        code: ErrorCode,
        path: String,
    },

    #[error("[{code}] Failed to read file '{path}': {source}")]
    FileReadFailed {
        code: ErrorCode,
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("[{code}] Failed to write file '{path}': {source}")]
    FileWriteFailed {
        code: ErrorCode,
        path: String,
        #[source]
        source: std::io::Error,
    },

    // Decode Errors
    #[error("[{code}] Unsupported image format: {format}")]
    UnsupportedFormat {
        code: ErrorCode,
        format: String,
    },

    #[error("[{code}] Failed to decode image: {message}")]
    DecodeFailed {
        code: ErrorCode,
        message: String,
    },

    #[error("[{code}] Corrupted image data")]
    CorruptedImage {
        code: ErrorCode,
    },

    // Size Limit Errors
    #[error("[{code}] Image dimension {dimension} exceeds maximum {max}")]
    DimensionExceedsLimit {
        code: ErrorCode,
        dimension: u32,
        max: u32,
    },

    #[error("[{code}] Image pixel count {pixels} exceeds maximum {max}")]
    PixelCountExceedsLimit {
        code: ErrorCode,
        pixels: u64,
        max: u64,
    },

    // Operation Errors
    #[error("[{code}] Crop bounds ({x}+{width}, {y}+{height}) exceed image dimensions ({img_width}x{img_height})")]
    InvalidCropBounds {
        code: ErrorCode,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        img_width: u32,
        img_height: u32,
    },

    #[error("[{code}] Unsupported rotation angle: {degrees}. Only 0, 90, 180, 270 (and negatives) are supported")]
    InvalidRotationAngle {
        code: ErrorCode,
        degrees: i32,
    },

    #[error("[{code}] Invalid resize dimensions: width={width:?}, height={height:?}")]
    InvalidResizeDimensions {
        code: ErrorCode,
        width: Option<u32>,
        height: Option<u32>,
    },

    #[error("[{code}] Unsupported color space: {color_space}")]
    UnsupportedColorSpace {
        code: ErrorCode,
        color_space: String,
    },

    // Encode Errors
    #[error("[{code}] Failed to encode as {format}: {message}")]
    EncodeFailed {
        code: ErrorCode,
        format: String,
        message: String,
    },

    // Configuration Errors
    #[error("[{code}] Unknown preset: '{name}'. Available: thumbnail, avatar, hero, social")]
    InvalidPreset {
        code: ErrorCode,
        name: String,
    },

    // State Errors
    #[error("[{code}] Image source already consumed. Use clone() for multi-output scenarios")]
    SourceConsumed {
        code: ErrorCode,
    },

    // Internal Errors
    #[error("[{code}] Internal error: {message}")]
    InternalPanic {
        code: ErrorCode,
        message: String,
    },

    // Generic Error
    #[error("[{code}] {message}")]
    Generic {
        code: ErrorCode,
        message: String,
    },
}

// Constructor Helpers
impl LazyImageError {
    pub fn file_not_found(path: impl Into<String>) -> Self {
        Self::FileNotFound {
            code: ErrorCode::FileNotFound,
            path: path.into(),
        }
    }

    pub fn file_read_failed(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::FileReadFailed {
            code: ErrorCode::FileReadFailed,
            path: path.into(),
            source,
        }
    }

    pub fn file_write_failed(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::FileWriteFailed {
            code: ErrorCode::FileWriteFailed,
            path: path.into(),
            source,
        }
    }

    pub fn unsupported_format(format: impl Into<String>) -> Self {
        Self::UnsupportedFormat {
            code: ErrorCode::UnsupportedFormat,
            format: format.into(),
        }
    }

    pub fn decode_failed(message: impl Into<String>) -> Self {
        Self::DecodeFailed {
            code: ErrorCode::DecodeFailed,
            message: message.into(),
        }
    }

    pub fn corrupted_image() -> Self {
        Self::CorruptedImage {
            code: ErrorCode::CorruptedImage,
        }
    }

    pub fn dimension_exceeds_limit(dimension: u32, max: u32) -> Self {
        Self::DimensionExceedsLimit {
            code: ErrorCode::DimensionExceedsLimit,
            dimension,
            max,
        }
    }

    pub fn pixel_count_exceeds_limit(pixels: u64, max: u64) -> Self {
        Self::PixelCountExceedsLimit {
            code: ErrorCode::PixelCountExceedsLimit,
            pixels,
            max,
        }
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
            code: ErrorCode::InvalidCropBounds,
            x,
            y,
            width,
            height,
            img_width,
            img_height,
        }
    }

    pub fn invalid_rotation_angle(degrees: i32) -> Self {
        Self::InvalidRotationAngle {
            code: ErrorCode::InvalidRotationAngle,
            degrees,
        }
    }

    pub fn invalid_resize_dimensions(width: Option<u32>, height: Option<u32>) -> Self {
        Self::InvalidResizeDimensions {
            code: ErrorCode::InvalidResizeDimensions,
            width,
            height,
        }
    }

    pub fn unsupported_color_space(color_space: impl Into<String>) -> Self {
        Self::UnsupportedColorSpace {
            code: ErrorCode::UnsupportedColorSpace,
            color_space: color_space.into(),
        }
    }

    pub fn encode_failed(format: impl Into<String>, message: impl Into<String>) -> Self {
        Self::EncodeFailed {
            code: ErrorCode::EncodeFailed,
            format: format.into(),
            message: message.into(),
        }
    }

    pub fn invalid_preset(name: impl Into<String>) -> Self {
        Self::InvalidPreset {
            code: ErrorCode::InvalidPreset,
            name: name.into(),
        }
    }

    pub fn source_consumed() -> Self {
        Self::SourceConsumed {
            code: ErrorCode::SourceConsumed,
        }
    }

    pub fn internal_panic(message: impl Into<String>) -> Self {
        Self::InternalPanic {
            code: ErrorCode::InternalPanic,
            message: message.into(),
        }
    }

    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic {
            code: ErrorCode::UnexpectedState,
            message: message.into(),
        }
    }

    /// Get the error code
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::FileNotFound { code, .. } => *code,
            Self::FileReadFailed { code, .. } => *code,
            Self::FileWriteFailed { code, .. } => *code,
            Self::UnsupportedFormat { code, .. } => *code,
            Self::DecodeFailed { code, .. } => *code,
            Self::CorruptedImage { code } => *code,
            Self::DimensionExceedsLimit { code, .. } => *code,
            Self::PixelCountExceedsLimit { code, .. } => *code,
            Self::InvalidCropBounds { code, .. } => *code,
            Self::InvalidRotationAngle { code, .. } => *code,
            Self::InvalidResizeDimensions { code, .. } => *code,
            Self::UnsupportedColorSpace { code, .. } => *code,
            Self::EncodeFailed { code, .. } => *code,
            Self::InvalidPreset { code, .. } => *code,
            Self::SourceConsumed { code } => *code,
            Self::InternalPanic { code, .. } => *code,
            Self::Generic { code, .. } => *code,
        }
    }

    /// Get error code as string (for backward compatibility)
    pub fn code_str(&self) -> &'static str {
        self.code().as_str()
    }
}

// Conversion to NAPI Error
#[cfg(feature = "napi")]
impl From<LazyImageError> for napi::Error {
    fn from(err: LazyImageError) -> Self {
        let error_code = err.code();
        let status = match error_code {
            // Input/Argument Errors -> InvalidArg
            ErrorCode::UnsupportedFormat
            | ErrorCode::DimensionExceedsLimit
            | ErrorCode::PixelCountExceedsLimit
            | ErrorCode::InvalidCropBounds
            | ErrorCode::InvalidRotationAngle
            | ErrorCode::InvalidResizeDimensions
            | ErrorCode::UnsupportedColorSpace
            | ErrorCode::InvalidPreset
            | ErrorCode::InvalidQuality => Status::InvalidArg,

            // I/O and System Errors -> GenericFailure
            ErrorCode::FileNotFound
            | ErrorCode::FileReadFailed
            | ErrorCode::FileWriteFailed => Status::GenericFailure,

            // Processing/Internal Errors -> GenericFailure
            ErrorCode::DecodeFailed
            | ErrorCode::CorruptedImage
            | ErrorCode::EncodeFailed
            | ErrorCode::SourceConsumed
            | ErrorCode::InternalPanic
            | ErrorCode::UnexpectedState
            | ErrorCode::OperationFailed
            | ErrorCode::InvalidImageFormat
            | ErrorCode::ImageTooLarge
            | ErrorCode::OutputPathInvalid => Status::GenericFailure,
        };

        // Create error with code information
        napi::Error::new(status, err.to_string())
    }
}

// Result type alias
pub type Result<T> = std::result::Result<T, LazyImageError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_display() {
        assert_eq!(ErrorCode::FileNotFound.as_str(), "E100");
        assert_eq!(ErrorCode::DecodeFailed.as_str(), "E131");
        assert_eq!(ErrorCode::InvalidCropBounds.as_str(), "E200");
        assert_eq!(ErrorCode::EncodeFailed.as_str(), "E300");
        assert_eq!(ErrorCode::InvalidPreset.as_str(), "E401");
        assert_eq!(ErrorCode::InternalPanic.as_str(), "E901");
    }

    #[test]
    fn test_error_code_numeric() {
        assert_eq!(ErrorCode::FileNotFound.as_u32(), 100);
        assert_eq!(ErrorCode::DecodeFailed.as_u32(), 131);
        assert_eq!(ErrorCode::InvalidCropBounds.as_u32(), 200);
        assert_eq!(ErrorCode::EncodeFailed.as_u32(), 300);
        assert_eq!(ErrorCode::InvalidPreset.as_u32(), 401);
        assert_eq!(ErrorCode::InternalPanic.as_u32(), 901);
    }

    #[test]
    fn test_error_code_recoverable() {
        // Recoverable errors
        assert!(ErrorCode::FileNotFound.is_recoverable());
        assert!(ErrorCode::FileReadFailed.is_recoverable());
        assert!(ErrorCode::FileWriteFailed.is_recoverable());
        assert!(ErrorCode::DimensionExceedsLimit.is_recoverable());
        assert!(ErrorCode::PixelCountExceedsLimit.is_recoverable());
        assert!(ErrorCode::InvalidCropBounds.is_recoverable());
        assert!(ErrorCode::InvalidRotationAngle.is_recoverable());
        assert!(ErrorCode::InvalidResizeDimensions.is_recoverable());
        assert!(ErrorCode::InvalidPreset.is_recoverable());
        assert!(ErrorCode::InvalidQuality.is_recoverable());
        assert!(ErrorCode::OutputPathInvalid.is_recoverable());
        assert!(ErrorCode::SourceConsumed.is_recoverable());

        // Non-recoverable errors
        assert!(!ErrorCode::InvalidImageFormat.is_recoverable());
        assert!(!ErrorCode::UnsupportedFormat.is_recoverable());
        assert!(!ErrorCode::ImageTooLarge.is_recoverable());
        assert!(!ErrorCode::CorruptedImage.is_recoverable());
        assert!(!ErrorCode::DecodeFailed.is_recoverable());
        assert!(!ErrorCode::UnsupportedColorSpace.is_recoverable());
        assert!(!ErrorCode::OperationFailed.is_recoverable());
        assert!(!ErrorCode::EncodeFailed.is_recoverable());
        assert!(!ErrorCode::InternalPanic.is_recoverable());
        assert!(!ErrorCode::UnexpectedState.is_recoverable());
    }

    #[test]
    fn test_error_code_category() {
        assert_eq!(ErrorCode::FileNotFound.category(), "E1xx: Input Errors");
        assert_eq!(ErrorCode::InvalidCropBounds.category(), "E2xx: Processing Errors");
        assert_eq!(ErrorCode::EncodeFailed.category(), "E3xx: Output Errors");
        assert_eq!(ErrorCode::InvalidPreset.category(), "E4xx: Configuration Errors");
        assert_eq!(ErrorCode::InternalPanic.category(), "E9xx: Internal Errors");
    }

    #[test]
    fn test_error_code_display_trait() {
        assert_eq!(format!("{}", ErrorCode::FileNotFound), "E100");
        assert_eq!(format!("{}", ErrorCode::DecodeFailed), "E131");
    }

    #[test]
    fn test_lazy_image_error_code() {
        let err = LazyImageError::file_not_found("/path/to/file.jpg");
        assert_eq!(err.code(), ErrorCode::FileNotFound);
        assert_eq!(err.code_str(), "E100");
    }

    #[test]
    fn test_lazy_image_error_display() {
        let err = LazyImageError::file_not_found("/path/to/file.jpg");
        let msg = err.to_string();
        assert!(msg.contains("E100"));
        assert!(msg.contains("/path/to/file.jpg"));
    }

    #[test]
    fn test_lazy_image_error_with_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let err = LazyImageError::file_read_failed("/path/to/file.jpg", io_err);
        assert_eq!(err.code(), ErrorCode::FileReadFailed);
        let msg = err.to_string();
        assert!(msg.contains("E101"));
        assert!(msg.contains("/path/to/file.jpg"));
    }

    #[test]
    fn test_all_error_constructors() {
        // Test all constructor helpers
        let _ = LazyImageError::file_not_found("test.jpg");
        let _ = LazyImageError::file_read_failed("test.jpg", std::io::Error::from(std::io::ErrorKind::NotFound));
        let _ = LazyImageError::file_write_failed("test.jpg", std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let _ = LazyImageError::unsupported_format("gif");
        let _ = LazyImageError::decode_failed("test");
        let _ = LazyImageError::corrupted_image();
        let _ = LazyImageError::dimension_exceeds_limit(10000, 8000);
        let _ = LazyImageError::pixel_count_exceeds_limit(1000000000, 100000000);
        let _ = LazyImageError::invalid_crop_bounds(100, 100, 500, 500, 200, 200);
        let _ = LazyImageError::invalid_rotation_angle(45);
        let _ = LazyImageError::invalid_resize_dimensions(None, None);
        let _ = LazyImageError::unsupported_color_space("CMYK");
        let _ = LazyImageError::encode_failed("jpeg", "test");
        let _ = LazyImageError::invalid_preset("unknown");
        let _ = LazyImageError::source_consumed();
        let _ = LazyImageError::internal_panic("test");
        let _ = LazyImageError::generic("test");
    }
}
