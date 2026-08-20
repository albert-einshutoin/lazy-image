#![cfg_attr(not(feature = "napi"), allow(dead_code))]

// src/engine/metadata.rs
//
// MetadataPolicy: single source of truth for which metadata to preserve
// in output images. Extracted from api.rs to keep the NAPI surface thin.

use crate::engine::firewall::FirewallPolicy;

/// Policy for which metadata to preserve in output images.
///
/// Modeled as a sum type so that incoherent combinations are *unrepresentable*:
/// the previous `{ icc: bool, exif: bool, gps: bool }` struct allowed `gps =
/// true` with `exif = false` (keep GPS but drop the EXIF block that carries it),
/// a silently wrong state. Here the `strip_gps` flag only exists on the variants
/// that actually keep EXIF, so the type system rules the bad state out.
///
/// The `effective_icc` / `effective_exif` / `strip_gps` accessors preserve the
/// historical interface used across the encode pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataPolicy {
    /// Strip all metadata (default; smallest output, most private).
    StripAll,
    /// Keep only the ICC colour profile; EXIF (and therefore GPS) is stripped.
    KeepIcc,
    /// Keep EXIF (and optionally strip GPS), but drop the ICC profile.
    KeepExif { strip_gps: bool },
    /// Keep both ICC and EXIF (optionally stripping GPS from the EXIF).
    KeepAll { strip_gps: bool },
}

impl MetadataPolicy {
    /// Default: strip all metadata for security and smaller file sizes.
    /// GPS is stripped by default for privacy (exceeds Sharp's capabilities).
    pub fn default_policy() -> Self {
        Self::StripAll
    }

    /// Strip all metadata (firewall strict mode).
    pub fn strip_all() -> Self {
        Self::StripAll
    }

    /// Build a policy from the independent keep-ICC / keep-EXIF / strip-GPS
    /// flags accepted by the public `keepMetadata` API. The mapping funnels the
    /// four `(icc, exif)` combinations into the four variants, so a `strip_gps`
    /// choice is only retained when EXIF is actually preserved.
    pub fn from_flags(icc: bool, exif: bool, strip_gps: bool) -> Self {
        match (icc, exif) {
            (false, false) => Self::StripAll,
            (true, false) => Self::KeepIcc,
            (false, true) => Self::KeepExif { strip_gps },
            (true, true) => Self::KeepAll { strip_gps },
        }
    }

    /// Apply the output metadata invariant owned by a built-in firewall policy.
    pub fn apply_firewall(&mut self, firewall: FirewallPolicy) {
        match firewall {
            FirewallPolicy::Strict => *self = Self::StripAll,
            FirewallPolicy::PublicUpload => *self = Self::KeepIcc,
            _ => {}
        }
    }

    /// Resolve final ICC policy.
    pub fn effective_icc(&self) -> bool {
        matches!(self, Self::KeepIcc | Self::KeepAll { .. })
    }

    /// Resolve final EXIF policy.
    pub fn effective_exif(&self) -> bool {
        matches!(self, Self::KeepExif { .. } | Self::KeepAll { .. })
    }

    /// Whether GPS tags should be stripped from EXIF.
    ///
    /// Returns `true` (strip) whenever EXIF is not kept — there is no EXIF block
    /// to carry GPS — and otherwise honours the variant's `strip_gps` flag.
    pub fn strip_gps(&self) -> bool {
        match self {
            Self::StripAll | Self::KeepIcc => true,
            Self::KeepExif { strip_gps } | Self::KeepAll { strip_gps } => *strip_gps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_all_keeps_nothing() {
        let p = MetadataPolicy::strip_all();
        assert!(!p.effective_icc());
        assert!(!p.effective_exif());
        assert!(p.strip_gps());
    }

    #[test]
    fn from_flags_maps_every_combination() {
        assert_eq!(
            MetadataPolicy::from_flags(false, false, false),
            MetadataPolicy::StripAll
        );
        assert_eq!(
            MetadataPolicy::from_flags(true, false, true),
            MetadataPolicy::KeepIcc
        );
        assert_eq!(
            MetadataPolicy::from_flags(false, true, false),
            MetadataPolicy::KeepExif { strip_gps: false }
        );
        assert_eq!(
            MetadataPolicy::from_flags(true, true, true),
            MetadataPolicy::KeepAll { strip_gps: true }
        );
    }

    #[test]
    fn keep_exif_honors_strip_gps_flag() {
        let keep = MetadataPolicy::from_flags(false, true, false);
        assert!(keep.effective_exif());
        assert!(!keep.strip_gps(), "strip_gps:false must be honored");

        let strip = MetadataPolicy::from_flags(false, true, true);
        assert!(strip.strip_gps());
    }

    #[test]
    fn gps_without_exif_is_unrepresentable() {
        // icc-only / strip-all never keep EXIF, so GPS is always stripped —
        // the old incoherent "keep GPS but drop EXIF" state cannot be built.
        assert!(MetadataPolicy::KeepIcc.strip_gps());
        assert!(MetadataPolicy::StripAll.strip_gps());
    }

    #[test]
    fn firewall_override_strips_all() {
        let mut p = MetadataPolicy::KeepAll { strip_gps: false };
        p.apply_firewall(FirewallPolicy::Strict);
        assert_eq!(p, MetadataPolicy::StripAll);
    }

    #[test]
    fn public_upload_forces_icc_only_regardless_of_prior_policy() {
        for mut policy in [
            MetadataPolicy::StripAll,
            MetadataPolicy::KeepAll { strip_gps: false },
        ] {
            policy.apply_firewall(FirewallPolicy::PublicUpload);
            assert_eq!(policy, MetadataPolicy::KeepIcc);
        }
    }
}
