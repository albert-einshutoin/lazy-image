// src/engine/firewall.rs
//
// Image Firewall configuration and enforcement helpers.

use crate::engine::io::extract_icc_profile;
use crate::error::LazyImageError;
use std::time::Instant;

const STRICT_MAX_PIXELS: u64 = 40_000_000; // ~8K x 5K
const LENIENT_MAX_PIXELS: u64 = 75_000_000; // generous but below global MAX_PIXELS
const STRICT_MAX_BYTES: u64 = 32 * 1024 * 1024; // 32MB input cap
const LENIENT_MAX_BYTES: u64 = 48 * 1024 * 1024; // 48MB input cap
const STRICT_TIMEOUT_MS: u64 = 5_000; // 5s wall clock (allows JPEG/WebP, strict on slow AVIF)
const LENIENT_TIMEOUT_MS: u64 = 30_000; // 30s wall clock (allows AVIF on large images)
const LENIENT_METADATA_LIMIT: u64 = 512 * 1024; // 512KB ICC cap

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirewallPolicy {
    Disabled,
    Strict,
    Lenient,
    Custom,
}

#[derive(Clone, Debug)]
pub struct FirewallConfig {
    pub enabled: bool,
    pub policy: FirewallPolicy,
    pub max_pixels: Option<u64>,
    pub max_bytes: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub reject_metadata: bool,
    metadata_max_bytes: Option<u64>,
}

impl Default for FirewallConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            policy: FirewallPolicy::Disabled,
            max_pixels: None,
            max_bytes: None,
            timeout_ms: None,
            reject_metadata: false,
            metadata_max_bytes: None,
        }
    }
}

impl FirewallConfig {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn strict() -> Self {
        Self {
            enabled: true,
            policy: FirewallPolicy::Strict,
            max_pixels: Some(STRICT_MAX_PIXELS),
            max_bytes: Some(STRICT_MAX_BYTES),
            timeout_ms: Some(STRICT_TIMEOUT_MS),
            reject_metadata: true,
            metadata_max_bytes: None,
        }
    }

    pub fn lenient() -> Self {
        Self {
            enabled: true,
            policy: FirewallPolicy::Lenient,
            max_pixels: Some(LENIENT_MAX_PIXELS),
            max_bytes: Some(LENIENT_MAX_BYTES),
            timeout_ms: Some(LENIENT_TIMEOUT_MS),
            reject_metadata: false,
            metadata_max_bytes: Some(LENIENT_METADATA_LIMIT),
        }
    }

    pub fn custom() -> Self {
        Self {
            enabled: true,
            policy: FirewallPolicy::Custom,
            max_pixels: None,
            max_bytes: None,
            timeout_ms: None,
            reject_metadata: false,
            metadata_max_bytes: None,
        }
    }

    pub fn apply_policy(policy: FirewallPolicy) -> Self {
        match policy {
            FirewallPolicy::Disabled => Self::disabled(),
            FirewallPolicy::Strict => Self::strict(),
            FirewallPolicy::Lenient => Self::lenient(),
            FirewallPolicy::Custom => Self::custom(),
        }
    }

    pub fn enforce_source_len(&self, len: usize) -> Result<(), LazyImageError> {
        if !self.enabled {
            return Ok(());
        }
        if let Some(limit) = self.max_bytes {
            let len_u64 = len as u64;
            if len_u64 > limit {
                return Err(LazyImageError::firewall_violation(format!(
                    "Image Firewall: input size {} bytes exceeds limit of {} bytes. \
                     Use .limits({{ maxBytes: {} }}) to allow larger files or switch to lenient policy.",
                    len_u64, limit, len_u64 + 1024 * 1024
                )));
            }
        }
        Ok(())
    }

    pub fn enforce_pixels(&self, width: u32, height: u32) -> Result<(), LazyImageError> {
        if !self.enabled {
            return Ok(());
        }
        if let Some(limit) = self.max_pixels {
            let pixels = width as u64 * height as u64;
            if pixels > limit {
                return Err(LazyImageError::firewall_violation(format!(
                    "Image Firewall: {}x{} ({} pixels) exceeds limit of {} pixels. \
                     Resize the image first with .resize() or use .limits({{ maxPixels: {} }}).",
                    width, height, pixels, limit, pixels + 1_000_000
                )));
            }
        }
        Ok(())
    }

    pub fn enforce_timeout(
        &self,
        started_at: Instant,
        stage: &'static str,
    ) -> Result<(), LazyImageError> {
        if !self.enabled {
            return Ok(());
        }
        if let Some(limit_ms) = self.timeout_ms {
            let elapsed_ms = started_at.elapsed().as_millis() as u64;
            if elapsed_ms > limit_ms {
                return Err(LazyImageError::firewall_violation(format!(
                    "Image Firewall: processing exceeded {}ms timeout at {} stage (elapsed: {}ms). \
                     Use .limits({{ timeoutMs: {} }}) for longer operations or switch to lenient policy.",
                    limit_ms, stage, elapsed_ms, elapsed_ms + 5000
                )));
            }
        }
        Ok(())
    }

    pub fn scan_metadata(&self, data: &[u8]) -> Result<(), LazyImageError> {
        if !self.enabled {
            return Ok(());
        }
        let Some(icc) = extract_icc_profile(data) else {
            return Ok(());
        };

        if self.reject_metadata {
            return Err(LazyImageError::firewall_violation(
                "Image Firewall: embedded ICC profile blocked under strict policy. \
                 Use .sanitize({ policy: 'lenient' }) to allow ICC profiles.",
            ));
        }

        if let Some(limit) = self.metadata_max_bytes {
            let icc_len = icc.len() as u64;
            if icc_len > limit {
                return Err(LazyImageError::firewall_violation(format!(
                    "Image Firewall: ICC profile ({} bytes) exceeds limit of {} bytes. \
                     This may indicate a malformed or malicious file.",
                    icc_len, limit
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use img_parts::{png::Png, Bytes, ImageICC};

    fn build_icc_payload(len: usize) -> Vec<u8> {
        let size = len.max(128);
        let mut data = vec![0u8; size];
        data[..4].copy_from_slice(&(size as u32).to_be_bytes());
        data[4..8].copy_from_slice(b"TEST");
        data[8] = 2;
        data[12..16].copy_from_slice(b"mntr");
        data[16..20].copy_from_slice(b"RGB ");
        data[20..24].copy_from_slice(b"XYZ ");
        data
    }

    fn png_with_icc(len: usize) -> Vec<u8> {
        let img = ImageBuffer::from_fn(2, 2, |x, y| Rgb([x as u8, y as u8, 0]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();

        let mut png = Png::from_bytes(Bytes::from(buf)).unwrap();
        png.set_icc_profile(Some(Bytes::from(build_icc_payload(len))));
        let mut out = Vec::new();
        png.encoder().write_to(&mut out).unwrap();
        out
    }

    #[test]
    fn strict_policy_enforces_pixels_and_metadata() {
        let cfg = FirewallConfig::strict();
        assert!(cfg.enforce_pixels(2000, 2000).is_ok());
        assert!(cfg.enforce_pixels(7000, 7000).is_err());

        let png = png_with_icc(256);
        assert!(cfg.scan_metadata(&png).is_err());
    }

    #[test]
    fn lenient_policy_allows_small_icc() {
        let cfg = FirewallConfig::lenient();
        let safe_png = png_with_icc(256);
        assert!(cfg.scan_metadata(&safe_png).is_ok());

        let oversized_png = png_with_icc((LENIENT_METADATA_LIMIT + 1) as usize);
        assert!(cfg.scan_metadata(&oversized_png).is_err());
    }

    #[test]
    fn timeout_enforced() {
        let cfg = FirewallConfig {
            enabled: true,
            policy: FirewallPolicy::Custom,
            max_pixels: None,
            max_bytes: None,
            timeout_ms: Some(1),
            reject_metadata: false,
            metadata_max_bytes: None,
        };
        let fake_start = Instant::now() - std::time::Duration::from_millis(5);
        assert!(cfg.enforce_timeout(fake_start, "decode").is_err());
    }
}
