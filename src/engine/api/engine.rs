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

#[cfg(feature = "napi")]
pub struct LoadFileTask {
    path: std::path::PathBuf,
    last_error: Option<LazyImageError>,
}

#[cfg(feature = "napi")]
#[napi]
impl Task for LoadFileTask {
    type Output = Source;
    type JsValue = ImageEngine;

    fn compute(&mut self) -> Result<Self::Output> {
        load_file_safe(&self.path).map_err(|error| {
            self.last_error = Some(error.clone());
            napi::Error::from(error)
        })
    }

    fn resolve(&mut self, _env: Env, source: Self::Output) -> Result<Self::JsValue> {
        Ok(ImageEngine::with_source(source))
    }

    fn reject(&mut self, env: Env, error: napi::Error) -> Result<Self::JsValue> {
        let lazy_error = self
            .last_error
            .take()
            .unwrap_or_else(|| LazyImageError::generic(error.to_string()));
        Err(crate::error::napi_error_with_code(&env, lazy_error)?)
    }
}

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
    // `napi_error_with_code` only fails if NAPI itself cannot allocate the JS
    // error object (heap pressure). Falling back to a status-only error keeps
    // the rejection on the Promise path instead of `.expect()` aborting the
    // entire Node.js process.
    crate::error::napi_error_with_code(env, err)
        .unwrap_or_else(|_| napi::Error::from_status(napi::Status::GenericFailure))
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
        Self::with_source(Source::from_vec(buffer.to_vec()))
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

        Ok(Self::with_source(source))
    }

    /// Create an engine from a file path without reading the source on Node's
    /// main thread. File open/read/mmap setup runs on an N-API worker.
    #[napi(js_name = "fromPathAsync", ts_return_type = "Promise<ImageEngine>")]
    pub fn from_path_async(env: Env, path: String) -> Result<AsyncTask<LoadFileTask>> {
        if path.trim().is_empty() {
            return Err(napi_err(
                &env,
                LazyImageError::invalid_argument("path", "<empty>", "path must not be empty"),
            ));
        }

        // Resolve relative paths before enqueueing. The worker may run after a
        // caller changes process.cwd(), but the selected source must retain the
        // same call-time semantics as synchronous fromPath().
        let path = std::path::absolute(&path).map_err(|error| {
            napi_err(&env, LazyImageError::file_read_failed(path.clone(), error))
        })?;

        Ok(AsyncTask::new(LoadFileTask {
            path,
            last_error: None,
        }))
    }

    /// Create a clone of this engine (for multi-output scenarios)
    #[napi(js_name = "clone")]
    pub fn clone_engine(&self) -> Result<ImageEngine> {
        let icc_profile = clone_once_lock(&self.icc_profile);
        let exif_data = clone_once_lock(&self.exif_data);

        Ok(ImageEngine {
            source: self.source.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
            last_preset: self.last_preset.clone(),
            icc_profile,
            exif_data,
            auto_orient: self.auto_orient,
            metadata_policy: self.metadata_policy,
            xmp_warning_emitted: self.xmp_warning_emitted,
            firewall: self.firewall,
        })
    }
}

// =============================================================================
// INTERNAL IMPLEMENTATION
// =============================================================================

impl ImageEngine {
    /// Build an engine around `source` with all other state at its defaults.
    pub(crate) fn with_source(source: Source) -> Self {
        ImageEngine {
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
        }
    }

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

/// Clone an initialized `OnceLock` value into a fresh lock without forcing lazy extraction.
fn clone_once_lock<T: Clone>(source: &OnceLock<T>) -> OnceLock<T> {
    let lock = OnceLock::new();
    if let Some(value) = source.get() {
        lock.set(value.clone())
            .unwrap_or_else(|_| unreachable!("freshly-created OnceLock must accept its first set"));
    }
    lock
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_source_initializes_default_engine_state() {
        let engine = ImageEngine::with_source(Source::from_vec(Vec::new()));

        assert!(engine.source.is_some());
        assert!(engine.decoded.is_none());
        assert!(engine.ops.is_empty());
        assert!(engine.last_preset.is_none());
        assert!(engine.icc_profile.get().is_none());
        assert!(engine.exif_data.get().is_none());
        assert!(engine.auto_orient);
        assert_eq!(engine.metadata_policy, MetadataPolicy::default_policy());
        assert!(!engine.xmp_warning_emitted);
        assert!(!engine.firewall.enabled);
        assert_eq!(
            engine.firewall.policy,
            crate::engine::firewall::FirewallPolicy::Disabled
        );
        assert_eq!(engine.firewall.max_pixels, None);
        assert_eq!(engine.firewall.max_bytes, None);
        assert_eq!(engine.firewall.timeout_ms, None);
        assert!(!engine.firewall.reject_metadata);
    }

    #[test]
    fn clone_once_lock_does_not_force_uninitialized_lock() {
        let source = OnceLock::<Option<Arc<Vec<u8>>>>::new();

        let cloned = clone_once_lock(&source);

        assert!(source.get().is_none());
        assert!(cloned.get().is_none());
    }

    #[test]
    fn clone_once_lock_copies_initialized_value() {
        let source = OnceLock::new();
        source.set(Some(Arc::new(vec![1, 2, 3]))).unwrap();

        let cloned = clone_once_lock(&source);

        assert_eq!(
            source.get().unwrap().as_ref().unwrap().as_slice(),
            &[1, 2, 3]
        );
        assert_eq!(
            cloned.get().unwrap().as_ref().unwrap().as_slice(),
            &[1, 2, 3]
        );
    }
}
