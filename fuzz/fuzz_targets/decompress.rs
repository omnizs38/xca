#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

const FUZZ_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

fuzz_target!(|data: &[u8]| {
    let _ = xca::frame_info_with_limit(data, FUZZ_OUTPUT_LIMIT);
    let _ = xca::check_with_limit(data, FUZZ_OUTPUT_LIMIT);
    let _ = xca::analyze_archive(data);
    let _ = xca::decompress_with_limit(data, FUZZ_OUTPUT_LIMIT);
    let mut streamed = Vec::new();
    let _ = xca::decompress_stream(
        &mut Cursor::new(data),
        &mut streamed,
        FUZZ_OUTPUT_LIMIT,
    );
});
