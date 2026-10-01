#![no_main]

use libfuzzer_sys::fuzz_target;

const MAX_INPUT: usize = 2 * 1024 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT {
        return;
    }
    let archive = xca::compress(data);
    let decoded = xca::decompress_with_limit(&archive, data.len()).unwrap();
    assert_eq!(decoded, data);
    let mut caller_owned = vec![0; data.len()];
    let written = xca::decompress_into_with_limit(&archive, &mut caller_owned, data.len()).unwrap();
    assert_eq!(written, data.len());
    assert_eq!(caller_owned, data);
});