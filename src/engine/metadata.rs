#![cfg_attr(not(feature = "napi"), allow(dead_code))]

// src/engine/metadata.rs
//
// MetadataPolicy: single source of truth for which metadata to preserve
// in output images. Extracted from api.rs to keep the NAPI surface thin.

/// Policy for which metadata to preserve in output images.
///
/// Centralizes the keep-ICC / keep-EXIF / strip-GPS decision so that every
/// output path reads a single source of truth instead of resolving ad-hoc
/// boolean combinations.
#[derive(Clone, Debug)]
pub struct MetadataPolicy {
    /// Whether to preserve ICC profile in output (default: false)
    pub icc: bool,
    /// Whether to preserve EXIF metadata in output (default: false)
    pub exif: bool,
    /// Whether to keep GPS tags in EXIF (default: false — stripped for privacy)
    pub gps: bool,
}

impl MetadataPolicy {
    /// Default: strip all metadata for security and smaller file sizes.
    /// GPS is stripped by default for privacy (exceeds Sharp's capabilities).
    pub fn default_policy() -> Self {
        Self {
            icc: false,
            exif: false,
            gps: false,
        }
    }

    /// Strip all metadata (firewall strict mode).
    pub fn strip_all() -> Self {
        Self {
            icc: false,
            exif: false,
            gps: false,
        }
    }

    /// Apply firewall override: if reject_metadata is true, strip everything.
    pub fn apply_firewall(&mut self, reject_metadata: bool) {
        if reject_metadata {
            *self = Self::strip_all();
        }
    }

    /// Resolve final ICC policy.
    pub fn effective_icc(&self) -> bool {
        self.icc
    }

    /// Resolve final EXIF policy.
    pub fn effective_exif(&self) -> bool {
        self.exif
    }

    /// Whether GPS tags should be stripped from EXIF.
    /// Returns true when GPS should be stripped (i.e., gps == false).
    pub fn strip_gps(&self) -> bool {
        !self.gps
    }
}
