// src/engine/common.rs
//
// Common utilities shared across engine modules.
// Provides unified error handling and type aliases.

// LazyImageError is used in EngineResult type alias when NAPI is disabled
#[cfg(not(feature = "napi"))]
use crate::error::LazyImageError;

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;

/// Unified Result type that works with or without NAPI.
/// When NAPI is enabled, uses napi::Result.
/// When NAPI is disabled, uses std::result::Result<T, LazyImageError>.
#[cfg(feature = "napi")]
pub type EngineResult<T> = Result<T>;

#[cfg(not(feature = "napi"))]
pub type EngineResult<T> = std::result::Result<T, LazyImageError>;

// to_engine_error removed - it was unused.
// Each module (decoder, encoder, pipeline, tasks) has its own error conversion helper
// that matches its specific Result type (DecoderResult, EncoderResult, etc.).

/// Convert a Result that may be napi::Result or std::result::Result to EngineResult.
/// This macro helps eliminate duplicate cfg blocks in stress.rs.
#[macro_export]
macro_rules! convert_result {
    ($result:expr) => {{
        #[cfg(feature = "napi")]
        {
            $result.map_err(|e| crate::error::LazyImageError::decode_failed(e.to_string()))?
        }
        #[cfg(not(feature = "napi"))]
        {
            $result?
        }
    }};
}
