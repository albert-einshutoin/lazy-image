#![no_main]

use lazy_image::engine::EncodeTask;
use lazy_image::ops::{Operation, OutputFormat};
use libfuzzer_sys::fuzz_target;
use std::sync::Arc;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let task = EncodeTask {
        source: Some(Arc::new(data.to_vec())),
        decoded: None,
        ops: Vec::<Operation>::new(),
        format: OutputFormat::Png,
        icc_profile: None,
        keep_metadata: false,
    };

    let _ = task.decode();
});
