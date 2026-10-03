#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

const MAX_SOURCE: usize = 1024 * 1024;
const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 || data.len() > MAX_SOURCE {
        return;
    }
    let source = &data[4..];
    let mut archive = xca::compress(source);
    let selector = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
    let position = selector % archive.len();
    let mask = data[0].wrapping_mul(17) | 1;
    archive[position] ^= mask;

    let _ = xca::frame_info_with_limit(&archive, OUTPUT_LIMIT);
    let _ = xca::check_with_limit(&archive, OUTPUT_LIMIT);
    let _ = xca::analyze_archive(&archive);
    let _ = xca::decompress_with_limit(&archive, OUTPUT_LIMIT);
    let mut output = Vec::new();
    let _ = xca::decompress_stream(
        &mut Cursor::new(&archive),
        &mut output,
        OUTPUT_LIMIT,
    );
    let mut exact = Vec::new();
    let _ = xca::decompress_stream_exact(
        &mut Cursor::new(&archive),
        &mut exact,
        OUTPUT_LIMIT,
    );
});
