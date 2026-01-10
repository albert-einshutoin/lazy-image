// src/codecs/mod.rs
//
// Codec-specific safe abstractions for FFI operations.

#[cfg(feature = "napi")]
pub mod avif_safe;

#[cfg(not(feature = "napi"))]
pub mod avif_safe;
