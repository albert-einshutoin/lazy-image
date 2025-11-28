// lib.rs
//
// lazy-image: A next-generation image processing engine for Node.js
//
// Design goals:
// - Faster than sharp
// - Smaller output than sharp  
// - Better quality than sharp
// - Lazy pipeline execution
// - Non-blocking async API

#[macro_use]
extern crate napi_derive;

mod engine;
mod ops;

// Re-export the engine for NAPI
pub use engine::ImageEngine;

/// Get library version
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get supported input formats
#[napi]
pub fn supported_input_formats() -> Vec<String> {
    vec![
        "jpeg".to_string(),
        "jpg".to_string(),
        "png".to_string(),
        "webp".to_string(),
    ]
}

/// Get supported output formats
#[napi]
pub fn supported_output_formats() -> Vec<String> {
    vec![
        "jpeg".to_string(),
        "jpg".to_string(),
        "png".to_string(),
        "webp".to_string(),
        "avif".to_string(),
    ]
}
