// src/engine/pipeline/mod.rs
//
// Pipeline operations entry point. The previous monolithic
// `src/engine/pipeline.rs` exceeded the 800-line ceiling enforced by
// CLAUDE.md, so the implementation is split into sibling files:
//
// - color_state:  pixel-format/bit-depth/transfer/ICC tracking types and
//                 their pure-state-update helpers.
// - capabilities: operation precondition tracking and pipeline-level
//                 validation (`validate_operation_sequence`).
// - optimize:     `optimize_ops` — resize/crop fusion + consecutive-resize
//                 merging.
// - apply:        `apply_ops_tracked`/`apply_ops` — the Copy-on-Write
//                 execution loop that actually mutates image data.
//
// All cross-module helpers are `pub(super)` so the public surface
// (`crate::engine::pipeline::*`) stays narrow: we re-export only the
// items already consumed by `engine.rs`, `tasks/processing.rs`, and the
// stress harness.

#![cfg_attr(not(feature = "napi"), allow(dead_code))]

use crate::error::LazyImageError;

mod apply;
mod capabilities;
mod color_state;
mod optimize;

// Type alias for Result - always use LazyImageError to preserve error taxonomy.
// This ensures pipeline errors are properly classified (CodecError, UserError,
// etc.) rather than collapsing into generic InternalBug errors.
pub(crate) type PipelineResult<T> = std::result::Result<T, LazyImageError>;

pub use apply::{apply_ops, apply_ops_tracked};
// `BitDepth`, `ColorSpace`, `TransferFn`, and `ColorTrackedImage` are part of
// the pipeline's public surface (they were `pub` in the original monolith)
// but currently have no in-crate callers — keep them addressable as
// `crate::engine::pipeline::*` for downstream Rust consumers and tests.
#[allow(unused_imports)]
pub use color_state::{BitDepth, ColorSpace, ColorState, ColorTrackedImage, IccState, TransferFn};
pub use optimize::optimize_ops;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
