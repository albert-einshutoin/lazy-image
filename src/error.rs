// src/error.rs
//
// Structured error types for lazy-image
// Replaces string-based Error::from_reason() with type-safe error handling

use napi::bindgen_prelude::*;

/// Custom error type for lazy-image operations
#[derive(Debug)]
pub enum LazyImageError {
    /// File read operation failed
    FileReadFailed {
        path: String,
        source: String,
    },
    /// Image source already consumed (cannot decode twice)
    SourceConsumed,
    /// Internal panic occurred (e.g., mozjpeg panicked)
    InternalPanic {
        message: String,
    },
    /// Image dimension exceeds maximum allowed
    DimensionExceedsLimit {
        dimension: u32,
        max: u32,
    },
    /// Total pixel count exceeds maximum allowed
    PixelCountExceedsLimit {
        pixels: u64,
        max: u64,
    },
    /// Invalid crop bounds
    InvalidCropBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        img_width: u32,
        img_height: u32,
    },
    /// Invalid rotation angle
    InvalidRotationAngle {
        degrees: i32,
    },
    /// Unsupported color space
    UnsupportedColorSpace {
        color_space: String,
    },
    /// Invalid preset name
    InvalidPreset {
        name: String,
    },
    /// Encode operation failed
    EncodeFailed {
        format: String,
        reason: String,
    },
    /// Decode operation failed
    DecodeFailed {
        reason: String,
    },
    /// Generic error with message
    Generic {
        message: String,
    },
}

impl LazyImageError {
    /// Create a file read error
    pub fn file_read_failed(path: &str, source: &dyn std::error::Error) -> Self {
        Self::FileReadFailed {
            path: path.to_string(),
            source: source.to_string(),
        }
    }

    /// Create a source consumed error
    pub fn source_consumed() -> Self {
        Self::SourceConsumed
    }

    /// Create an internal panic error
    pub fn internal_panic(message: &str) -> Self {
        Self::InternalPanic {
            message: message.to_string(),
        }
    }

    /// Create a dimension exceeds limit error
    pub fn dimension_exceeds_limit(dimension: u32, max: u32) -> Self {
        Self::DimensionExceedsLimit { dimension, max }
    }

    /// Create a pixel count exceeds limit error
    pub fn pixel_count_exceeds_limit(pixels: u64, max: u64) -> Self {
        Self::PixelCountExceedsLimit { pixels, max }
    }

    /// Create an invalid crop bounds error
    pub fn invalid_crop_bounds(x: u32, y: u32, width: u32, height: u32, img_width: u32, img_height: u32) -> Self {
        Self::InvalidCropBounds {
            x,
            y,
            width,
            height,
            img_width,
            img_height,
        }
    }

    /// Create an invalid rotation angle error
    pub fn invalid_rotation_angle(degrees: i32) -> Self {
        Self::InvalidRotationAngle { degrees }
    }

    /// Create an unsupported color space error
    pub fn unsupported_color_space(color_space: &str) -> Self {
        Self::UnsupportedColorSpace {
            color_space: color_space.to_string(),
        }
    }

    /// Create an invalid preset error
    pub fn invalid_preset(name: &str) -> Self {
        Self::InvalidPreset {
            name: name.to_string(),
        }
    }

    /// Create an encode failed error
    pub fn encode_failed(format: &str, reason: String) -> Self {
        Self::EncodeFailed {
            format: format.to_string(),
            reason,
        }
    }

    /// Create a decode failed error
    pub fn decode_failed(reason: &str) -> Self {
        Self::DecodeFailed {
            reason: reason.to_string(),
        }
    }
}

impl std::fmt::Display for LazyImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileReadFailed { path, source } => {
                write!(f, "failed to read file '{}': {}", path, source)
            }
            Self::SourceConsumed => {
                write!(f, "image source already consumed")
            }
            Self::InternalPanic { message } => {
                write!(f, "{}", message)
            }
            Self::DimensionExceedsLimit { dimension, max } => {
                write!(f, "image too large: {} exceeds max dimension {}", dimension, max)
            }
            Self::PixelCountExceedsLimit { pixels, max } => {
                write!(f, "image too large: {} pixels exceeds max {}", pixels, max)
            }
            Self::InvalidCropBounds { x, y, width, height, img_width, img_height } => {
                write!(
                    f,
                    "crop bounds ({}+{}, {}+{}) exceed image dimensions ({}x{})",
                    x, width, y, height, img_width, img_height
                )
            }
            Self::InvalidRotationAngle { degrees } => {
                write!(
                    f,
                    "unsupported rotation angle: {}. Only 0, 90, 180, 270 (and negatives) are supported",
                    degrees
                )
            }
            Self::UnsupportedColorSpace { color_space } => {
                write!(f, "unsupported color space: {}", color_space)
            }
            Self::InvalidPreset { name } => {
                write!(
                    f,
                    "unknown preset: '{}'. Available: thumbnail, avatar, hero, social",
                    name
                )
            }
            Self::EncodeFailed { format, reason } => {
                write!(f, "{} encode failed: {}", format, reason)
            }
            Self::DecodeFailed { reason } => {
                write!(f, "decode failed: {}", reason)
            }
            Self::Generic { message } => {
                write!(f, "{}", message)
            }
        }
    }
}

impl std::error::Error for LazyImageError {}

impl From<LazyImageError> for Error {
    fn from(err: LazyImageError) -> Self {
        Error::from_reason(err.to_string())
    }
}

/// Result type alias for lazy-image operations
pub type Result<T> = std::result::Result<T, LazyImageError>;
