// src/engine/tasks/encode.rs
//
// Buffer-output encode tasks:
// - `EncodeTask`             — basic `toBuffer()` path
// - `EncodeWithMetricsTask`  — `toBuffer()` with ProcessingMetrics
// - `EncodeTargetBytesTask`  — binary search over quality for a byte budget

use super::context::TaskContext;
use super::processing::{encode_prepared, prepare_for_encode, EncodeContext};
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

/// Result payload for the byte-budget binary search (before NAPI conversion).
#[derive(Debug)]
pub struct TargetBytesOutput {
    pub data: Vec<u8>,
    pub quality: u8,
    pub bytes_out: u32,
    pub budget_met: bool,
    pub target_bytes: u32,
    pub metrics: crate::ProcessingMetrics,
}

define_encode_task! {
    /// Async task that performs byte-budget binary search entirely in Rust.
    ///
    /// Previous implementation did this in JS, requiring ~7 NAPI round-trips per
    /// call (one per binary-search iteration). Moving the loop to Rust eliminates
    /// all intermediate JS↔NAPI boundary crossings.
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
    /// Run binary search over quality to find the best encode within byte budget.
    ///
    /// Decode + ops are performed **once** via [`prepare_for_encode`]; only the
    /// encode step (and EXIF embed) re-runs per iteration. For a 4000x3000 JPEG
    /// with a resize op this eliminates a 5-7x re-decode penalty (issue #558).
    pub(super) fn search(&self) -> std::result::Result<TargetBytesOutput, LazyImageError> {
        // Decode + apply ops once. Memory permit is held by `prepared` so the
        // semaphore reservation stays valid across every encode iteration below.
        let base_ctx = self.ctx.encode_context();
        let prepared = prepare_for_encode(
            base_ctx.source,
            base_ctx.decoded,
            base_ctx.ops,
            base_ctx.icc_present,
            base_ctx.auto_orient,
            base_ctx.firewall,
            base_ctx.format,
        )?;

        let mut low = self.min_quality;
        let mut high = self.max_quality;
        let mut best_under: Option<(Vec<u8>, u8, u32, crate::ProcessingMetrics)> = None;
        let mut best_over: Option<(Vec<u8>, u8, u32, crate::ProcessingMetrics)> = None;

        while low <= high {
            let quality = low + (high - low) / 2;
            let format_at_q = self.ctx.format.with_quality(quality);
            // Reuse the base context, overriding only the per-iteration format.
            let iter_ctx = EncodeContext {
                format: &format_at_q,
                ..base_ctx
            };

            let mut metrics = crate::ProcessingMetrics::default();
            let encoded = encode_prepared(&prepared, &iter_ctx, Some(&mut metrics))?;

            let size = encoded.len() as u32;

            if size <= self.target_bytes {
                best_under = Some((encoded, quality, size, metrics));
                if quality == u8::MAX {
                    break;
                }
                low = quality + 1;
            } else {
                let dominated = best_over.as_ref().is_some_and(|(_, _, prev_size, _)| {
                    size > *prev_size
                        || (size == *prev_size && quality <= best_over.as_ref().unwrap().1)
                });
                if best_over.is_none() || !dominated {
                    best_over = Some((encoded, quality, size, metrics));
                }
                high = quality.saturating_sub(1);
                if quality == 0 {
                    break;
                }
            }
        }

        let (data, quality, bytes_out, metrics) = match (best_under, best_over) {
            (Some(under), _) => under,
            (None, Some(over)) => over,
            (None, None) => {
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
