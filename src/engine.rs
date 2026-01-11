// src/engine.rs
//
// The core of lazy-image. A lazy pipeline that:
// 1. Queues operations without executing
// 2. Runs everything in a single pass on compute()
// 3. Uses NAPI AsyncTask to not block Node.js main thread
//
// This file is now a facade that delegates to the decomposed modules in engine/

// =============================================================================
// SECURITY LIMITS
// =============================================================================

/// Maximum allowed image dimension (width or height).
/// Images larger than 32768x32768 are rejected to prevent decompression bombs.
/// This is the same limit used by libvips/sharp.
pub const MAX_DIMENSION: u32 = 32768;

/// Maximum allowed total pixels (width * height).
/// 100 megapixels = 400MB uncompressed RGBA. Beyond this is likely malicious.
pub const MAX_PIXELS: u64 = 100_000_000;

// =============================================================================
// MODULE DECOMPOSITION
// =============================================================================

// Import decomposed modules
#[cfg(feature = "napi")]
mod api;
mod decoder;
mod encoder;
mod io;
mod pipeline;
#[cfg(feature = "napi")]
mod pool;
#[cfg(feature = "napi")]
mod tasks;

// Re-export commonly used types and functions
#[cfg(feature = "napi")]
pub use api::{Dimensions, ImageEngine, PresetResult};
pub use decoder::{check_dimensions, decode_jpeg_mozjpeg};
pub use encoder::{encode_avif, encode_jpeg, encode_png, encode_webp, embed_icc_jpeg, embed_icc_png, embed_icc_webp, QualitySettings};
pub use io::{extract_icc_profile, Source};
pub use pipeline::{
    apply_ops, calc_resize_dimensions, fast_resize, fast_resize_internal, fast_resize_owned,
    optimize_ops, ResizeError,
};
#[cfg(feature = "napi")]
pub use tasks::BatchResult;

// Stress test function re-export
#[cfg(feature = "stress")]
pub use tasks::run_stress_iteration;

// Tests have been distributed to their respective modules:
// - engine/pipeline.rs: resize_calc_tests, apply_ops_tests, optimize_ops_tests, fast_resize_tests
// - engine/decoder.rs: security_tests, decode_tests
// - engine/encoder.rs: encode_tests
// - engine/io.rs: icc_tests
// - engine/api.rs: fast_resize_owned_returns_error_instead_of_dummy_image
