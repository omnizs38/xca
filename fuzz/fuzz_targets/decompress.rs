#![no_main]

use libfuzzer_sys::fuzz_target;

const FUZZ_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

fuzz_target!(|data: &[u8]| {
    let _ = xca::frame_info(data);
    let _ = xca::decompress_with_limit(data, FUZZ_OUTPUT_LIMIT);
});