// src/engine/tasks/write.rs
//
// File-output encode tasks. Each task writes a single encoded image atomically
// via `super::atomic::write_encoded_output`.
//
// - `WriteFileTask`             — `toFile()` without metrics
// - `WriteFileWithMetricsTask`  — `toFile()` with ProcessingMetrics
// - `UnifiedEncodeTask`         — `encode()` (buffer + optional metrics)
// - `UnifiedEncodeToFileTask`   — `encodeToFile()` (file + optional metrics)

use super::atomic::write_encoded_output;
use super::context::TaskContext;
#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
use napi::{bindgen_prelude::BufferSlice, Env, Task};

define_encode_task! {
    pub struct WriteFileTask {
        pub output_path: String,
    }
    napi {
        type Output = u32;
        type JsValue = u32;
        compute(self) {
            let data = match self.ctx.process_and_encode(None) {
                Ok(data) => data,
                Err(lazy_err) => {
                    return Err(self.ctx.store_and_convert_error(lazy_err));
                }
            };

            match write_encoded_output(&self.output_path, &data) {
                Ok(bytes_written) => Ok(bytes_written),
                Err(lazy_err) => {
                    Err(self.ctx.store_and_convert_error(lazy_err))
                }
            }
        }
        resolve(self, _env, output) {
            Ok(output)
        }
    }
}

define_encode_task! {
    pub struct WriteFileWithMetricsTask {
        pub output_path: String,
    }
    napi {
        type Output = (u32, crate::ProcessingMetrics);
        type JsValue = crate::FileOutputWithMetrics;
        compute(self) {
            let mut metrics = crate::ProcessingMetrics::default();
            let data = match self.ctx.process_and_encode(Some(&mut metrics)) {
                Ok(data) => data,
                Err(lazy_err) => {
                    return Err(self.ctx.store_and_convert_error(lazy_err));
                }
            };

            match write_encoded_output(&self.output_path, &data) {
                Ok(bytes_written) => Ok((bytes_written, metrics)),
                Err(lazy_err) => {
                    Err(self.ctx.store_and_convert_error(lazy_err))
                }
            }
        }
        resolve(self, _env, output) {
            let (bytes_written, metrics) = output;
            Ok(crate::FileOutputWithMetrics {
                bytes_written,
                metrics,
            })
        }
    }
}

// ============================================================================================
// Unified encode tasks — power encode() and encodeToFile()
// ============================================================================================

define_encode_task! {
    /// Task for `encode()` — returns `EncodeResult { data, metrics? }`.
    pub struct UnifiedEncodeTask {
        pub want_metrics: bool,
    }
    napi {
        type Output = (Vec<u8>, Option<crate::ProcessingMetrics>);
        type JsValue = crate::EncodeResult;
        compute(self) {
            let mut metrics = if self.want_metrics {
                Some(crate::ProcessingMetrics::default())
            } else {
                None
            };

            match self.ctx.process_and_encode(metrics.as_mut()) {
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
            Ok(crate::EncodeResult {
                data: js_buffer,
                metrics,
            })
        }
    }
}

define_encode_task! {
    /// Task for `encodeToFile()` — returns `FileEncodeResult { bytesWritten, metrics? }`.
    pub struct UnifiedEncodeToFileTask {
        pub want_metrics: bool,
        pub output_path: String,
    }
    napi {
        type Output = (u32, Option<crate::ProcessingMetrics>);
        type JsValue = crate::FileEncodeResult;
        compute(self) {
            let mut metrics = if self.want_metrics {
                Some(crate::ProcessingMetrics::default())
            } else {
                None
            };

            let data = match self.ctx.process_and_encode(metrics.as_mut()) {
                Ok(d) => d,
                Err(lazy_err) => {
                    return Err(self.ctx.store_and_convert_error(lazy_err));
                }
            };

            match write_encoded_output(&self.output_path, &data) {
                Ok(bytes_written) => {
                    #[cfg(feature = "napi")]
                    { self.ctx.last_error = None; }
                    Ok((bytes_written, metrics))
                }
                Err(lazy_err) => {
                    Err(self.ctx.store_and_convert_error(lazy_err))
                }
            }
        }
        resolve(self, _env, output) {
            let (bytes_written, metrics) = output;
            Ok(crate::FileEncodeResult {
                bytes_written,
                metrics,
            })
        }
    }
}
