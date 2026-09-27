//! XCA is an embeddable, dependency-free lossless compression library.
//!
//! The default encoder writes XCA2 frames and the decoder accepts both XCA2
//! and the earlier XCA1 prototype format.

use std::fmt;

const MAGIC_V1: &[u8; 4] = b"XCA1";
const MAGIC_V2: &[u8; 4] = b"XCA2";
const V1_HEADER_LEN: usize = 13;
const V2_HEADER_LEN: usize = 28;
const METHOD_STORE: u8 = 0;
const METHOD_RLE: u8 = 1;
const METHOD_LZ: u8 = 2;
const FLAG_CHECKSUM: u8 = 1;
const HASH_SIZE: usize = 1 << 16;
const MAX_DISTANCE: usize = u16::MAX as usize;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = u8::MAX as usize;
const NONE: usize = usize::MAX;
const DEFAULT_OUTPUT_LIMIT: usize = 1 << 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Stored,
    Rle,
    Lz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameInfo {
    pub version: u8,
    pub method: Method,
    pub level: u8,
    pub original_size: usize,
    pub frame_size: usize,
    pub checksum: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Truncated,
    BadMagic,
    UnsupportedMethod(u8),
    InvalidLevel(u8),
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
            Self::UnsupportedMethod(method) => write!(f, "unsupported XCA method {method}"),
            Self::InvalidLevel(level) => write!(f, "compression level must be between 1 and 9, got {level}"),
            Self::InvalidMatch => write!(f, "invalid LZ match"),
            Self::InvalidRun => write!(f, "invalid RLE payload"),
            Self::LengthMismatch { expected, actual } => write!(f, "decoded length mismatch: expected {expected}, got {actual}"),
            Self::ChecksumMismatch { expected, actual } => write!(f, "checksum mismatch: expected {expected:08x}, got {actual:08x}"),
            Self::OutputLimitExceeded { requested, limit } => write!(f, "decoded size {requested} exceeds configured limit {limit}"),
            Self::TrailingData => write!(f, "unexpected trailing data"),
            Self::InputTooLarge => write!(f, "input is too large for this platform or format"),
        }
    }
}

impl std::error::Error for Error {}

/// Compresses data at the balanced default level (6).
pub fn compress(input: &[u8]) -> Vec<u8> {
    compress_with_level(input, 6).expect("the built-in compression level is valid")
}

/// Compresses data into one self-contained XCA2 frame.
pub fn compress_with_level(input: &[u8], level: u8) -> Result<Vec<u8>, Error> {
    if !(1..=9).contains(&level) {
        return Err(Error::InvalidLevel(level));
    }
    let original_len = u64::try_from(input.len()).map_err(|_| Error::InputTooLarge)?;
    let lz = encode_lz(input, search_depth(level));
    let (method, payload) = if lz.len() < input.len() {
        (METHOD_LZ, lz.as_slice())
    } else {
        (METHOD_STORE, input)
    };
    let payload_len = u64::try_from(payload.len()).map_err(|_| Error::InputTooLarge)?;

    let mut output = Vec::with_capacity(V2_HEADER_LEN + payload.len());
    output.extend_from_slice(MAGIC_V2);
    output.push(method);
    output.push(level);
    output.push(FLAG_CHECKSUM);
    output.push(0);
    output.extend_from_slice(&original_len.to_le_bytes());
    output.extend_from_slice(&payload_len.to_le_bytes());
    output.extend_from_slice(&crc32(input).to_le_bytes());
    output.extend_from_slice(payload);
    Ok(output)
}

/// Decompresses a frame with a conservative 1 GiB output limit.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    decompress_with_limit(input, DEFAULT_OUTPUT_LIMIT)
}

/// Decompresses a frame while refusing allocations above `max_output_size`.
pub fn decompress_with_limit(input: &[u8], max_output_size: usize) -> Result<Vec<u8>, Error> {
    if input.len() < 4 {
        return Err(Error::Truncated);
    }
    match &input[..4] {
        magic if magic == MAGIC_V2 => decode_v2(input, max_output_size),
        magic if magic == MAGIC_V1 => decode_v1(input, max_output_size),
        _ => Err(Error::BadMagic),
    }
}

/// Reads frame metadata without decompressing the payload.
pub fn frame_info(input: &[u8]) -> Result<FrameInfo, Error> {
    if input.len() < 4 {
        return Err(Error::Truncated);
    }
    if &input[..4] == MAGIC_V2 {
        let header = parse_v2_header(input)?;
        Ok(FrameInfo {
            version: 2,
            method: method_from_byte(header.method)?,
            level: header.level,
            original_size: header.original_size,
            frame_size: input.len(),
            checksum: header.flags & FLAG_CHECKSUM != 0,
        })
    } else if &input[..4] == MAGIC_V1 {
        if input.len() < V1_HEADER_LEN {
            return Err(Error::Truncated);
        }
        let original_size = usize_from_u64(read_u64(&input[5..13]))?;
        Ok(FrameInfo {
            version: 1,
            method: method_from_byte(input[4])?,
            level: 0,
            original_size,
            frame_size: input.len(),
            checksum: false,
        })
    } else {
        Err(Error::BadMagic)
    }
}

struct V2Header {
    method: u8,
    level: u8,
    flags: u8,
    original_size: usize,
    payload_size: usize,
    checksum: u32,
}

fn parse_v2_header(input: &[u8]) -> Result<V2Header, Error> {
    if input.len() < V2_HEADER_LEN {
        return Err(Error::Truncated);
    }
    let original_size = usize_from_u64(read_u64(&input[8..16]))?;
    let payload_size = usize_from_u64(read_u64(&input[16..24]))?;
    let expected_frame_size = V2_HEADER_LEN.checked_add(payload_size).ok_or(Error::InputTooLarge)?;
    if input.len() < expected_frame_size {
        return Err(Error::Truncated);
    }
    if input.len() > expected_frame_size {
        return Err(Error::TrailingData);
    }
    Ok(V2Header {
        method: input[4],
        level: input[5],
        flags: input[6],
        original_size,
        payload_size,
        checksum: u32::from_le_bytes(input[24..28].try_into().expect("fixed-size field")),
    })
}

fn decode_v2(input: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    let header = parse_v2_header(input)?;
    if header.original_size > limit {
        return Err(Error::OutputLimitExceeded { requested: header.original_size, limit });
    }
    let payload = &input[V2_HEADER_LEN..V2_HEADER_LEN + header.payload_size];
    let output = match header.method {
        METHOD_STORE => {
            if payload.len() != header.original_size {
                return Err(Error::LengthMismatch { expected: header.original_size, actual: payload.len() });
            }
            payload.to_vec()
        }
        METHOD_LZ => decode_lz(payload, header.original_size)?,
        method => return Err(Error::UnsupportedMethod(method)),
    };
    if header.flags & FLAG_CHECKSUM != 0 {
        let actual = crc32(&output);
        if actual != header.checksum {
            return Err(Error::ChecksumMismatch { expected: header.checksum, actual });
        }
    }
    Ok(output)
}

fn decode_v1(input: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    if input.len() < V1_HEADER_LEN {
        return Err(Error::Truncated);
    }
    let expected = usize_from_u64(read_u64(&input[5..13]))?;
    if expected > limit {
        return Err(Error::OutputLimitExceeded { requested: expected, limit });
    }
    let payload = &input[V1_HEADER_LEN..];
    let output = match input[4] {
        METHOD_STORE => payload.to_vec(),
        METHOD_RLE => decode_rle(payload, expected)?,
        method => return Err(Error::UnsupportedMethod(method)),
    };
    if output.len() != expected {
        return Err(Error::LengthMismatch { expected, actual: output.len() });
    }
    Ok(output)
}

fn method_from_byte(method: u8) -> Result<Method, Error> {
    match method {
        METHOD_STORE => Ok(Method::Stored),
        METHOD_RLE => Ok(Method::Rle),
        METHOD_LZ => Ok(Method::Lz),
        value => Err(Error::UnsupportedMethod(value)),
    }
}

fn search_depth(level: u8) -> usize {
    [0, 2, 4, 8, 16, 32, 64, 96, 160, 256][level as usize]
}

fn hash_at(input: &[u8], position: usize) -> usize {
    let value = (input[position] as usize).wrapping_mul(251)
        ^ (input[position + 1] as usize).wrapping_mul(31)
        ^ input[position + 2] as usize;
    value & (HASH_SIZE - 1)
}

fn insert_position(input: &[u8], position: usize, head: &mut [usize], previous: &mut [usize]) {
    if position + MIN_MATCH > input.len() {
        return;
    }
    let hash = hash_at(input, position);
    previous[position] = head[hash];
    head[hash] = position;
}

fn find_match(input: &[u8], position: usize, head: &[usize], previous: &[usize], depth: usize) -> (usize, usize) {
    if position + MIN_MATCH > input.len() {
        return (0, 0);
    }
    let mut candidate = head[hash_at(input, position)];
    let mut best_distance = 0;
    let mut best_length = 0;
    let mut attempts = 0;
    while candidate != NONE && attempts < depth {
        let distance = position - candidate;
        if distance > MAX_DISTANCE {
            break;
        }
        let mut length = 0;
        let max_length = MAX_MATCH.min(input.len() - position);
        while length < max_length && input[candidate + length] == input[position + length] {
            length += 1;
        }
        if length > best_length && length >= MIN_MATCH {
            best_length = length;
            best_distance = distance;
            if length == max_length {
                break;
            }
        }
        candidate = previous[candidate];
        attempts += 1;
    }
    (best_distance, best_length)
}

fn encode_lz(input: &[u8], depth: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len());
    let mut head = vec![NONE; HASH_SIZE];
    let mut previous = vec![NONE; input.len()];
    let mut position = 0;

    while position < input.len() {
        let flags_position = output.len();
        output.push(0);
        let mut flags = 0u8;
        for bit in 0..8 {
            if position == input.len() {
                break;
            }
            let (distance, length) = find_match(input, position, &head, &previous, depth);
            if length >= MIN_MATCH {
                flags |= 1 << bit;
                output.extend_from_slice(&(distance as u16).to_le_bytes());
                output.push(length as u8);
                for inserted in position..position + length {
                    insert_position(input, inserted, &mut head, &mut previous);
                }
                position += length;
            } else {
                output.push(input[position]);
                insert_position(input, position, &mut head, &mut previous);
                position += 1;
            }
        }
        output[flags_position] = flags;
    }
    output
}

fn decode_lz(payload: &[u8], expected: usize) -> Result<Vec<u8>, Error> {
    let mut output = Vec::with_capacity(expected);
    let mut cursor = 0;
    while output.len() < expected {
        let flags = *payload.get(cursor).ok_or(Error::Truncated)?;
        cursor += 1;
        for bit in 0..8 {
            if output.len() == expected {
                break;
            }
            if flags & (1 << bit) == 0 {
                output.push(*payload.get(cursor).ok_or(Error::Truncated)?);
                cursor += 1;
            } else {
                if cursor + 3 > payload.len() {
                    return Err(Error::Truncated);
                }
                let distance = u16::from_le_bytes([payload[cursor], payload[cursor + 1]]) as usize;
                let length = payload[cursor + 2] as usize;
                cursor += 3;
                if distance == 0 || distance > output.len() || length < MIN_MATCH || output.len().saturating_add(length) > expected {
                    return Err(Error::InvalidMatch);
                }
                for _ in 0..length {
                    let source = output.len() - distance;
                    output.push(output[source]);
                }
            }
        }
    }
    if cursor != payload.len() {
        return Err(Error::TrailingData);
    }
    Ok(output)
}

fn decode_rle(payload: &[u8], expected: usize) -> Result<Vec<u8>, Error> {
    if payload.len() % 2 != 0 {
        return Err(Error::InvalidRun);
    }
    let mut output = Vec::with_capacity(expected);
    for pair in payload.chunks_exact(2) {
        let count = pair[0] as usize;
        if count == 0 || output.len().saturating_add(count) > expected {
            return Err(Error::InvalidRun);
        }
        output.extend(std::iter::repeat_n(pair[1], count));
    }
    Ok(output)
}

fn read_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("fixed-size field"))
}

fn usize_from_u64(value: u64) -> Result<usize, Error> {
    usize::try_from(value).map_err(|_| Error::InputTooLarge)
}

fn crc32(input: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in input {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(feature = "c-api")]
mod c_api;

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(data: &[u8]) {
        for level in 1..=9 {
            let encoded = compress_with_level(data, level).unwrap();
            assert_eq!(decompress(&encoded).unwrap(), data);
        }
    }

    #[test]
    fn round_trips_empty_input() {
        round_trip(b"");
    }

    #[test]
    fn round_trips_text_and_binary() {
        round_trip(b"the quick brown fox; the quick brown fox; the quick brown fox");
        round_trip(&(0u8..=255).collect::<Vec<_>>());
    }

    #[test]
    fn round_trips_overlapping_matches() {
        round_trip(&vec![b'a'; 100_000]);
    }

    #[test]
    fn compresses_repetition() {
        let input = vec![42; 10_000];
        assert!(compress(&input).len() < input.len() / 10);
    }

    #[test]
    fn detects_corruption() {
        let mut encoded = compress(&vec![7; 4096]);
        let last = encoded.len() - 1;
        encoded[last] ^= 1;
        assert!(decompress(&encoded).is_err());
    }

    #[test]
    fn enforces_output_limit() {
        let encoded = compress(&vec![0; 1024]);
        assert!(matches!(decompress_with_limit(&encoded, 100), Err(Error::OutputLimitExceeded { .. })));
    }

    #[test]
    fn rejects_invalid_level() {
        assert_eq!(compress_with_level(b"data", 0), Err(Error::InvalidLevel(0)));
    }

    #[test]
    fn reads_frame_information() {
        let encoded = compress_with_level(b"metadata", 4).unwrap();
        let info = frame_info(&encoded).unwrap();
        assert_eq!(info.version, 2);
        assert_eq!(info.level, 4);
        assert_eq!(info.original_size, 8);
        assert!(info.checksum);
    }

    #[test]
    fn decodes_v1_stored_frames() {
        let mut frame = b"XCA1".to_vec();
        frame.push(METHOD_STORE);
        frame.extend_from_slice(&4u64.to_le_bytes());
        frame.extend_from_slice(b"data");
        assert_eq!(decompress(&frame).unwrap(), b"data");
    }
}
