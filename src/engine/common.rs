// src/engine/common.rs
//
// Common utilities shared across engine modules.
// Provides unified error handling and type aliases.

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

/// Convert any error to LazyImageError.
/// This helper handles both napi::Error (when NAPI is enabled) and LazyImageError.
#[cfg(feature = "napi")]
pub fn to_engine_error(err: impl std::error::Error) -> LazyImageError {
    // If it's already a LazyImageError wrapped in napi::Error, extract it
    // Otherwise, convert the error message to LazyImageError
    LazyImageError::decode_failed(err.to_string())
}

#[cfg(not(feature = "napi"))]
pub fn to_engine_error(err: LazyImageError) -> LazyImageError {
    err
}

/// Convert a Result that may be napi::Result or std::result::Result to EngineResult.
/// This macro helps eliminate duplicate cfg blocks in stress.rs.
#[macro_export]
macro_rules! convert_result {
    ($result:expr) => {
        {
            #[cfg(feature = "napi")]
            {
                $result.map_err(|e| {
                    crate::error::LazyImageError::decode_failed(e.to_string())
                })?
            }
            #[cfg(not(feature = "napi"))]
            {
                $result?
            }
        }
    };
}
