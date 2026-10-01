use std::io::Cursor;

use xca::{
    compress, compress_stream, decompress, decompress_stream, decompress_with_options, frame_info,
    CompressionOptions, DecodeOptions, Error,
};

#[test]
fn frame_info_rejects_nonzero_terminator_checksum() {
    let mut archive = compress(b"terminator validation");
    *archive.last_mut().unwrap() = 1;
    assert!(matches!(frame_info(&archive), Err(Error::InvalidMatch)));
    assert!(decompress(&archive).is_err());
}

#[test]
fn streaming_decoder_rejects_trailing_data() {
    let mut archive = Vec::new();
    compress_stream(
        &mut Cursor::new(b"strict stream"),
        &mut archive,
        CompressionOptions::default(),
    )
    .unwrap();
    archive.push(0xaa);
    let mut output = Vec::new();
    assert!(matches!(
        decompress_stream(&mut Cursor::new(archive), &mut output, 1024),
        Err(Error::TrailingData)
    ));
}

#[test]
fn streaming_decoder_is_bounded_for_reference_archives() {
    let unit = vec![0x5a; 128 * 1024];
    let data = unit.repeat(8);
    let archive = compress(&data);
    let mut output = Vec::new();
    assert!(matches!(
        decompress_stream(&mut Cursor::new(archive), &mut output, data.len()),
        Err(Error::StreamingReferencesUnsupported)
    ));
}

#[test]
fn per_call_decode_options_do_not_change_global_state() {
    let data = b"per-call options ".repeat(8192);
    let archive = compress(&data);
    let decoded = decompress_with_options(
        &archive,
        DecodeOptions {
            max_output_size: data.len(),
            thread_limit: 1,
        },
    )
    .unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn legacy_format_vectors_remain_decodable() {
    let data = b"permanent XCA compatibility vector".repeat(1024);
    let current = compress(&data);
    for (magic, profile) in [(b"XCA4", 1), (b"XCA5", 3), (b"XCA6", 5), (b"XCA7", 7)] {
        let mut archive = current.clone();
        archive[..4].copy_from_slice(magic);
        archive[4] = profile;
        let info = frame_info(&archive).unwrap();
        assert_eq!(info.version, magic[3] - b'0');
        assert_eq!(decompress(&archive).unwrap(), data);
    }
}
