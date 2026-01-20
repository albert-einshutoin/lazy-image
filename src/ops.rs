// src/ops.rs
//
// Lazy pipeline operations.
// These are cheap to create and store - the expensive work happens in compute().

use std::str::FromStr;

/// Image operations that can be queued for lazy execution.
///
/// Design principle: each operation is self-contained and stateless.
/// No references, no lifetimes, no bullshit.
#[derive(Clone, Debug)]
pub enum Operation {
    /// Resize with optional width/height.
    /// The behavior when both width/height are provided depends on the fit mode.
    Resize {
        width: Option<u32>,
        height: Option<u32>,
        fit: ResizeFit,
    },

    /// Crop a region from the image
    Crop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },

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

    /// Auto-orient based on EXIF Orientation tag (1-8)
    AutoOrient { orientation: u16 },

    /// Grayscale conversion
    Grayscale,

    /// Ensure RGB/RGBA pixel format (not true color space conversion)
    /// This operation only normalizes pixel format, not color space transformation.
    ColorSpace { target: ColorSpace },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColorSpace {
    /// sRGB format (RGB/RGBA pixel format normalization)
    Srgb,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResizeFit {
    /// Maintain aspect ratio while fitting inside the bounding box (default)
    Inside,
    /// Maintain aspect ratio but ensure the bounding box is fully covered (may crop)
    Cover,
    /// Ignore aspect ratio and force exact dimensions
    Fill,
}

impl Default for ResizeFit {
    fn default() -> Self {
        ResizeFit::Inside
    }
}

impl FromStr for ResizeFit {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "inside" => Ok(ResizeFit::Inside),
            "cover" => Ok(ResizeFit::Cover),
            "fill" => Ok(ResizeFit::Fill),
            other => Err(format!(
                "unknown resize fit '{other}'. Expected inside, cover, or fill"
            )),
        }
    }
}

/// Output format for encoding
#[derive(Clone, Debug)]
pub enum OutputFormat {
    Jpeg { quality: u8, fast_mode: bool },
    Png,
    WebP { quality: u8 },
    Avif { quality: u8 },
}

impl OutputFormat {
    /// Create OutputFormat from string with format-specific default quality.
    ///
    /// Default quality by format (when quality is None):
    /// - JPEG: 85 (high quality, balanced file size)
    /// - WebP: 80 (optimal for WebP's compression characteristics)
    /// - AVIF: 60 (AVIF's high compression efficiency means lower quality still looks great)
    ///
    /// These defaults are chosen based on each format's characteristics and real-world usage.
    pub fn from_str(format: &str, quality: Option<u8>) -> Result<Self, String> {
        Self::from_str_with_options(format, quality, false)
    }

    /// Create OutputFormat from string with format-specific default quality and fast mode option.
    ///
    /// # Arguments
    /// * `format` - Output format string (jpeg, png, webp, avif)
    /// * `quality` - Quality value (0-100, None uses format-specific default)
    /// * `fast_mode` - Fast mode flag (only applies to JPEG, default: false)
    pub fn from_str_with_options(
        format: &str,
        quality: Option<u8>,
        fast_mode: bool,
    ) -> Result<Self, String> {
        match format.to_lowercase().as_str() {
            "jpeg" | "jpg" => {
                let q = quality.unwrap_or(85); // JPEG default: 85
                Ok(Self::Jpeg {
                    quality: q,
                    fast_mode,
                })
            }
            "png" => Ok(Self::Png),
            "webp" => {
                let q = quality.unwrap_or(80); // WebP default: 80
                Ok(Self::WebP { quality: q })
            }
            "avif" => {
                let q = quality.unwrap_or(60); // AVIF default: 60 (high compression efficiency)
                Ok(Self::Avif { quality: q })
            }
            other => Err(format!("unsupported format: {other}")),
        }
    }

    /// Return canonical lowercase string for telemetry/export
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Jpeg { .. } => "jpeg",
            OutputFormat::Png => "png",
            OutputFormat::WebP { .. } => "webp",
            OutputFormat::Avif { .. } => "avif",
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
        Self {
            width,
            height,
            format,
        }
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
        Self::new(
            Some(1920),
            None,
            OutputFormat::Jpeg {
                quality: 85,
                fast_mode: false,
            },
        )
    }

    /// Social preset: 1200x630, JPEG quality 80
    /// Use case: OGP/Twitter Card images
    pub fn social() -> Self {
        Self::new(
            Some(1200),
            Some(630),
            OutputFormat::Jpeg {
                quality: 80,
                fast_mode: false,
            },
        )
    }
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    mod output_format_tests {
        use super::*;

        #[test]
        fn test_jpeg_with_quality() {
            let format = OutputFormat::from_str("jpeg", Some(90)).unwrap();
            assert!(matches!(format, OutputFormat::Jpeg { quality: 90, .. }));
        }

        #[test]
        fn test_jpeg_default_quality() {
            let format = OutputFormat::from_str("jpeg", None).unwrap();
            assert!(matches!(format, OutputFormat::Jpeg { quality: 85, .. }));
        }

        #[test]
        fn test_jpg_alias() {
            let format = OutputFormat::from_str("jpg", None).unwrap();
            assert!(matches!(format, OutputFormat::Jpeg { quality: 85, .. }));
        }

        #[test]
        fn test_jpg_with_quality() {
            let format = OutputFormat::from_str("jpg", Some(75)).unwrap();
            assert!(matches!(format, OutputFormat::Jpeg { quality: 75, .. }));
        }

        #[test]
        fn test_webp_with_quality() {
            let format = OutputFormat::from_str("webp", Some(90)).unwrap();
            assert!(matches!(format, OutputFormat::WebP { quality: 90 }));
        }

        #[test]
        fn test_webp_default_quality() {
            let format = OutputFormat::from_str("webp", None).unwrap();
            assert!(matches!(format, OutputFormat::WebP { quality: 80 }));
        }

        #[test]
        fn test_avif_with_quality() {
            let format = OutputFormat::from_str("avif", Some(70)).unwrap();
            assert!(matches!(format, OutputFormat::Avif { quality: 70 }));
        }

        #[test]
        fn test_avif_default_quality() {
            let format = OutputFormat::from_str("avif", None).unwrap();
            assert!(matches!(format, OutputFormat::Avif { quality: 60 }));
        }

        #[test]
        fn test_png_format() {
            let format = OutputFormat::from_str("png", None).unwrap();
            assert!(matches!(format, OutputFormat::Png));
        }

        #[test]
        fn test_png_ignores_quality() {
            let format = OutputFormat::from_str("png", Some(50)).unwrap();
            assert!(matches!(format, OutputFormat::Png));
        }

        #[test]
        fn test_case_insensitive_jpeg() {
            assert!(OutputFormat::from_str("JPEG", None).is_ok());
            assert!(OutputFormat::from_str("Jpeg", None).is_ok());
            assert!(OutputFormat::from_str("jPeG", None).is_ok());
        }

        #[test]
        fn test_case_insensitive_jpg() {
            assert!(OutputFormat::from_str("JPG", None).is_ok());
            assert!(OutputFormat::from_str("Jpg", None).is_ok());
        }

        #[test]
        fn test_case_insensitive_webp() {
            assert!(OutputFormat::from_str("WEBP", None).is_ok());
            assert!(OutputFormat::from_str("WebP", None).is_ok());
            assert!(OutputFormat::from_str("wEbP", None).is_ok());
        }

        #[test]
        fn test_case_insensitive_avif() {
            assert!(OutputFormat::from_str("AVIF", None).is_ok());
            assert!(OutputFormat::from_str("Avif", None).is_ok());
        }

        #[test]
        fn test_case_insensitive_png() {
            assert!(OutputFormat::from_str("PNG", None).is_ok());
            assert!(OutputFormat::from_str("Png", None).is_ok());
        }

        #[test]
        fn test_unsupported_format() {
            let result = OutputFormat::from_str("gif", None);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("unsupported format"));
        }

        #[test]
        fn test_unsupported_format_bmp() {
            let result = OutputFormat::from_str("bmp", None);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("unsupported format"));
        }

        #[test]
        fn test_empty_format() {
            let result = OutputFormat::from_str("", None);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("unsupported format"));
        }

        #[test]
        fn test_quality_range() {
            // 品質値の範囲テスト（1-100が有効）
            let format = OutputFormat::from_str("jpeg", Some(1)).unwrap();
            assert!(matches!(format, OutputFormat::Jpeg { quality: 1, .. }));

            let format = OutputFormat::from_str("jpeg", Some(100)).unwrap();
            assert!(matches!(format, OutputFormat::Jpeg { quality: 100, .. }));
        }
    }

    mod preset_config_tests {
        use super::*;

        #[test]
        fn test_thumbnail_preset() {
            let preset = PresetConfig::get("thumbnail").unwrap();
            assert_eq!(preset.width, Some(150));
            assert_eq!(preset.height, Some(150));
            assert!(matches!(preset.format, OutputFormat::WebP { quality: 75 }));
        }

        #[test]
        fn test_avatar_preset() {
            let preset = PresetConfig::get("avatar").unwrap();
            assert_eq!(preset.width, Some(200));
            assert_eq!(preset.height, Some(200));
            assert!(matches!(preset.format, OutputFormat::WebP { quality: 80 }));
        }

        #[test]
        fn test_hero_preset() {
            let preset = PresetConfig::get("hero").unwrap();
            assert_eq!(preset.width, Some(1920));
            assert_eq!(preset.height, None); // アスペクト比維持
            assert!(matches!(
                preset.format,
                OutputFormat::Jpeg { quality: 85, .. }
            ));
        }

        #[test]
        fn test_social_preset() {
            let preset = PresetConfig::get("social").unwrap();
            assert_eq!(preset.width, Some(1200));
            assert_eq!(preset.height, Some(630)); // OGP標準サイズ
            assert!(matches!(
                preset.format,
                OutputFormat::Jpeg { quality: 80, .. }
            ));
        }

        #[test]
        fn test_case_insensitive_thumbnail() {
            assert!(PresetConfig::get("THUMBNAIL").is_some());
            assert!(PresetConfig::get("Thumbnail").is_some());
            assert!(PresetConfig::get("ThUmBnAiL").is_some());

            let preset = PresetConfig::get("THUMBNAIL").unwrap();
            assert_eq!(preset.width, Some(150));
            assert_eq!(preset.height, Some(150));
        }

        #[test]
        fn test_case_insensitive_avatar() {
            assert!(PresetConfig::get("AVATAR").is_some());
            assert!(PresetConfig::get("Avatar").is_some());

            let preset = PresetConfig::get("AVATAR").unwrap();
            assert_eq!(preset.width, Some(200));
            assert_eq!(preset.height, Some(200));
        }

        #[test]
        fn test_case_insensitive_hero() {
            assert!(PresetConfig::get("HERO").is_some());
            assert!(PresetConfig::get("Hero").is_some());

            let preset = PresetConfig::get("HERO").unwrap();
            assert_eq!(preset.width, Some(1920));
            assert_eq!(preset.height, None);
        }

        #[test]
        fn test_case_insensitive_social() {
            assert!(PresetConfig::get("SOCIAL").is_some());
            assert!(PresetConfig::get("Social").is_some());

            let preset = PresetConfig::get("SOCIAL").unwrap();
            assert_eq!(preset.width, Some(1200));
            assert_eq!(preset.height, Some(630));
        }

        #[test]
        fn test_unknown_preset() {
            assert!(PresetConfig::get("unknown").is_none());
            assert!(PresetConfig::get("").is_none());
            assert!(PresetConfig::get("invalid").is_none());
            assert!(PresetConfig::get("thumbnails").is_none()); // 複数形は無効
        }

        #[test]
        fn test_preset_new() {
            let preset = PresetConfig::new(
                Some(800),
                Some(600),
                OutputFormat::Jpeg {
                    quality: 90,
                    fast_mode: false,
                },
            );
            assert_eq!(preset.width, Some(800));
            assert_eq!(preset.height, Some(600));
            assert!(matches!(
                preset.format,
                OutputFormat::Jpeg { quality: 90, .. }
            ));
        }

        #[test]
        fn test_preset_new_with_none() {
            let preset = PresetConfig::new(Some(1920), None, OutputFormat::WebP { quality: 80 });
            assert_eq!(preset.width, Some(1920));
            assert_eq!(preset.height, None);
            assert!(matches!(preset.format, OutputFormat::WebP { quality: 80 }));
        }
    }
}
