#![cfg_attr(not(feature = "napi"), allow(dead_code))]

// src/engine/api/mod.rs
//
// Public surface for the `engine::api` module. Functionality is split across
// sibling files so each unit stays well under the 800-line per-file ceiling
// from the project's coding-style rules:
//   - `engine.rs`        — `ImageEngine` struct, NAPI constructors, internal
//                          lazy accessors, shared `napi_err` helper, and the
//                          `MetadataPolicy` re-export
//   - `types.rs`         — `#[napi(object)]` parameter/result structs
//   - `pipeline_ops.rs`  — queued pipeline ops (resize/crop/rotate/...) plus
//                          presets
//   - `output_ops.rs`    — async output methods (`toBuffer`, `toFile`,
//                          unified `encode`/`encodeToFile`)
//   - `batch_ops.rs`     — sync inspection + parallel batch processing
//
// Each `#[napi] impl ImageEngine { ... }` block lives in its own file and is
// processed independently by the napi macro — there is no requirement that
// all methods on an exported struct sit in a single block.

mod batch_ops;
mod engine;
mod output_ops;
mod pipeline_ops;
mod types;

// Re-exports preserve the historical `crate::engine::api::*` paths used by
// `src/engine.rs`, `src/engine/tasks/*`, integration tests, and the napi
// codegen output.
pub use engine::{ImageEngine, MetadataPolicy};

// These types are NAPI-exported parameter/result structs. They are reached
// from JS bindings via napi codegen, not by Rust call sites, so the compiler
// sees no in-crate use — keep the re-exports under `#[allow(unused_imports)]`
// so the public API surface stays addressable as `crate::engine::api::*`.
#[cfg(feature = "napi")]
#[allow(unused_imports)]
pub use types::{
    BatchOptions, Dimensions, FirewallLimitOptions, KeepMetadataOptions, PresetResult,
    ResizeOptions, SanitizeOptions,
};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
