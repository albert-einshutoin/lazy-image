// src/engine/tasks/mod.rs
//
// Async task implementations for NAPI.
//
// These tasks run in background threads (via napi's `AsyncTask`) so they do
// not block the Node.js main thread. The module is split by concern:
//
// - `macros`     — `define_encode_task!` macro
// - `atomic`     — atomic file-write helper
// - `context`    — `TaskContext`, `BatchResult`, decode helpers
// - `processing` — two-phase encode pipeline (`prepare_for_encode` + `encode_prepared`)
// - `encode`     — buffer-output tasks (`EncodeTask`, `EncodeWithMetricsTask`, `EncodeTargetBytesTask`)
// - `write`      — file-output tasks (`WriteFileTask`, `UnifiedEncodeTask`, ...)
// - `batch`      — multi-input batch tasks (`BatchTask`, `BatchWithMetricsTask`)
//
// Internal seams (helpers, `EncodeContext`, `PreparedEncode`) are `pub(super)`
// so the public API surface from this directory remains exactly the set of
// task structs (and `BatchResult` for NAPI) re-exported below.

// `unused_imports` is suppressed for non-napi builds because most of the
// re-exports below feed `api.rs`, which is itself `cfg(feature = "napi")`.
// `dead_code` covers struct/function definitions that are only reachable
// through the NAPI `Task` impls.
#![cfg_attr(not(feature = "napi"), allow(dead_code, unused_imports))]

#[macro_use]
mod macros;

mod atomic;
mod batch;
mod context;
mod encode;
mod processing;
mod write;

#[cfg(all(test, not(feature = "napi")))]
mod tests;

pub use batch::{BatchTask, BatchWithMetricsTask};
#[cfg(feature = "napi")]
pub use context::BatchResult;
pub use context::TaskContext;
pub use encode::{EncodeTargetBytesTask, EncodeTask, EncodeWithMetricsTask, WriteTargetBytesTask};
pub use write::{
    UnifiedEncodeTask, UnifiedEncodeToFileTask, WriteFileTask, WriteFileWithMetricsTask,
};
