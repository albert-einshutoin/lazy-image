// src/engine/io/mod.rs
//
// I/O subsystem for the engine. Split across three submodules:
//
// - `source`: `Source` enum + `MmapGuard`/`MemoryGuard` + `load_file_safe`
// - `icc`:    ICC profile extraction across JPEG/PNG/WebP/AVIF containers
// - `exif`:   EXIF metadata extraction and inspection helpers
//
// `mod.rs` owns nothing but module declarations and re-exports that keep the
// historical `crate::engine::io::*` paths stable for the rest of the engine.

pub mod exif;
pub mod icc;
pub mod source;

#[cfg(test)]
mod test_helpers;

// -----------------------------------------------------------------------------
// Public API re-exports (used by engine.rs and external callers via NAPI).
// -----------------------------------------------------------------------------

pub use exif::{exif_has_gps, extract_exif_raw, has_exif};
pub use icc::{extract_icc_profile, extract_icc_profile_lossy};
// `IccExtractionResult` only appears in the signature of `extract_icc_profile`,
// `MmapGuard` and `MemoryGuard` only inside `Source` variants. They're still
// part of the public surface (downstream Rust consumers may name them), so
// re-export at the historical path even though the in-crate caller list is
// empty.
#[allow(unused_imports)]
pub use icc::IccExtractionResult;
pub use source::{load_file_safe, Source};
#[allow(unused_imports)]
pub use source::{MemoryGuard, MmapGuard};

// -----------------------------------------------------------------------------
// Crate-internal re-exports — narrow surface for items that other engine
// submodules reach by the historical `crate::engine::io::*` path. ICC-specific
// helpers (`IccExtractor`, `JpegIccExtractor`, etc.) stay inside `icc` and are
// reachable via `crate::engine::io::icc::*` when needed.
// -----------------------------------------------------------------------------

#[cfg(test)]
pub(crate) use exif::find_exif_segment_jpeg_for_tests;
