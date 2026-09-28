//! XCA4: an independently implemented adaptive lossless compression codec.
use std::fmt;
mod codec;
mod v4;
pub(crate) use codec::crc32;
pub use v4::{
    analyze_archive, compress_stream, decompress_stream, ArchiveAnalysis, CompressionOptions,
    StreamStats,
};
const DEFAULT_OUTPUT_LIMIT: usize = 1 << 30;
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
    pub level: u8,
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
                write!(f, "compression level must be between 1 and 9, got {v}")
            }
            Self::InvalidBlockSize(v) => write!(f, "invalid XCA block size {v}"),
            Self::Io(v) => write!(f, "I/O error: {v}"),
            Self::InvalidMatch => write!(f, "invalid XCA dictionary match"),
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
    compress_with_level(input, 6).expect("valid built-in level")
}
pub fn compress_with_level(input: &[u8], level: u8) -> Result<Vec<u8>, Error> {
    v4::compress_slice(input, level)
}
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    decompress_with_limit(input, DEFAULT_OUTPUT_LIMIT)
}
pub fn decompress_with_limit(input: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    v4::decompress_slice(input, limit)
}
pub fn frame_info(input: &[u8]) -> Result<FrameInfo, Error> {
    v4::frame_info(input)
}
#[cfg(feature = "c-api")]
mod c_api;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_all_levels() {
        let d = b"XCA independent codec XCA independent codec";
        for l in 1..=9 {
            assert_eq!(decompress(&compress_with_level(d, l).unwrap()).unwrap(), d)
        }
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
        assert!(decompress(&d).is_err())
    }
    #[test]
    fn enforces_limit() {
        let d = compress(&vec![0; 1024]);
        assert!(matches!(
            decompress_with_limit(&d, 100),
            Err(Error::OutputLimitExceeded { .. })
        ))
    }
}
