//! XCA8: an independently implemented adaptive lossless compression codec.
use std::{fmt, path::Path};
mod codec;
mod file_io;
mod huffman;
mod v4;
pub(crate) use codec::crc32;
pub use v4::{
    analyze_archive, compress_stream, decompress_stream, ArchiveAnalysis, CompressionOptions,
    StreamStats,
};
const DEFAULT_OUTPUT_LIMIT: usize = 1 << 30;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStats {
    pub input_bytes: u64,
    pub output_bytes: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Stored,
    Rle,
    Lz,
    DeltaLz,
    XorLz,
    Adaptive,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameInfo {
    pub version: u8,
    pub method: Method,
    pub legacy_level: Option<u8>,
    pub original_size: usize,
    pub frame_size: usize,
    pub checksum: bool,
    pub blocks: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Truncated,
    BadMagic,
    UnsupportedMethod(u8),
    InvalidLevel(u8),
    InvalidBlockSize(usize),
    Io(String),
    InvalidMatch,
    InvalidEntropyData,
    InvalidRun,
    LengthMismatch { expected: usize, actual: usize },
    ChecksumMismatch { expected: u32, actual: u32 },
    OutputLimitExceeded { requested: usize, limit: usize },
    TrailingData,
    InputTooLarge,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated XCA stream"),
            Self::BadMagic => write!(f, "invalid XCA magic"),
            Self::UnsupportedMethod(v) => write!(f, "unsupported XCA method {v}"),
            Self::InvalidLevel(v) => {
                write!(f, "invalid legacy compression level {v}")
            }
            Self::InvalidBlockSize(v) => write!(f, "invalid XCA block size {v}"),
            Self::Io(v) => write!(f, "I/O error: {v}"),
            Self::InvalidMatch => write!(f, "invalid XCA dictionary match"),
            Self::InvalidEntropyData => write!(f, "invalid XCA entropy stream"),
            Self::InvalidRun => write!(f, "invalid XCA run"),
            Self::LengthMismatch { expected, actual } => write!(
                f,
                "decoded length mismatch: expected {expected}, got {actual}"
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                f,
                "checksum mismatch: expected {expected:08x}, got {actual:08x}"
            ),
            Self::OutputLimitExceeded { requested, limit } => {
                write!(f, "decoded size {requested} exceeds limit {limit}")
            }
            Self::TrailingData => write!(f, "unexpected trailing data"),
            Self::InputTooLarge => write!(f, "input is too large"),
        }
    }
}
impl std::error::Error for Error {}
pub fn compress(input: &[u8]) -> Vec<u8> {
    v4::compress_slice(input).expect("built-in unified profile is valid")
}
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    decompress_with_limit(input, DEFAULT_OUTPUT_LIMIT)
}
pub fn decompress_with_limit(input: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    v4::decompress_slice(input, limit)
}
pub fn decompress_into(input: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    decompress_into_with_limit(input, output, DEFAULT_OUTPUT_LIMIT)
}
pub fn decompress_into_with_limit(
    input: &[u8],
    output: &mut [u8],
    limit: usize,
) -> Result<usize, Error> {
    v4::decompress_into_slice(input, output, limit)
}
pub fn frame_info(input: &[u8]) -> Result<FrameInfo, Error> {
    v4::frame_info(input)
}
pub fn check(input: &[u8]) -> Result<usize, Error> {
    v4::verify_slice(input, DEFAULT_OUTPUT_LIMIT)
}
pub fn compress_file(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<FileStats, Error> {
    let input = file_io::ReadMap::open(input_path.as_ref())?;
    let output = compress(input.as_slice());
    file_io::write_all(output_path.as_ref(), &output)?;
    Ok(FileStats {
        input_bytes: input.as_slice().len() as u64,
        output_bytes: output.len() as u64,
    })
}
pub fn decompress_file(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<FileStats, Error> {
    decompress_file_with_limit(input_path, output_path, DEFAULT_OUTPUT_LIMIT)
}
pub fn decompress_file_with_limit(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    limit: usize,
) -> Result<FileStats, Error> {
    let output_path = output_path.as_ref();
    let input = file_io::ReadMap::open(input_path.as_ref())?;
    let output_len = frame_info(input.as_slice())?.original_size;
    if output_len > limit {
        return Err(Error::OutputLimitExceeded {
            requested: output_len,
            limit,
        });
    }
    let result = {
        let mut output = file_io::WriteMap::create(output_path, output_len)?;
        decompress_into_with_limit(input.as_slice(), output.as_mut_slice(), limit)
    };
    match result {
        Ok(_) => Ok(FileStats {
            input_bytes: input.as_slice().len() as u64,
            output_bytes: output_len as u64,
        }),
        Err(error) => {
            file_io::remove_failed_output(output_path);
            Err(error)
        }
    }
}
#[cfg(feature = "c-api")]
mod c_api;
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[test]
    fn round_trip_unified_profile() {
        let d = b"XCA independent codec XCA independent codec";
        assert_eq!(decompress(&compress(d)).unwrap(), d)
    }
    #[test]
    fn compresses_runs() {
        let d = vec![42; 10_000];
        assert!(compress(&d).len() < d.len() / 10)
    }
    #[test]
    fn detects_damage() {
        let mut d = compress(&vec![7; 4096]);
        let i = d.len() - 1;
        d[i] ^= 1;
        assert!(decompress(&d).is_err());
        assert!(check(&d).is_err());
    }
    #[test]
    fn enforces_limit() {
        let d = compress(&vec![0; 1024]);
        assert!(matches!(
            decompress_with_limit(&d, 100),
            Err(Error::OutputLimitExceeded { .. })
        ))
    }
    #[test]
    fn decompresses_into_caller_buffer() {
        let data = b"caller-owned output ".repeat(8192);
        let archive = compress(&data);
        let mut output = vec![0; data.len()];
        assert_eq!(decompress_into(&archive, &mut output).unwrap(), data.len());
        assert_eq!(output, data);
        let mut short = vec![0; data.len() - 1];
        assert!(matches!(
            decompress_into(&archive, &mut short),
            Err(Error::LengthMismatch { .. })
        ));
    }
    #[test]
    fn file_helpers_round_trip() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir();
        let source = root.join(format!("xca-{nonce}-source.bin"));
        let archive = root.join(format!("xca-{nonce}-archive.xca"));
        let restored = root.join(format!("xca-{nonce}-restored.bin"));
        let data = b"portable XCA file helper ".repeat(4096);

        fs::write(&source, &data).unwrap();
        let compressed = compress_file(&source, &archive).unwrap();
        let decompressed = decompress_file(&archive, &restored).unwrap();

        assert_eq!(compressed.input_bytes, data.len() as u64);
        assert_eq!(decompressed.output_bytes, data.len() as u64);
        assert_eq!(fs::read(&restored).unwrap(), data);

        let mut damaged = fs::read(&archive).unwrap();
        let middle = damaged.len() / 2;
        damaged[middle] ^= 1;
        fs::write(&archive, damaged).unwrap();
        fs::remove_file(&restored).unwrap();
        assert!(decompress_file(&archive, &restored).is_err());
        assert!(!restored.exists());

        fs::write(&source, []).unwrap();
        compress_file(&source, &archive).unwrap();
        decompress_file(&archive, &restored).unwrap();
        assert!(fs::read(&restored).unwrap().is_empty());

        for path in [source, archive, restored] {
            let _ = fs::remove_file(path);
        }
    }
}
