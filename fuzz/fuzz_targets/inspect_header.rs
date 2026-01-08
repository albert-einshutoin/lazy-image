#![no_main]

use lazy_image::inspect_header_from_bytes;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = inspect_header_from_bytes(data);
});
