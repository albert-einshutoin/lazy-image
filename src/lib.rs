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

use image::io::Reader as ImageReader;
use napi::bindgen_prelude::*;
use std::io::Cursor;

// Re-export the engine for NAPI
pub use engine::ImageEngine;

/// Image metadata returned by inspect()
#[napi(object)]
pub struct ImageMetadata {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Detected format (jpeg, png, webp, gif, etc.)
    pub format: Option<String>,
}

/// Inspect image metadata WITHOUT decoding pixels.
/// This reads only the header bytes - extremely fast (<1ms).
/// 
/// Use this to check dimensions before processing, or to reject
/// images that are too large without wasting CPU on decoding.
#[napi]
pub fn inspect(buffer: Buffer) -> Result<ImageMetadata> {
    let cursor = Cursor::new(buffer.as_ref());
    
    let reader = ImageReader::new(cursor)
        .with_guessed_format()
        .map_err(|e| Error::from_reason(format!("failed to read image header: {e}")))?;
    
    // Get format from header (no decoding)
    let format = reader.format().map(|f| format!("{:?}", f).to_lowercase());
    
    // Get dimensions from header (minimal decoding - just reads header bytes)
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| Error::from_reason(format!("failed to read dimensions: {e}")))?;
    
    Ok(ImageMetadata {
        width,
        height,
        format,
    })
}

/// Inspect image metadata from a file path WITHOUT loading into Node.js heap.
/// **Memory-efficient**: Reads directly from filesystem, bypassing V8 entirely.
/// This is the recommended way for server-side metadata inspection.
#[napi(js_name = "inspectFile")]
pub fn inspect_file(path: String) -> Result<ImageMetadata> {
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(&path)
        .map_err(|e| Error::from_reason(format!("failed to open file '{}': {}", path, e)))?;
    
    let reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|e| Error::from_reason(format!("failed to read image header: {e}")))?;
    
    // Get format from header (no decoding)
    let format = reader.format().map(|f| format!("{:?}", f).to_lowercase());
    
    // Get dimensions from header (minimal decoding - just reads header bytes)
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| Error::from_reason(format!("failed to read dimensions: {e}")))?;
    
    Ok(ImageMetadata {
        width,
        height,
        format,
    })
}

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
