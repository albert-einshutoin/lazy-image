// src/error.rs
//
// Unified error handling for lazy-image
// Uses thiserror for type-safe error handling with error codes

use thiserror::Error;

/// lazy-image エラーコード体系
/// E1xx: 入力エラー
/// E2xx: 処理エラー
/// E3xx: 出力エラー
/// E4xx: 設定エラー
/// E9xx: 内部エラー
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // 入力エラー (E1xx)
    FileNotFound = 100,
    FileReadFailed = 101,
    InvalidImageFormat = 110,
    UnsupportedFormat = 111,
    ImageTooLarge = 120,
    DimensionExceedsLimit = 121,
    PixelCountExceedsLimit = 122,
    CorruptedImage = 130,
    DecodeFailed = 131,

    // 処理エラー (E2xx)
    InvalidCropBounds = 200,
    InvalidRotationAngle = 201,
    InvalidResizeDimensions = 202,
    UnsupportedColorSpace = 210,
    OperationFailed = 299,

    // 出力エラー (E3xx)
    EncodeFailed = 300,
    FileWriteFailed = 301,
    OutputPathInvalid = 302,

    // 設定エラー (E4xx)
    InvalidQuality = 400,
    InvalidPreset = 401,

    // 内部エラー (E9xx)
    SourceConsumed = 900,
    InternalPanic = 901,
    UnexpectedState = 999,
}

impl ErrorCode {
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
}

#[derive(Debug, Error)]
pub enum LazyImageError {
    // ファイルI/Oエラー
    #[error("[{code}] File not found: {path}")]
    FileNotFound {
        code: &'static str,
        path: String,
    },

    #[error("[{code}] Failed to read file '{path}': {source}")]
    FileReadFailed {
        code: &'static str,
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("[{code}] Failed to write file '{path}': {source}")]
    FileWriteFailed {
        code: &'static str,
        path: String,
        #[source]
        source: std::io::Error,
    },

    // デコードエラー
    #[error("[{code}] Unsupported image format: {format}")]
    UnsupportedFormat {
        code: &'static str,
        format: String,
    },

    #[error("[{code}] Failed to decode image: {message}")]
    DecodeFailed {
        code: &'static str,
        message: String,
    },

    #[error("[{code}] Corrupted image data")]
    CorruptedImage {
        code: &'static str,
    },

    // サイズ制限エラー
    #[error("[{code}] Image dimension {dimension} exceeds maximum {max}")]
    DimensionExceedsLimit {
        code: &'static str,
        dimension: u32,
        max: u32,
    },

    #[error("[{code}] Image pixel count {pixels} exceeds maximum {max}")]
    PixelCountExceedsLimit {
        code: &'static str,
        pixels: u64,
        max: u64,
    },

    // 操作エラー
    #[error("[{code}] Crop bounds ({x}+{width}, {y}+{height}) exceed image dimensions ({img_width}x{img_height})")]
    InvalidCropBounds {
        code: &'static str,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        img_width: u32,
        img_height: u32,
    },

    #[error("[{code}] Unsupported rotation angle: {degrees}. Only 0, 90, 180, 270 (and negatives) are supported")]
    InvalidRotationAngle {
        code: &'static str,
        degrees: i32,
    },

    #[error("[{code}] Invalid resize dimensions: width={width:?}, height={height:?}")]
    InvalidResizeDimensions {
        code: &'static str,
        width: Option<u32>,
        height: Option<u32>,
    },

    #[error("[{code}] Unsupported color space: {color_space}")]
    UnsupportedColorSpace {
        code: &'static str,
        color_space: String,
    },

    // エンコードエラー
    #[error("[{code}] Failed to encode as {format}: {message}")]
    EncodeFailed {
        code: &'static str,
        format: String,
        message: String,
    },

    // 設定エラー
    #[error("[{code}] Unknown preset: '{name}'. Available: thumbnail, avatar, hero, social")]
    InvalidPreset {
        code: &'static str,
        name: String,
    },

    // 状態エラー
    #[error("[{code}] Image source already consumed. Use clone() for multi-output scenarios")]
    SourceConsumed {
        code: &'static str,
    },

    // 内部エラー
    #[error("[{code}] Internal error: {message}")]
    InternalPanic {
        code: &'static str,
        message: String,
    },

    // 汎用エラー（mainブランチから追加）
    #[error("[{code}] {message}")]
    Generic {
        code: &'static str,
        message: String,
    },
}

// コンストラクタヘルパー
impl LazyImageError {
    pub fn file_not_found(path: impl Into<String>) -> Self {
        Self::FileNotFound {
            code: ErrorCode::FileNotFound.as_str(),
            path: path.into(),
        }
    }

    pub fn file_read_failed(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::FileReadFailed {
            code: ErrorCode::FileReadFailed.as_str(),
            path: path.into(),
            source,
        }
    }

    pub fn file_write_failed(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::FileWriteFailed {
            code: ErrorCode::FileWriteFailed.as_str(),
            path: path.into(),
            source,
        }
    }

    pub fn unsupported_format(format: impl Into<String>) -> Self {
        Self::UnsupportedFormat {
            code: ErrorCode::UnsupportedFormat.as_str(),
            format: format.into(),
        }
    }

    pub fn decode_failed(message: impl Into<String>) -> Self {
        Self::DecodeFailed {
            code: ErrorCode::DecodeFailed.as_str(),
            message: message.into(),
        }
    }

    pub fn corrupted_image() -> Self {
        Self::CorruptedImage {
            code: ErrorCode::CorruptedImage.as_str(),
        }
    }

    pub fn dimension_exceeds_limit(dimension: u32, max: u32) -> Self {
        Self::DimensionExceedsLimit {
            code: ErrorCode::DimensionExceedsLimit.as_str(),
            dimension,
            max,
        }
    }

    pub fn pixel_count_exceeds_limit(pixels: u64, max: u64) -> Self {
        Self::PixelCountExceedsLimit {
            code: ErrorCode::PixelCountExceedsLimit.as_str(),
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
            code: ErrorCode::InvalidCropBounds.as_str(),
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
            code: ErrorCode::InvalidRotationAngle.as_str(),
            degrees,
        }
    }

    pub fn invalid_resize_dimensions(width: Option<u32>, height: Option<u32>) -> Self {
        Self::InvalidResizeDimensions {
            code: ErrorCode::InvalidResizeDimensions.as_str(),
            width,
            height,
        }
    }

    pub fn unsupported_color_space(color_space: impl Into<String>) -> Self {
        Self::UnsupportedColorSpace {
            code: ErrorCode::UnsupportedColorSpace.as_str(),
            color_space: color_space.into(),
        }
    }

    pub fn encode_failed(format: impl Into<String>, message: impl Into<String>) -> Self {
        Self::EncodeFailed {
            code: ErrorCode::EncodeFailed.as_str(),
            format: format.into(),
            message: message.into(),
        }
    }

    pub fn invalid_preset(name: impl Into<String>) -> Self {
        Self::InvalidPreset {
            code: ErrorCode::InvalidPreset.as_str(),
            name: name.into(),
        }
    }

    pub fn source_consumed() -> Self {
        Self::SourceConsumed {
            code: ErrorCode::SourceConsumed.as_str(),
        }
    }

    pub fn internal_panic(message: impl Into<String>) -> Self {
        Self::InternalPanic {
            code: ErrorCode::InternalPanic.as_str(),
            message: message.into(),
        }
    }

    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic {
            code: ErrorCode::UnexpectedState.as_str(),
            message: message.into(),
        }
    }

    /// エラーコードを取得
    pub fn code(&self) -> &'static str {
        match self {
            Self::FileNotFound { code, .. } => code,
            Self::FileReadFailed { code, .. } => code,
            Self::FileWriteFailed { code, .. } => code,
            Self::UnsupportedFormat { code, .. } => code,
            Self::DecodeFailed { code, .. } => code,
            Self::CorruptedImage { code } => code,
            Self::DimensionExceedsLimit { code, .. } => code,
            Self::PixelCountExceedsLimit { code, .. } => code,
            Self::InvalidCropBounds { code, .. } => code,
            Self::InvalidRotationAngle { code, .. } => code,
            Self::InvalidResizeDimensions { code, .. } => code,
            Self::UnsupportedColorSpace { code, .. } => code,
            Self::EncodeFailed { code, .. } => code,
            Self::InvalidPreset { code, .. } => code,
            Self::SourceConsumed { code } => code,
            Self::InternalPanic { code, .. } => code,
            Self::Generic { code, .. } => code,
        }
    }
}

// NAPIエラーへの変換
impl From<LazyImageError> for napi::Error {
    fn from(err: LazyImageError) -> Self {
        napi::Error::from_reason(err.to_string())
    }
}

// Result型エイリアス
pub type Result<T> = std::result::Result<T, LazyImageError>;
