//! XCA's versioned container and adaptive codec prototype.

use std::fmt;

const MAGIC: &[u8; 4] = b"XCA1";
const HEADER_LEN: usize = 13;
const METHOD_STORE: u8 = 0;
const METHOD_RLE: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Truncated,
    BadMagic,
    UnsupportedMethod(u8),
    LengthMismatch { expected: usize, actual: usize },
    InvalidRun,
    InputTooLarge,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated XCA stream"),
            Self::BadMagic => write!(f, "invalid XCA magic"),
            Self::UnsupportedMethod(method) => write!(f, "unsupported XCA method {method}"),
            Self::LengthMismatch { expected, actual } => {
                write!(f, "decoded length mismatch: expected {expected}, got {actual}")
            }
            Self::InvalidRun => write!(f, "invalid RLE payload"),
            Self::InputTooLarge => write!(f, "input is too large for this platform"),
        }
    }
}

impl std::error::Error for Error {}

/// Compresses bytes into a versioned XCA frame.
///
/// The prototype selects RLE only when it beats verbatim storage. Future
/// versions will keep the frame stable while adding stronger pipelines.
pub fn compress(input: &[u8]) -> Vec<u8> {
    let rle = encode_rle(input);
    let (method, payload) = if rle.len() < input.len() {
        (METHOD_RLE, rle.as_slice())
    } else {
        (METHOD_STORE, input)
    };

    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(MAGIC);
    out.push(method);
    out.extend_from_slice(&(input.len() as u64).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// Decompresses one complete XCA frame and validates its decoded length.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    if input.len() < HEADER_LEN {
        return Err(Error::Truncated);
    }
    if &input[..4] != MAGIC {
        return Err(Error::BadMagic);
    }

    let expected_u64 = u64::from_le_bytes(input[5..13].try_into().expect("fixed-size header"));
    let expected = usize::try_from(expected_u64).map_err(|_| Error::InputTooLarge)?;
    let payload = &input[HEADER_LEN..];
    let decoded = match input[4] {
        METHOD_STORE => payload.to_vec(),
        METHOD_RLE => decode_rle(payload, expected)?,
        method => return Err(Error::UnsupportedMethod(method)),
    };

    if decoded.len() != expected {
        return Err(Error::LengthMismatch {
            expected,
            actual: decoded.len(),
        });
    }
    Ok(decoded)
}

fn encode_rle(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < input.len() {
        let value = input[index];
        let mut count = 1usize;
        while index + count < input.len() && input[index + count] == value && count < 255 {
            count += 1;
        }
        out.push(count as u8);
        out.push(value);
        index += count;
    }
    out
}

fn decode_rle(payload: &[u8], expected: usize) -> Result<Vec<u8>, Error> {
    if payload.len() % 2 != 0 {
        return Err(Error::InvalidRun);
    }
    let mut out = Vec::with_capacity(expected);
    for pair in payload.chunks_exact(2) {
        let count = pair[0] as usize;
        if count == 0 || out.len().saturating_add(count) > expected {
            return Err(Error::InvalidRun);
        }
        out.extend(std::iter::repeat_n(pair[1], count));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(data: &[u8]) {
        let encoded = compress(data);
        let decoded = decompress(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn round_trips_empty_input() {
        round_trip(b"");
    }

    #[test]
    fn round_trips_mixed_input() {
        round_trip(b"aaaabbbccd-0123456789");
    }

    #[test]
    fn round_trips_long_runs() {
        round_trip(&vec![42; 1024]);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut encoded = compress(b"data");
        encoded[0] = b'!';
        assert_eq!(decompress(&encoded), Err(Error::BadMagic));
    }
}
