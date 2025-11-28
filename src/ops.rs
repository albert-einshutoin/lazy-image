// src/ops.rs
//
// Lazy pipeline operations.
// These are cheap to create and store - the expensive work happens in compute().

/// Image operations that can be queued for lazy execution.
///
/// Design principle: each operation is self-contained and stateless.
/// No references, no lifetimes, no bullshit.
#[derive(Clone, Debug)]
pub enum Operation {
    /// Resize with optional width/height (maintains aspect ratio if one is None)
    Resize { width: Option<u32>, height: Option<u32> },

    /// Crop a region from the image
    Crop { x: u32, y: u32, width: u32, height: u32 },

    /// Rotate by 90, 180, or 270 degrees
    Rotate { degrees: i32 },

    /// Flip horizontally
    FlipH,

    /// Flip vertically
    FlipV,

    /// Adjust brightness (-100 to 100)
    Brightness { value: i32 },

    /// Adjust contrast (-100 to 100)
    Contrast { value: i32 },

    /// Grayscale conversion
    Grayscale,
}

/// Output format for encoding
#[derive(Clone, Debug)]
pub enum OutputFormat {
    Jpeg { quality: u8 },
    Png,
    WebP { quality: u8 },
    Avif { quality: u8 },
}

impl OutputFormat {
    pub fn from_str(format: &str, quality: Option<u8>) -> Result<Self, String> {
        let q = quality.unwrap_or(80);
        match format.to_lowercase().as_str() {
            "jpeg" | "jpg" => Ok(Self::Jpeg { quality: q }),
            "png" => Ok(Self::Png),
            "webp" => Ok(Self::WebP { quality: q }),
            "avif" => Ok(Self::Avif { quality: q }),
            other => Err(format!("unsupported format: {other}")),
        }
    }
}
