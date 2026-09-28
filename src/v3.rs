use super::{crc32, decode_lz, decode_rle, encode_lz, encode_rle, Error, FrameInfo, Method};
use std::io::{Cursor, Read, Write};
const MAGIC: &[u8; 4] = b"XCA3";
const HEADER: usize = 12;
const BH: usize = 13;
const END: u8 = 255;
const STORE: u8 = 0;
const RLE: u8 = 1;
const LZ: u8 = 2;
const DELTA: u8 = 3;
const XOR: u8 = 4;
const MIN: usize = 4096;
const MAX: usize = 16 * 1024 * 1024;
const DEFAULT: usize = 256 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressionOptions {
    pub level: u8,
    pub block_size: usize,
}
impl Default for CompressionOptions {
    fn default() -> Self {
        Self { level: 6, block_size: DEFAULT }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamStats {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub blocks: u32,
}
pub fn compress_slice(input: &[u8], level: u8) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    compress_stream(&mut Cursor::new(input), &mut out, CompressionOptions { level, ..Default::default() })?;
    Ok(out)
}
pub fn decompress_slice(input: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    let mut r = Cursor::new(input); let mut out = Vec::new();
    decompress_stream(&mut r, &mut out, limit)?;
    if r.position() as usize != input.len() { return Err(Error::TrailingData); }
    Ok(out)
}
pub fn compress_stream<R: Read, W: Write>(reader: &mut R, writer: &mut W, opt: CompressionOptions) -> Result<StreamStats, Error> {
    valid(opt)?;
    let mut st = StreamStats { output_bytes: HEADER as u64, ..Default::default() };
    writer.write_all(MAGIC).map_err(ioe)?;
    writer.write_all(&[opt.level, 1, 0, 0]).map_err(ioe)?;
    writer.write_all(&(opt.block_size as u32).to_le_bytes()).map_err(ioe)?;
    let mut buf = vec![0; opt.block_size];
    loop {
        let n = read_block(reader, &mut buf)?; if n == 0 { break; }
        let e = encode(&buf[..n], opt.level);
        writer.write_all(&[e.0]).map_err(ioe)?;
        writer.write_all(&(n as u32).to_le_bytes()).map_err(ioe)?;
        writer.write_all(&(e.1.len() as u32).to_le_bytes()).map_err(ioe)?;
        writer.write_all(&crc32(&buf[..n]).to_le_bytes()).map_err(ioe)?;
        writer.write_all(&e.1).map_err(ioe)?;
        st.input_bytes += n as u64; st.output_bytes += (BH + e.1.len()) as u64;
        st.blocks = st.blocks.checked_add(1).ok_or(Error::InputTooLarge)?
    }
    writer.write_all(&[END]).map_err(ioe)?; writer.write_all(&[0; 12]).map_err(ioe)?;
    st.output_bytes += BH as u64; Ok(st)
}
pub fn decompress_stream<R: Read, W: Write>(reader: &mut R, writer: &mut W, limit: usize) -> Result<StreamStats, Error> {
    let mut h = [0; HEADER]; read_exact(reader, &mut h)?;
    if &h[..4] != MAGIC { return Err(Error::BadMagic); }
    if !(1..=9).contains(&h[4]) { return Err(Error::InvalidLevel(h[4])); }
    let bs = u32::from_le_bytes(h[8..12].try_into().unwrap()) as usize;
    if !(MIN..=MAX).contains(&bs) { return Err(Error::InvalidBlockSize(bs)); }
    let mut st = StreamStats { input_bytes: HEADER as u64, ..Default::default() };
    loop {
        let mut b = [0; BH]; read_exact(reader, &mut b)?; st.input_bytes += BH as u64;
        let m = b[0]; let n = u32::from_le_bytes(b[1..5].try_into().unwrap()) as usize;
        let pn = u32::from_le_bytes(b[5..9].try_into().unwrap()) as usize;
        let crc = u32::from_le_bytes(b[9..13].try_into().unwrap());
        if m == END { if n != 0 || pn != 0 || crc != 0 { return Err(Error::InvalidMatch); } break; }
        if n == 0 || n > bs || pn > n { return Err(Error::InvalidBlockSize(n)); }
        let total = (st.output_bytes as usize).checked_add(n).ok_or(Error::InputTooLarge)?;
        if total > limit { return Err(Error::OutputLimitExceeded { requested: total, limit }); }
        let mut p = vec![0; pn]; read_exact(reader, &mut p)?; st.input_bytes += pn as u64;
        let data = decode(m, &p, n)?; let actual = crc32(&data);
        if actual != crc { return Err(Error::ChecksumMismatch { expected: crc, actual }); }
        writer.write_all(&data).map_err(ioe)?; st.output_bytes += n as u64;
        st.blocks = st.blocks.checked_add(1).ok_or(Error::InputTooLarge)?
    }
    Ok(st)
}
pub fn frame_info(input: &[u8]) -> Result<FrameInfo, Error> {
    if input.len() < HEADER || &input[..4] != MAGIC { return Err(Error::Truncated); }
    let bs = u32::from_le_bytes(input[8..12].try_into().unwrap()) as usize;
    if !(MIN..=MAX).contains(&bs) { return Err(Error::InvalidBlockSize(bs)); }
    let (mut c, mut blocks, mut size) = (HEADER, 0u32, 0usize);
    while c + BH <= input.len() {
        let m = input[c]; let n = u32::from_le_bytes(input[c + 1..c + 5].try_into().unwrap()) as usize;
        let pn = u32::from_le_bytes(input[c + 5..c + 9].try_into().unwrap()) as usize; c += BH;
        if m == END {
            if n != 0 || pn != 0 || c != input.len() { return Err(Error::TrailingData); }
            return Ok(FrameInfo { version: 3, method: Method::Adaptive, level: input[4], original_size: size, frame_size: input.len(), checksum: true, blocks });
        }
        if n == 0 || n > bs || pn > n { return Err(Error::InvalidBlockSize(n)); }
        method(m)?; c = c.checked_add(pn).ok_or(Error::InputTooLarge)?;
        if c > input.len() { return Err(Error::Truncated); }
        size = size.checked_add(n).ok_or(Error::InputTooLarge)?;
        blocks = blocks.checked_add(1).ok_or(Error::InputTooLarge)?
    }
    Err(Error::Truncated)
}
fn encode(input: &[u8], level: u8) -> (u8, Vec<u8>) {
    let d = super::search_depth(level); let mut best = (STORE, input.to_vec());
    candidate(&mut best, RLE, encode_rle(input)); candidate(&mut best, LZ, encode_lz(input, d));
    candidate(&mut best, DELTA, encode_lz(&forward(input, false), d));
    candidate(&mut best, XOR, encode_lz(&forward(input, true), d)); best
}
fn candidate(best: &mut (u8, Vec<u8>), m: u8, p: Vec<u8>) { if p.len() < best.1.len() { *best = (m, p) } }
fn decode(m: u8, p: &[u8], n: usize) -> Result<Vec<u8>, Error> {
    match m {
        STORE if p.len() == n => Ok(p.to_vec()), STORE => Err(Error::LengthMismatch { expected: n, actual: p.len() }),
        RLE => decode_rle(p, n), LZ => decode_lz(p, n), DELTA => decode_lz(p, n).map(|v| inverse(v, false)),
        XOR => decode_lz(p, n).map(|v| inverse(v, true)), x => Err(Error::UnsupportedMethod(x)),
    }
}
fn method(m: u8) -> Result<Method, Error> { match m { STORE => Ok(Method::Stored), RLE => Ok(Method::Rle), LZ => Ok(Method::Lz), DELTA => Ok(Method::DeltaLz), XOR => Ok(Method::XorLz), x => Err(Error::UnsupportedMethod(x)) } }
fn forward(input: &[u8], xor: bool) -> Vec<u8> {
    if input.is_empty() { return Vec::new(); } let mut o = Vec::with_capacity(input.len()); o.push(input[0]);
    o.extend(input.windows(2).map(|p| if xor { p[1] ^ p[0] } else { p[1].wrapping_sub(p[0]) })); o
}
fn inverse(mut v: Vec<u8>, xor: bool) -> Vec<u8> { for i in 1..v.len() { v[i] = if xor { v[i] ^ v[i - 1] } else { v[i].wrapping_add(v[i - 1]) } } v }
fn valid(o: CompressionOptions) -> Result<(), Error> {
    if !(1..=9).contains(&o.level) { return Err(Error::InvalidLevel(o.level)); }
    if !(MIN..=MAX).contains(&o.block_size) { return Err(Error::InvalidBlockSize(o.block_size)); } Ok(())
}
fn read_block<R: Read>(r: &mut R, b: &mut [u8]) -> Result<usize, Error> { let mut n = 0; while n < b.len() { match r.read(&mut b[n..]).map_err(ioe)? { 0 => break, x => n += x } } Ok(n) }
fn read_exact<R: Read>(r: &mut R, b: &mut [u8]) -> Result<(), Error> { r.read_exact(b).map_err(|e| if e.kind() == std::io::ErrorKind::UnexpectedEof { Error::Truncated } else { ioe(e) }) }
fn ioe(e: std::io::Error) -> Error { Error::Io(e.to_string()) }
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn streams_across_multiple_blocks() {
        let input: Vec<u8> = (0..25_000).map(|i| (i % 251) as u8).collect(); let mut encoded = Vec::new();
        let stats = compress_stream(&mut Cursor::new(&input), &mut encoded, CompressionOptions { level: 7, block_size: MIN }).unwrap();
        assert!(stats.blocks > 1); let mut decoded = Vec::new();
        let decoded_stats = decompress_stream(&mut Cursor::new(&encoded), &mut decoded, input.len()).unwrap();
        assert_eq!(decoded, input); assert_eq!(decoded_stats.blocks, stats.blocks);
    }
    #[test] fn selects_predictive_transform_for_counter_data() {
        let input: Vec<u8> = (0..4096).map(|i| i as u8).collect(); let encoded = compress_slice(&input, 6).unwrap();
        assert!(matches!(encoded[HEADER], DELTA | XOR)); assert_eq!(decompress_slice(&encoded, input.len()).unwrap(), input);
    }
    #[test] fn rejects_truncated_stream() { let mut encoded = compress_slice(b"independent codec", 6).unwrap(); encoded.pop(); assert_eq!(decompress_slice(&encoded, 1024), Err(Error::Truncated)); }
}
