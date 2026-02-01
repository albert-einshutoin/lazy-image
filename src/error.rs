// src/error.rs
//
// Unified error handling for lazy-image
// Uses thiserror for simple, type-safe error handling
//
// Error Taxonomy:
// - UserError: Invalid input, recoverable
// - CodecError: Format/encoding issues
// - ResourceLimit: Memory/time/dimension limits
// - InternalBug: Library bugs (should not happen)

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
use std::borrow::Cow;
use thiserror::Error;

/// Error taxonomy for proper error handling in JavaScript
///
/// This 4-tier taxonomy enables proper error handling:
/// - UserError: Invalid input, recoverable by user
/// - CodecError: Format/encoding issues
/// - ResourceLimit: Memory/time/dimension limits
/// - InternalBug: Library bugs (should not happen)
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "napi", napi)]
#[cfg_attr(not(feature = "napi"), derive(Clone, Copy))]
#[repr(u32)]
pub enum ErrorCategory {
    /// Invalid input, recoverable by user
    UserError,
    /// Format/encoding issues
    CodecError,
    /// Memory/time/dimension limits
    ResourceLimit,
    /// Library bugs (should not happen)
    InternalBug,
}

/// Fine-grained error codes for diagnostics and recovery guidance.
///
/// Ranges:
/// - E1xx: Input errors
/// - E2xx: Processing/operation errors
/// - E3xx: Output errors
/// - E4xx: Configuration errors
/// - E9xx: Internal errors
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "napi", napi)]
#[cfg_attr(not(feature = "napi"), derive(Copy, Clone))]
#[repr(u16)]
pub enum ErrorCode {
    // Input errors (E1xx)
    FileNotFound = 100,
    FileReadFailed = 101,
    MmapFailed = 102,
    UnsupportedFormat = 111,
    CorruptedImage = 130,
    DecodeFailed = 131,
    DimensionExceedsLimit = 121,
    PixelCountExceedsLimit = 122,
    FirewallViolation = 123,

    // Processing errors (E2xx)
    InvalidCropBounds = 200,
    InvalidCropDimensions = 201,
    InvalidRotationAngle = 202,
    InvalidResizeDimensions = 203,
    InvalidResizeFit = 204,
    UnsupportedColorSpace = 210,
    ResizeFailed = 299,

    // Output errors (E3xx)
    EncodeFailed = 300,
    FileWriteFailed = 301,

    // Configuration errors (E4xx)
    InvalidArgument = 400,
    InvalidPreset = 401,
    InvalidFirewallPolicy = 402,

    // Internal errors (E9xx)
    SourceConsumed = 900,
    InternalPanic = 901,
    Generic = 999,
}

impl ErrorCode {
    /// Return string literal representation (e.g., "E100")
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::FileNotFound => "E100",
            ErrorCode::FileReadFailed => "E101",
            ErrorCode::MmapFailed => "E102",
            ErrorCode::UnsupportedFormat => "E111",
            ErrorCode::CorruptedImage => "E130",
            ErrorCode::DecodeFailed => "E131",
            ErrorCode::DimensionExceedsLimit => "E121",
            ErrorCode::PixelCountExceedsLimit => "E122",
            ErrorCode::FirewallViolation => "E123",
            ErrorCode::InvalidCropBounds => "E200",
            ErrorCode::InvalidCropDimensions => "E201",
            ErrorCode::InvalidRotationAngle => "E202",
            ErrorCode::InvalidResizeDimensions => "E203",
            ErrorCode::InvalidResizeFit => "E204",
            ErrorCode::UnsupportedColorSpace => "E210",
            ErrorCode::ResizeFailed => "E299",
            ErrorCode::EncodeFailed => "E300",
            ErrorCode::FileWriteFailed => "E301",
            ErrorCode::InvalidArgument => "E400",
            ErrorCode::InvalidPreset => "E401",
            ErrorCode::InvalidFirewallPolicy => "E402",
            ErrorCode::SourceConsumed => "E900",
            ErrorCode::InternalPanic => "E901",
            ErrorCode::Generic => "E999",
        }
    }

    /// Map error code to error category.
    pub fn category(&self) -> ErrorCategory {
        match self {
            ErrorCode::FileNotFound => ErrorCategory::UserError,
            ErrorCode::FileReadFailed => ErrorCategory::ResourceLimit,
            ErrorCode::MmapFailed => ErrorCategory::ResourceLimit,
            ErrorCode::UnsupportedFormat => ErrorCategory::CodecError,
            ErrorCode::CorruptedImage => ErrorCategory::CodecError,
            ErrorCode::DecodeFailed => ErrorCategory::CodecError,
            ErrorCode::DimensionExceedsLimit => ErrorCategory::ResourceLimit,
            ErrorCode::PixelCountExceedsLimit => ErrorCategory::ResourceLimit,
            ErrorCode::FirewallViolation => ErrorCategory::ResourceLimit,
            ErrorCode::InvalidCropBounds => ErrorCategory::UserError,
            ErrorCode::InvalidCropDimensions => ErrorCategory::UserError,
            ErrorCode::InvalidRotationAngle => ErrorCategory::UserError,
            ErrorCode::InvalidResizeDimensions => ErrorCategory::UserError,
            ErrorCode::InvalidResizeFit => ErrorCategory::UserError,
            ErrorCode::UnsupportedColorSpace => ErrorCategory::CodecError,
            ErrorCode::ResizeFailed => ErrorCategory::CodecError,
            ErrorCode::EncodeFailed => ErrorCategory::CodecError,
            ErrorCode::FileWriteFailed => ErrorCategory::ResourceLimit,
            ErrorCode::InvalidArgument => ErrorCategory::UserError,
            ErrorCode::InvalidPreset => ErrorCategory::UserError,
            ErrorCode::InvalidFirewallPolicy => ErrorCategory::UserError,
            ErrorCode::SourceConsumed => ErrorCategory::UserError,
            ErrorCode::InternalPanic => ErrorCategory::InternalBug,
            ErrorCode::Generic => ErrorCategory::InternalBug,
        }
    }

    /// Whether this error is recoverable by the caller.
    pub fn is_recoverable(&self) -> bool {
        match self.category() {
            ErrorCategory::UserError | ErrorCategory::ResourceLimit => true,
            ErrorCategory::CodecError | ErrorCategory::InternalBug => false,
        }
    }
}

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

    #[error("Failed to memory-map file '{path}': {source}")]
    MmapFailed {
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

    #[error("Image Firewall blocked the image: {reason}")]
    FirewallViolation { reason: Cow<'static, str> },

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

    #[error("Invalid crop dimensions: width={width}, height={height}")]
    InvalidCropDimensions { width: u32, height: u32 },

    #[error(
        "Unsupported rotation angle: {degrees}. Only 0, 90, 180, 270 (and negatives) are supported"
    )]
    InvalidRotationAngle { degrees: i32 },

    #[error(
        "Invalid resize dimensions: width={width:?}, height={height:?}. Width/height must be between 1 and MAX_DIMENSION."
    )]
    InvalidResizeDimensions {
        width: Option<u32>,
        height: Option<u32>,
    },

    #[error("Invalid resize fit: '{value}'. Expected inside, cover, or fill")]
    InvalidResizeFit { value: Cow<'static, str> },

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

    #[error("Unknown firewall policy: '{policy}'. Expected strict or lenient")]
    InvalidFirewallPolicy { policy: Cow<'static, str> },

    #[error("Invalid value for {name}: {value}. {reason}")]
    InvalidArgument {
        name: Cow<'static, str>,
        value: Cow<'static, str>,
        reason: Cow<'static, str>,
    },

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

impl Clone for LazyImageError {
    fn clone(&self) -> Self {
        match self {
            Self::FileNotFound { path } => Self::FileNotFound { path: path.clone() },
            Self::FileReadFailed { path, source } => Self::FileReadFailed {
                path: path.clone(),
                source: std::io::Error::new(source.kind(), source.to_string()),
            },
            Self::MmapFailed { path, source } => Self::MmapFailed {
                path: path.clone(),
                source: std::io::Error::new(source.kind(), source.to_string()),
            },
            Self::FileWriteFailed { path, source } => Self::FileWriteFailed {
                path: path.clone(),
                source: std::io::Error::new(source.kind(), source.to_string()),
            },
            Self::UnsupportedFormat { format } => Self::UnsupportedFormat {
                format: format.clone(),
            },
            Self::DecodeFailed { message } => Self::DecodeFailed {
                message: message.clone(),
            },
            Self::CorruptedImage => Self::CorruptedImage,
            Self::DimensionExceedsLimit { dimension, max } => Self::DimensionExceedsLimit {
                dimension: *dimension,
                max: *max,
            },
            Self::PixelCountExceedsLimit { pixels, max } => Self::PixelCountExceedsLimit {
                pixels: *pixels,
                max: *max,
            },
            Self::FirewallViolation { reason } => Self::FirewallViolation {
                reason: reason.clone(),
            },
            Self::InvalidCropBounds {
                x,
                y,
                width,
                height,
                img_width,
                img_height,
            } => Self::InvalidCropBounds {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                img_width: *img_width,
                img_height: *img_height,
            },
            Self::InvalidCropDimensions { width, height } => Self::InvalidCropDimensions {
                width: *width,
                height: *height,
            },
            Self::InvalidRotationAngle { degrees } => {
                Self::InvalidRotationAngle { degrees: *degrees }
            }
            Self::InvalidResizeDimensions { width, height } => Self::InvalidResizeDimensions {
                width: *width,
                height: *height,
            },
            Self::InvalidResizeFit { value } => Self::InvalidResizeFit {
                value: value.clone(),
            },
            Self::ResizeFailed {
                source_width,
                source_height,
                target_width,
                target_height,
                message,
            } => Self::ResizeFailed {
                source_width: *source_width,
                source_height: *source_height,
                target_width: *target_width,
                target_height: *target_height,
                message: message.clone(),
            },
            Self::UnsupportedColorSpace { color_space } => Self::UnsupportedColorSpace {
                color_space: color_space.clone(),
            },
            Self::EncodeFailed { format, message } => Self::EncodeFailed {
                format: format.clone(),
                message: message.clone(),
            },
            Self::InvalidPreset { name } => Self::InvalidPreset { name: name.clone() },
            Self::InvalidFirewallPolicy { policy } => Self::InvalidFirewallPolicy {
                policy: policy.clone(),
            },
            Self::InvalidArgument {
                name,
                value,
                reason,
            } => Self::InvalidArgument {
                name: name.clone(),
                value: value.clone(),
                reason: reason.clone(),
            },
            Self::SourceConsumed => Self::SourceConsumed,
            Self::InternalPanic { message } => Self::InternalPanic {
                message: message.clone(),
            },
            Self::Generic { message } => Self::Generic {
                message: message.clone(),
            },
        }
    }
}

// Constructor Helpers
impl LazyImageError {
    /// Fine-grained error code for this error variant (E***)
    pub fn code(&self) -> ErrorCode {
        match self {
            // Input errors (E1xx)
            Self::FileNotFound { .. } => ErrorCode::FileNotFound,
            Self::FileReadFailed { .. } => ErrorCode::FileReadFailed,
            Self::MmapFailed { .. } => ErrorCode::MmapFailed,
            Self::UnsupportedFormat { .. } => ErrorCode::UnsupportedFormat,
            Self::CorruptedImage => ErrorCode::CorruptedImage,
            Self::DecodeFailed { .. } => ErrorCode::DecodeFailed,
            Self::DimensionExceedsLimit { .. } => ErrorCode::DimensionExceedsLimit,
            Self::PixelCountExceedsLimit { .. } => ErrorCode::PixelCountExceedsLimit,
            Self::FirewallViolation { .. } => ErrorCode::FirewallViolation,

            // Processing errors (E2xx)
            Self::InvalidCropBounds { .. } => ErrorCode::InvalidCropBounds,
            Self::InvalidCropDimensions { .. } => ErrorCode::InvalidCropDimensions,
            Self::InvalidRotationAngle { .. } => ErrorCode::InvalidRotationAngle,
            Self::InvalidResizeDimensions { .. } => ErrorCode::InvalidResizeDimensions,
            Self::InvalidResizeFit { .. } => ErrorCode::InvalidResizeFit,
            Self::UnsupportedColorSpace { .. } => ErrorCode::UnsupportedColorSpace,
            Self::ResizeFailed { .. } => ErrorCode::ResizeFailed,

            // Output errors (E3xx)
            Self::EncodeFailed { .. } => ErrorCode::EncodeFailed,
            Self::FileWriteFailed { .. } => ErrorCode::FileWriteFailed,

            // Configuration errors (E4xx)
            Self::InvalidArgument { .. } => ErrorCode::InvalidArgument,
            Self::InvalidPreset { .. } => ErrorCode::InvalidPreset,
            Self::InvalidFirewallPolicy { .. } => ErrorCode::InvalidFirewallPolicy,

            // Internal errors (E9xx)
            Self::SourceConsumed => ErrorCode::SourceConsumed,
            Self::InternalPanic { .. } => ErrorCode::InternalPanic,
            Self::Generic { .. } => ErrorCode::Generic,
        }
    }

    /// Short recovery guidance intended for end-user display or logs.
    pub fn recovery_hint(&self) -> &'static str {
        match self.code() {
            // Input errors
            ErrorCode::FileNotFound => "Verify the input path and ensure the file exists.",
            ErrorCode::FileReadFailed => "Check file permissions and disk health, then retry.",
            ErrorCode::MmapFailed => "Free memory or disk resources and confirm file permissions.",
            ErrorCode::UnsupportedFormat => {
                "Convert the image to a supported format (jpeg, png, webp)."
            }
            ErrorCode::CorruptedImage => {
                "Re-download or regenerate the image; the file appears corrupted."
            }
            ErrorCode::DecodeFailed => {
                "Try opening the file in another viewer or re-encode the source image."
            }
            ErrorCode::DimensionExceedsLimit => {
                "Resize the image to fit within the maximum dimension limits."
            }
            ErrorCode::PixelCountExceedsLimit => {
                "Reduce resolution or process the image in smaller tiles."
            }
            ErrorCode::FirewallViolation => {
                "Adjust firewall limits (bytes/pixels/metadata) or use smaller inputs."
            }

            // Processing errors
            ErrorCode::InvalidCropBounds => {
                "Ensure crop x/y/width/height stay within the image dimensions."
            }
            ErrorCode::InvalidCropDimensions => {
                "Use positive crop width and height greater than zero."
            }
            ErrorCode::InvalidRotationAngle => {
                "Use rotation angles in 90-degree increments (0, 90, 180, 270)."
            }
            ErrorCode::InvalidResizeDimensions => {
                "Provide at least one positive dimension (width or height)."
            }
            ErrorCode::InvalidResizeFit => "Use fit values: inside, cover, or fill.",
            ErrorCode::UnsupportedColorSpace => {
                "Convert the image to sRGB or a supported color space before processing."
            }
            ErrorCode::ResizeFailed => {
                "Try different resize parameters or re-encode the input before resizing."
            }

            // Output errors
            ErrorCode::EncodeFailed => {
                "Adjust output format/quality or try re-encoding the source image."
            }
            ErrorCode::FileWriteFailed => {
                "Check output path, permissions, and available disk space."
            }

            // Configuration errors
            ErrorCode::InvalidArgument => {
                "Pass a valid argument value as documented for this option."
            }
            ErrorCode::InvalidPreset => {
                "Use a supported preset name (thumbnail, avatar, hero, social)."
            }
            ErrorCode::InvalidFirewallPolicy => "Use firewall policy 'strict' or 'lenient'.",

            // Internal errors
            ErrorCode::SourceConsumed => "Clone the engine or reload the source before reusing it.",
            ErrorCode::InternalPanic => {
                "Report this issue with logs; it indicates an unexpected internal error."
            }
            ErrorCode::Generic => {
                "Report this issue with full context; an unexpected state occurred."
            }
        }
    }

    pub fn file_not_found(path: impl Into<Cow<'static, str>>) -> Self {
        Self::FileNotFound { path: path.into() }
    }

    pub fn file_read_failed(path: impl Into<Cow<'static, str>>, source: std::io::Error) -> Self {
        Self::FileReadFailed {
            path: path.into(),
            source,
        }
    }

    pub fn mmap_failed(path: impl Into<Cow<'static, str>>, source: std::io::Error) -> Self {
        Self::MmapFailed {
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

    pub fn firewall_violation(reason: impl Into<Cow<'static, str>>) -> Self {
        Self::FirewallViolation {
            reason: reason.into(),
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
            x,
            y,
            width,
            height,
            img_width,
            img_height,
        }
    }

    pub fn invalid_crop_dimensions(width: u32, height: u32) -> Self {
        Self::InvalidCropDimensions { width, height }
    }

    pub fn invalid_rotation_angle(degrees: i32) -> Self {
        Self::InvalidRotationAngle { degrees }
    }

    pub fn invalid_resize_dimensions(width: Option<u32>, height: Option<u32>) -> Self {
        Self::InvalidResizeDimensions { width, height }
    }

    pub fn invalid_resize_fit(value: impl Into<Cow<'static, str>>) -> Self {
        Self::InvalidResizeFit {
            value: value.into(),
        }
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
        Self::InvalidPreset { name: name.into() }
    }

    pub fn invalid_firewall_policy(policy: impl Into<Cow<'static, str>>) -> Self {
        Self::InvalidFirewallPolicy {
            policy: policy.into(),
        }
    }

    pub fn invalid_argument(
        name: impl Into<Cow<'static, str>>,
        value: impl Into<Cow<'static, str>>,
        reason: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::InvalidArgument {
            name: name.into(),
            value: value.into(),
            reason: reason.into(),
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

    /// Check if this error is recoverable (user can fix it)
    ///
    /// This method is consistent with category():
    /// - UserError errors are always recoverable
    /// - ResourceLimit errors are recoverable (user can free resources, resize image, etc.)
    /// - CodecError and InternalBug errors are not recoverable
    pub fn is_recoverable(&self) -> bool {
        match self.category() {
            ErrorCategory::UserError | ErrorCategory::ResourceLimit => true,
            ErrorCategory::CodecError | ErrorCategory::InternalBug => false,
        }
    }

    /// Get the error category for this error
    pub fn category(&self) -> ErrorCategory {
        match self {
            // UserError: Invalid input, recoverable
            Self::FileNotFound { .. }
            | Self::InvalidCropBounds { .. }
            | Self::InvalidCropDimensions { .. }
            | Self::InvalidRotationAngle { .. }
            | Self::InvalidResizeDimensions { .. }
            | Self::InvalidResizeFit { .. }
            | Self::InvalidPreset { .. }
            | Self::InvalidFirewallPolicy { .. }
            | Self::InvalidArgument { .. }
            | Self::SourceConsumed => ErrorCategory::UserError,

            // CodecError: Format/encoding issues
            Self::UnsupportedFormat { .. }
            | Self::DecodeFailed { .. }
            | Self::CorruptedImage
            | Self::EncodeFailed { .. }
            | Self::UnsupportedColorSpace { .. }
            // Note: ResizeFailed is classified as CodecError because it represents
            // a processing failure during image transformation, which is similar to
            // encoding/decoding issues. In a future version, a ProcessingError category
            // might be more appropriate.
            | Self::ResizeFailed { .. } => ErrorCategory::CodecError,

            // ResourceLimit: Memory/time/dimension limits
            // Note: FileReadFailed/MmapFailed/FileWriteFailed are classified as ResourceLimit
            // because they often indicate resource constraints (disk full, memory pressure,
            // file system limits). However, they can also represent I/O errors (permissions,
            // file locks, etc.). These errors are recoverable by the user (fixing permissions,
            // freeing disk space, etc.), which is consistent with is_recoverable() returning true.
            Self::DimensionExceedsLimit { .. }
            | Self::PixelCountExceedsLimit { .. }
            | Self::FirewallViolation { .. }
            | Self::FileReadFailed { .. }
            | Self::MmapFailed { .. }
            | Self::FileWriteFailed { .. } => ErrorCategory::ResourceLimit,

            // InternalBug: Library bugs (should not happen)
            Self::InternalPanic { .. }
            | Self::Generic { .. } => ErrorCategory::InternalBug,
        }
    }
}

/// Helper function to create NAPI error with category code
/// This allows JavaScript code to access error.code (e.g., "LAZY_IMAGE_USER_ERROR")
///
/// This function should be used when Env is available to add custom properties.
/// For code that doesn't have Env, use the From<LazyImageError> for napi::Error implementation.
#[cfg(feature = "napi")]
pub fn create_napi_error_with_code(env: &Env, err: LazyImageError) -> napi::Result<napi::JsObject> {
    let category = err.category();
    let error_code = err.code();

    // Create error object with original message (no prefix to avoid breaking changes)
    // Use create_error with message string directly to avoid Status prefix in message
    let err_msg = err.to_string();
    let message_with_code = format!("[{}] {}", error_code.as_str(), err_msg);
    // Create error with clean message (Status will be added by napi::Error::new, but we'll override it)
    let mut error_obj = env.create_error(napi::Error::new(
        match category {
            ErrorCategory::UserError => Status::InvalidArg,
            ErrorCategory::CodecError => Status::InvalidArg,
            ErrorCategory::ResourceLimit => Status::GenericFailure,
            ErrorCategory::InternalBug => Status::GenericFailure,
        },
        message_with_code.clone(),
    ))?;

    // Override message property to ensure clean message (with code prefix, without Status)
    // napi::Error::new() may include Status in message, so we set message property directly
    error_obj.set_named_property("message", env.create_string(&message_with_code)?)?;

    // Add error.code property (category-level, backward compatible)
    let code_value = env.create_string(category.code())?;
    error_obj.set_named_property("code", code_value)?;

    // Add error.errorCode property (fine-grained classification like E200)
    let error_code_value = env.create_string(error_code.as_str())?;
    error_obj.set_named_property("errorCode", error_code_value)?;

    // Add error.category property (ErrorCategory enum value as number)
    // Use #[repr(u32)] to get the enum value directly
    let category_value = env.create_uint32(category as u32)?;
    error_obj.set_named_property("category", category_value)?;

    // Add recoveryHint property for user-facing guidance
    let recovery_hint = env.create_string(err.recovery_hint())?;
    error_obj.set_named_property("recoveryHint", recovery_hint)?;

    Ok(error_obj)
}

/// Helper function to convert LazyImageError to napi::Error with code/category
/// The returned napi::Error references a JsError object which already includes
/// the structured properties, so callers can simply `return Err(...)`.
#[cfg(feature = "napi")]
pub fn napi_error_with_code(env: &Env, err: LazyImageError) -> napi::Result<napi::Error> {
    let error_obj = create_napi_error_with_code(env, err)?;
    let js_unknown = error_obj.into_unknown();
    Ok(napi::Error::from(js_unknown))
}

// Conversion to NAPI Error (fallback - should not be used when Env is available)
// Note: This creates a basic error without error.code/category properties.
// Use create_napi_error_with_code() or napi_error_with_code() when Env is available for proper error handling.
#[cfg(feature = "napi")]
impl From<LazyImageError> for napi::Error {
    fn from(err: LazyImageError) -> Self {
        let category = err.category();
        let code = err.code();
        let status = match category {
            ErrorCategory::UserError => Status::InvalidArg,
            ErrorCategory::CodecError => Status::InvalidArg,
            ErrorCategory::ResourceLimit => Status::GenericFailure,
            ErrorCategory::InternalBug => Status::GenericFailure,
        };

        // Fallback conversion without Env: embed fine-grained code in message so
        // JavaScript can still classify errors (parseable "[E***]" prefix).
        let message_with_code = format!("[{}] {}", code.as_str(), err);
        napi::Error::new(status, message_with_code)
    }
}

// Note: From<napi::Error> for LazyImageError is no longer needed
// because decoder.rs and pipeline.rs now return LazyImageError directly
// instead of napi::Error. This preserves error taxonomy (CodecError, ResourceLimit, etc.)
// instead of converting everything to generic InternalBug errors.

#[cfg(feature = "napi")]
impl ErrorCategory {
    /// Get string representation of error category
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCategory::UserError => "UserError",
            ErrorCategory::CodecError => "CodecError",
            ErrorCategory::ResourceLimit => "ResourceLimit",
            ErrorCategory::InternalBug => "InternalBug",
        }
    }

    /// Get the LAZY_IMAGE_* error code string for this category
    pub fn code(&self) -> &'static str {
        match self {
            ErrorCategory::UserError => "LAZY_IMAGE_USER_ERROR",
            ErrorCategory::CodecError => "LAZY_IMAGE_CODEC_ERROR",
            ErrorCategory::ResourceLimit => "LAZY_IMAGE_RESOURCE_LIMIT",
            ErrorCategory::InternalBug => "LAZY_IMAGE_INTERNAL_BUG",
        }
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

    #[cfg(feature = "napi")]
    #[test]
    fn test_error_category_as_str() {
        assert_eq!(ErrorCategory::UserError.as_str(), "UserError");
        assert_eq!(ErrorCategory::CodecError.as_str(), "CodecError");
        assert_eq!(ErrorCategory::ResourceLimit.as_str(), "ResourceLimit");
        assert_eq!(ErrorCategory::InternalBug.as_str(), "InternalBug");
    }
}
