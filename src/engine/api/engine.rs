#![cfg_attr(not(feature = "napi"), allow(dead_code))]

// src/engine/api/engine.rs
//
// `ImageEngine` struct definition, NAPI constructor methods, and the internal
// lazy accessors for ICC profile and EXIF data. Pipeline operations, output
// methods, and batch helpers live in sibling files inside this directory so
// each unit stays well under the 800-line per-file ceiling enforced by the
// project's coding-style rules.

use super::super::firewall::FirewallConfig;
#[allow(unused_imports)]
use crate::engine::io::{extract_exif_raw, extract_icc_profile_lossy, load_file_safe, Source};
#[cfg(feature = "napi")]
use crate::error::LazyImageError;
use crate::ops::{Operation, PresetConfig};
use image::DynamicImage;
use std::sync::Arc;
use std::sync::OnceLock;

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;

// MetadataPolicy lives in the sibling `metadata` module (src/engine/metadata.rs).
// Re-export so downstream code (tasks.rs, engine.rs tests) can still reach it
// via `super::api::MetadataPolicy` or `crate::engine::api::MetadataPolicy`.
pub use super::super::metadata::MetadataPolicy;

/// Convert a `LazyImageError` into a `napi::Error`, attaching the code and
/// category metadata expected by the JS error surface.
///
/// Shared across every sibling file in `engine/api/` so each impl block keeps
/// using the historical `napi_err(&env, err)` call site.
#[cfg(feature = "napi")]
pub(super) fn napi_err(env: &Env, err: LazyImageError) -> napi::Error {
    crate::error::napi_error_with_code(env, err).expect("failed to create napi error")
}

/// The main image processing engine.
///
/// Usage:
/// ```js
/// const result = await ImageEngine.from(buffer)
///   .resize(800)
///   .rotate(90)
///   .grayscale()
///   .toBuffer('jpeg', 75);
/// ```
#[cfg_attr(feature = "napi", napi)]
pub struct ImageEngine {
    /// Image source - supports in-memory data and memory-mapped files
    pub(crate) source: Option<Source>,
    /// Decoded image (populated after first decode or on sync operations)
    /// Uses Arc to share decoded image between engines. Combined with Cow<DynamicImage>
    /// in apply_ops, this enables true Copy-on-Write: no deep copy until mutation.
    pub(crate) decoded: Option<Arc<DynamicImage>>,
    /// Queued operations
    pub(crate) ops: Vec<Operation>,
    /// Last applied preset (used by toPresetBuffer/toPresetFile convenience APIs)
    pub(crate) last_preset: Option<PresetConfig>,
    /// ICC color profile extracted from source image (lazy: extracted on first access)
    pub(crate) icc_profile: OnceLock<Option<Arc<Vec<u8>>>>,
    /// Raw EXIF data extracted from source image (lazy: extracted on first access)
    /// Stored as raw bytes to avoid serialization issues with little_exif::Metadata
    pub(crate) exif_data: OnceLock<Option<Arc<Vec<u8>>>>,
    /// Whether to auto-apply EXIF Orientation (default: true)
    pub(crate) auto_orient: bool,
    /// Single source of truth for metadata preservation decisions.
    pub(crate) metadata_policy: MetadataPolicy,
    /// Whether we already emitted a warning about XMP being unsupported.
    pub(crate) xmp_warning_emitted: bool,
    pub(crate) firewall: FirewallConfig,
}

#[cfg(feature = "napi")]
#[napi]
impl ImageEngine {
    // =========================================================================
    // CONSTRUCTORS
    // =========================================================================

    /// Create engine from a buffer.
    /// Full pixel decode, queued operations, and metadata extraction are all
    /// deferred until output methods run (true lazy evaluation).
    #[napi(factory)]
    pub fn from(buffer: Buffer) -> Self {
        let data = buffer.to_vec();

        ImageEngine {
            source: Some(Source::from_vec(data)),
            decoded: None,
            ops: Vec::new(),
            last_preset: None,
            icc_profile: OnceLock::new(),
            exif_data: OnceLock::new(),
            auto_orient: true,
            metadata_policy: MetadataPolicy::default_policy(),
            xmp_warning_emitted: false,
            firewall: FirewallConfig::disabled(),
        }
    }

    /// Create engine from a file path.
    /// Full pixel decode, queued operations, and metadata extraction are all
    /// deferred until output methods run (true lazy evaluation).
    /// Small/medium files are read into memory to prevent SIGBUS; very large files (>256 MB)
    /// use mmap with advisory locks.
    /// This is the recommended way for server-side processing of large images.
    #[napi(factory, js_name = "fromPath")]
    pub fn from_path(env: Env, path: String) -> Result<Self> {
        if path.trim().is_empty() {
            return Err(napi_err(
                &env,
                LazyImageError::invalid_argument("path", "<empty>", "path must not be empty"),
            ));
        }

        let path_buf = std::path::PathBuf::from(&path);

        let source = match load_file_safe(&path_buf) {
            Ok(s) => s,
            Err(e) => return Err(crate::error::napi_error_with_code(&env, e)?),
        };

        Ok(ImageEngine {
            source: Some(source),
            decoded: None,
            ops: Vec::new(),
            last_preset: None,
            icc_profile: OnceLock::new(),
            exif_data: OnceLock::new(),
            auto_orient: true,
            metadata_policy: MetadataPolicy::default_policy(),
            xmp_warning_emitted: false,
            firewall: FirewallConfig::disabled(),
        })
    }

    /// Create a clone of this engine (for multi-output scenarios)
    #[napi(js_name = "clone")]
    pub fn clone_engine(&self) -> Result<ImageEngine> {
        // If OnceLock is already initialized, clone the value into a new initialized OnceLock.
        // If not yet initialized, create a fresh uninitialized OnceLock (avoids forcing extraction).
        let icc_profile = {
            let lock = OnceLock::new();
            if let Some(val) = self.icc_profile.get() {
                let _ = lock.set(val.clone());
            }
            lock
        };
        let exif_data = {
            let lock = OnceLock::new();
            if let Some(val) = self.exif_data.get() {
                let _ = lock.set(val.clone());
            }
            lock
        };

        Ok(ImageEngine {
            source: self.source.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
            last_preset: self.last_preset.clone(),
            icc_profile,
            exif_data,
            auto_orient: self.auto_orient,
            metadata_policy: self.metadata_policy.clone(),
            xmp_warning_emitted: self.xmp_warning_emitted,
            firewall: self.firewall.clone(),
        })
    }
}

// =============================================================================
// INTERNAL IMPLEMENTATION
// =============================================================================

impl ImageEngine {
    /// Lazily extract and return the ICC profile from the source data.
    /// The extraction runs at most once; subsequent calls return the cached result.
    pub(super) fn icc_profile(&self) -> Option<&Arc<Vec<u8>>> {
        self.icc_profile
            .get_or_init(|| {
                self.source
                    .as_ref()
                    .and_then(|s| s.as_bytes())
                    .and_then(|bytes| extract_icc_profile_lossy(bytes).map(Arc::new))
            })
            .as_ref()
    }

    /// Lazily extract and return the raw EXIF data from the source data.
    /// The extraction runs at most once; subsequent calls return the cached result.
    pub(super) fn exif_data(&self) -> Option<&Arc<Vec<u8>>> {
        self.exif_data
            .get_or_init(|| {
                self.source
                    .as_ref()
                    .and_then(|s| s.as_bytes())
                    .and_then(|bytes| extract_exif_raw(bytes).map(Arc::new))
            })
            .as_ref()
    }
}
