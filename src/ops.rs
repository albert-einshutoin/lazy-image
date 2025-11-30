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

    /// Color space conversion (currently supports basic RGB/RGBA assurance)
    ColorSpace { target: ColorSpace },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColorSpace {
    Srgb,
    DisplayP3, // Placeholder
    AdobeRgb,  // Placeholder
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

// =============================================================================
// PRESETS - Common configurations for web image optimization
// =============================================================================

/// Preset configuration for common use cases.
/// Each preset defines optimal settings for a specific purpose.
#[derive(Clone, Debug)]
pub struct PresetConfig {
    /// Target width (None = maintain aspect ratio)
    pub width: Option<u32>,
    /// Target height (None = maintain aspect ratio)
    pub height: Option<u32>,
    /// Output format
    pub format: OutputFormat,
}

impl PresetConfig {
    /// Create a new preset configuration
    pub fn new(width: Option<u32>, height: Option<u32>, format: OutputFormat) -> Self {
        Self { width, height, format }
    }

    /// Get the built-in preset by name
    pub fn get(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "thumbnail" => Some(Self::thumbnail()),
            "avatar" => Some(Self::avatar()),
            "hero" => Some(Self::hero()),
            "social" => Some(Self::social()),
            _ => None,
        }
    }

    /// Thumbnail preset: 150x150, WebP quality 75
    /// Use case: Gallery thumbnails, preview images
    pub fn thumbnail() -> Self {
        Self::new(Some(150), Some(150), OutputFormat::WebP { quality: 75 })
    }

    /// Avatar preset: 200x200, WebP quality 80
    /// Use case: User profile pictures
    pub fn avatar() -> Self {
        Self::new(Some(200), Some(200), OutputFormat::WebP { quality: 80 })
    }

    /// Hero preset: 1920 width, JPEG quality 85
    /// Use case: Hero images, banners
    pub fn hero() -> Self {
        Self::new(Some(1920), None, OutputFormat::Jpeg { quality: 85 })
    }

    /// Social preset: 1200x630, JPEG quality 80
    /// Use case: OGP/Twitter Card images
    pub fn social() -> Self {
        Self::new(Some(1200), Some(630), OutputFormat::Jpeg { quality: 80 })
    }
}
