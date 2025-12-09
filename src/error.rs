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
/// E1xx: Input Errors
/// E2xx: Processing Errors
/// E3xx: Output Errors
/// E4xx: Configuration Errors
/// E9xx: Internal Errors
#[cfg(feature = "napi")]
#[napi]
#[derive(Debug, PartialEq, Eq)]
pub enum ErrorCode {
    // Input Errors (E1xx)
    FileNotFound = 100,
    FileReadFailed = 101,
    InvalidImageFormat = 110,
    UnsupportedFormat = 111,
    ImageTooLarge = 120,
    DimensionExceedsLimit = 121,
    PixelCountExceedsLimit = 122,
    CorruptedImage = 130,
    DecodeFailed = 131,

    // Processing Errors (E2xx)
    InvalidCropBounds = 200,
    InvalidRotationAngle = 201,
    InvalidResizeDimensions = 202,
    UnsupportedColorSpace = 210,
    OperationFailed = 299,

    // Output Errors (E3xx)
    EncodeFailed = 300,
    FileWriteFailed = 301,
    OutputPathInvalid = 302,

    // Configuration Errors (E4xx)
    InvalidQuality = 400,
    InvalidPreset = 401,

    // Internal Errors (E9xx)
    SourceConsumed = 900,
    InternalPanic = 901,
    UnexpectedState = 999,
}

#[cfg(not(feature = "napi"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // Input Errors (E1xx)
    FileNotFound = 100,
    FileReadFailed = 101,
    InvalidImageFormat = 110,
    UnsupportedFormat = 111,
    ImageTooLarge = 120,
    DimensionExceedsLimit = 121,
    PixelCountExceedsLimit = 122,
    CorruptedImage = 130,
    DecodeFailed = 131,

    // Processing Errors (E2xx)
    InvalidCropBounds = 200,
    InvalidRotationAngle = 201,
    InvalidResizeDimensions = 202,
    UnsupportedColorSpace = 210,
    OperationFailed = 299,

    // Output Errors (E3xx)
    EncodeFailed = 300,
    FileWriteFailed = 301,
    OutputPathInvalid = 302,

    // Configuration Errors (E4xx)
    InvalidQuality = 400,
    InvalidPreset = 401,

    // Internal Errors (E9xx)
    SourceConsumed = 900,
    InternalPanic = 901,
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
