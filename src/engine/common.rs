// src/engine/common.rs
//
// Common utilities shared across engine modules.
// Provides unified error handling and type aliases.

use crate::error::LazyImageError;

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};

/// Unified Result type that works with or without NAPI.
/// When NAPI is enabled, uses napi::Result.
/// When NAPI is disabled, uses std::result::Result<T, LazyImageError>.
// Only needed by stress tooling; avoid compiling in non-stress builds to prevent dead_code warnings.
#[cfg(all(feature = "stress", feature = "napi"))]
pub type EngineResult<T> = napi::Result<T>;

#[cfg(all(feature = "stress", not(feature = "napi")))]
pub type EngineResult<T> = std::result::Result<T, LazyImageError>;

// to_engine_error removed - it was unused.
// Each module (decoder, encoder, pipeline, tasks) has its own error conversion helper
// that matches its specific Result type (DecoderResult, EncoderResult, etc.).

/// Panic policy helper for codec operations.
///
/// All decode/encode entry points must wrap their core logic with this helper so
/// that panics from third-party libraries (mozjpeg, image, libavif, etc.) are
/// downgraded to `LazyImageError::InternalPanic` instead of aborting the
/// process. This enforces the "panics → InternalBug" rule described in the
/// panic policy.
pub fn run_with_panic_policy<F, T>(
    context: &'static str,
    op: F,
) -> std::result::Result<T, LazyImageError>
where
    F: FnOnce() -> std::result::Result<T, LazyImageError>,
{
    match panic::catch_unwind(AssertUnwindSafe(op)) {
        Ok(result) => result,
        Err(payload) => Err(LazyImageError::internal_panic(format!(
            "{context}: {}",
            panic_payload_message(payload.as_ref())
        ))),
    }
}

fn panic_payload_message(payload: &(dyn Any + Send + 'static)) -> String {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        msg.to_string()
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        msg.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

/// Convert a Result that may be napi::Result or std::result::Result to EngineResult.
/// This macro helps eliminate duplicate cfg blocks in stress.rs.
#[macro_export]
macro_rules! convert_result {
    ($result:expr) => {{
        #[cfg(feature = "napi")]
        {
            $result.map_err(|e| $crate::error::LazyImageError::decode_failed(e.to_string()))?
        }
        #[cfg(not(feature = "napi"))]
        {
            $result?
        }
    }};
}
