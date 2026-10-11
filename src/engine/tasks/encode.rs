// src/engine/tasks/encode.rs
//
// Buffer-output encode tasks:
// - `EncodeTask`             — basic `toBuffer()` path
// - `EncodeWithMetricsTask`  — `toBuffer()` with ProcessingMetrics
// - `EncodeTargetBytesTask`  — bounded quality search for a byte budget

use super::atomic::write_encoded_output;
use super::context::TaskContext;
use super::processing::{encode_prepared, prepare_for_encode, EncodeContext, PreparedEncode};
use crate::error::LazyImageError;
#[cfg(test)]
use image::DynamicImage;
#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
use napi::{bindgen_prelude::BufferSlice, Env, Task};
#[cfg(test)]
use std::borrow::Cow;

define_encode_task! {
    pub struct EncodeTask {}
    napi {
        type Output = Vec<u8>;
        type JsValue = napi::bindgen_prelude::Buffer;
        compute(self) {
            match self.ctx.process_and_encode(None) {
                Ok(result) => {
                    #[cfg(feature = "napi")]
                    { self.ctx.last_error = None; }
                    Ok(result)
                }
                Err(lazy_err) => {
                    Err(self.ctx.store_and_convert_error(lazy_err))
                }
            }
        }
        resolve(self, env, output) {
            BufferSlice::from_data(&env, output)?.into_buffer(&env)
        }
    }
}

#[cfg(test)]
impl EncodeTask {
    /// Delegate to TaskContext for backward compatibility with test callers.
    #[cfg(not(feature = "napi"))]
    pub(crate) fn process_and_encode(
        &self,
        metrics: Option<&mut crate::ProcessingMetrics>,
    ) -> std::result::Result<Vec<u8>, LazyImageError> {
        self.ctx.process_and_encode(metrics)
    }

    pub(crate) fn decode(&self) -> std::result::Result<Cow<'_, DynamicImage>, LazyImageError> {
        self.ctx.decode()
    }
}

define_encode_task! {
    pub struct EncodeWithMetricsTask {}
    napi {
        type Output = (Vec<u8>, crate::ProcessingMetrics);
        type JsValue = crate::OutputWithMetrics;
        compute(self) {
            let mut metrics = crate::ProcessingMetrics::default();
            match self.ctx.process_and_encode(Some(&mut metrics)) {
                Ok(data) => {
                    #[cfg(feature = "napi")]
                    { self.ctx.last_error = None; }
                    Ok((data, metrics))
                }
                Err(lazy_err) => {
                    Err(self.ctx.store_and_convert_error(lazy_err))
                }
            }
        }
        resolve(self, env, output) {
            let (data, metrics) = output;
            let js_buffer = BufferSlice::from_data(&env, data)?.into_buffer(&env)?;
            Ok(crate::OutputWithMetrics {
                data: js_buffer,
                metrics,
            })
        }
    }
}

/// Result payload for the byte-budget quality search (before NAPI conversion).
#[derive(Debug)]
pub struct TargetBytesOutput {
    pub data: Vec<u8>,
    pub quality: u8,
    pub bytes_out: u32,
    pub budget_met: bool,
    pub target_bytes: u32,
    pub metrics: crate::ProcessingMetrics,
}

/// Result payload for native target-bytes file output. The selected encoded
/// bytes never cross into V8; only metadata is resolved to JavaScript.
pub struct FileTargetBytesOutput {
    pub bytes_written: u32,
    pub quality: u8,
    pub budget_met: bool,
    pub target_bytes: u32,
    pub metrics: crate::ProcessingMetrics,
}

define_encode_task! {
    /// Async task that performs byte-budget quality search entirely in Rust,
    /// without intermediate JS↔NAPI boundary crossings.
    pub struct EncodeTargetBytesTask {
        pub target_bytes: u32,
        pub min_quality: u8,
        pub max_quality: u8,
        pub quality_floor_policy_strict: bool,
    }
    napi {
        type Output = TargetBytesOutput;
        type JsValue = crate::TargetBytesResult;
        compute(self) {
            match self.search() {
                Ok(result) => {
                    #[cfg(feature = "napi")]
                    { self.ctx.last_error = None; }
                    Ok(result)
                }
                Err(lazy_err) => {
                    Err(self.ctx.store_and_convert_error(lazy_err))
                }
            }
        }
        resolve(self, env, output) {
            let js_buffer = BufferSlice::from_data(&env, output.data)?.into_buffer(&env)?;
            Ok(crate::TargetBytesResult {
                data: js_buffer,
                quality: output.quality as u32,
                bytes_out: output.bytes_out,
                budget_met: output.budget_met,
                target_bytes: output.target_bytes,
                metrics: output.metrics,
            })
        }
    }
}

impl EncodeTargetBytesTask {
    /// Try qualities from highest to lowest, stopping at the first budget fit.
    ///
    /// Decode + ops are performed **once** via [`prepare_for_encode`]; only the
    /// encode step (and EXIF embed) re-runs per iteration, at most 100 times for
    /// the validated public quality range. Size need not be monotonic in quality.
    pub(super) fn search(&self) -> std::result::Result<TargetBytesOutput, LazyImageError> {
        // Decode + apply ops once. Memory permit is held by `prepared` so the
        // semaphore reservation stays valid across every encode iteration below.
        let base_ctx = self.ctx.encode_context();
        let prepared = prepare_for_encode(
            base_ctx.source,
            base_ctx.decoded,
            base_ctx.ops,
            base_ctx.icc_state,
            base_ctx.auto_orient,
            base_ctx.firewall,
            base_ctx.format,
        )?;

        self.search_prepared(&prepared, base_ctx)
    }

    pub(super) fn search_prepared(
        &self,
        prepared: &PreparedEncode,
        base_ctx: EncodeContext<'_>,
    ) -> std::result::Result<TargetBytesOutput, LazyImageError> {
        let mut best: Option<(Vec<u8>, u8, u32, crate::ProcessingMetrics)> = None;
        let mut search_cpu_time = 0.0;
        let mut search_peak_rss = 0;

        for quality in (self.min_quality..=self.max_quality).rev() {
            let format_at_q = self.ctx.format.with_quality(quality);
            // Reuse the base context, overriding only the per-iteration format.
            let iter_ctx = EncodeContext {
                format: &format_at_q,
                ..base_ctx
            };

            let mut metrics = crate::ProcessingMetrics::default();
            let encoded = encode_prepared(prepared, &iter_ctx, Some(&mut metrics))?;
            search_cpu_time = metrics.cpu_time;
            search_peak_rss = search_peak_rss.max(metrics.peak_rss);

            let size = match u32::try_from(encoded.len()) {
                Ok(size) => size,
                Err(_) => {
                    // target_bytes is a u32, so an unrepresentable candidate is
                    // necessarily over budget. Try the remaining qualities.
                    continue;
                }
            };

            if size <= self.target_bytes {
                best = Some((encoded, quality, size, metrics));
                break;
            } else {
                // Best-effort keeps the smallest output; descending order retains
                // the highest quality on a size tie without a monotonicity assumption.
                if best
                    .as_ref()
                    .is_none_or(|(_, _, prev_size, _)| size < *prev_size)
                {
                    best = Some((encoded, quality, size, metrics));
                }
            }
        }

        let (data, quality, bytes_out, mut metrics) = match best {
            Some(candidate) => candidate,
            None => {
                return Err(LazyImageError::encode_failed(
                    self.ctx.format.as_str(),
                    "Failed to encode any candidate while searching for target bytes",
                ));
            }
        };

        let budget_met = bytes_out <= self.target_bytes;
        if !budget_met && self.quality_floor_policy_strict {
            return Err(LazyImageError::encode_failed(
                self.ctx.format.as_str(),
                format!(
                    "Unable to meet targetBytes={} within quality range {}-{}",
                    self.target_bytes, self.min_quality, self.max_quality,
                ),
            ));
        }

        // The selected candidate may precede later rejected probes. Keep its
        // encode/output fields, but report cumulative work for the whole search.
        metrics.total_ms = prepared.start_total.elapsed().as_secs_f64() * 1000.0;
        metrics.processing_time = metrics.total_ms / 1000.0;
        metrics.cpu_time = search_cpu_time;
        metrics.peak_rss = search_peak_rss;

        Ok(TargetBytesOutput {
            data,
            quality,
            bytes_out,
            budget_met,
            target_bytes: self.target_bytes,
            metrics,
        })
    }
}

/// Async target-bytes search whose winning Rust-owned candidate is written
/// atomically before resolving metadata to JavaScript.
pub struct WriteTargetBytesTask {
    pub search: EncodeTargetBytesTask,
    pub output_path: String,
}

#[cfg(feature = "napi")]
#[napi]
impl Task for WriteTargetBytesTask {
    type Output = FileTargetBytesOutput;
    type JsValue = crate::FileTargetBytesNativeResult;

    fn compute(&mut self) -> Result<Self::Output> {
        let output = match self.search.search() {
            Ok(output) => output,
            Err(error) => return Err(self.search.ctx.store_and_convert_error(error)),
        };
        let bytes_written = match write_encoded_output(&self.output_path, &output.data) {
            Ok(bytes_written) => bytes_written,
            Err(error) => return Err(self.search.ctx.store_and_convert_error(error)),
        };

        self.search.ctx.last_error = None;
        Ok(FileTargetBytesOutput {
            bytes_written,
            quality: output.quality,
            budget_met: output.budget_met,
            target_bytes: output.target_bytes,
            metrics: output.metrics,
        })
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(crate::FileTargetBytesNativeResult {
            bytes_written: output.bytes_written,
            quality: output.quality as u32,
            budget_met: output.budget_met,
            target_bytes: output.target_bytes,
            metrics: output.metrics,
        })
    }

    fn reject(&mut self, env: Env, error: napi::Error) -> Result<Self::JsValue> {
        let lazy_error = self.search.ctx.take_stored_error(error);
        Err(crate::error::napi_error_with_code(&env, lazy_error)?)
    }
}
