use crate::{crc32, Error, FrameInfo, Method};
use std::io::{Read, Write};
use std::thread;
const MAGIC: &[u8; 4] = b"XCA4";
const HEADER: usize = 12;
const BH: usize = 14;
const END: u8 = 255;
const STORED: u8 = 0;
const PULSE: u8 = 1;
const NONE_P: u8 = 0;
const DELTA: u8 = 1;
const XOR: u8 = 2;
const MIN_BLOCK: usize = 4096;
const MAX_BLOCK: usize = 16 * 1024 * 1024;
const DEFAULT_BLOCK: usize = 256 * 1024;
const HS: usize = 1 << 16;
const MAX_D: usize = u16::MAX as usize;
const MIN_M: usize = 4;
const MAX_M: usize = u16::MAX as usize;
const NONE: usize = usize::MAX;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressionOptions {
    pub level: u8,
    pub block_size: usize,
}
impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            level: 5,
            block_size: DEFAULT_BLOCK,
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamStats {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub blocks: u32,
}
struct FastWorkspace {
    positions: Vec<u32>,
    stamps: Vec<u32>,
    generation: u32,
}
impl FastWorkspace {
    fn new() -> Self {
        Self {
            positions: vec![0; HS],
            stamps: vec![0; HS],
            generation: 0,
        }
    }
    fn begin_block(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamps.fill(0);
            self.generation = 1;
        }
    }
    fn get(&self, hash: usize) -> Option<usize> {
        (self.stamps[hash] == self.generation).then_some(self.positions[hash] as usize)
    }
    fn set(&mut self, hash: usize, position: usize) {
        self.positions[hash] = position as u32;
        self.stamps[hash] = self.generation;
    }
}
pub fn compress_slice(input: &[u8], level: u8) -> Result<Vec<u8>, Error> {
    let options = CompressionOptions {
        level,
        ..Default::default()
    };
    valid(options)?;
    let count = input.len().div_ceil(options.block_size);
    let workers = thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(count.max(1));
    let blocks = thread::scope(|scope| {
        let mut handles = Vec::new();
        for worker in 0..workers {
            handles.push(scope.spawn(move || {
                let mut local = Vec::new();
                let mut fast = FastWorkspace::new();
                for index in (worker..count).step_by(workers) {
                    let start = index * options.block_size;
                    let end = (start + options.block_size).min(input.len());
                    local.push((index, encode_block(&input[start..end], level, &mut fast)));
                }
                local
            }));
        }
        let mut ordered: Vec<Option<EncodedBlock>> = (0..count).map(|_| None).collect();
        for handle in handles {
            for (index, block) in handle.join().expect("XCA worker panicked") {
                ordered[index] = Some(block);
            }
        }
        ordered.into_iter().map(Option::unwrap).collect::<Vec<_>>()
    });
    let capacity = HEADER + BH + blocks.iter().map(|b| BH + b.payload.len()).sum::<usize>();
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&[level, 1, 0, 0]);
    output.extend_from_slice(&(options.block_size as u32).to_le_bytes());
    for block in blocks {
        write_block(&mut output, &block);
    }
    output.extend_from_slice(&[END, 0]);
    output.extend_from_slice(&[0; 12]);
    Ok(output)
}
#[derive(Clone, Copy)]
struct BlockDescriptor<'a> {
    method: u8,
    predictor: u8,
    original: usize,
    payload: &'a [u8],
    checksum: u32,
}
fn decode_block(block: BlockDescriptor<'_>) -> Result<Vec<u8>, Error> {
    let mut decoded = match block.method {
        STORED if block.predictor == NONE_P && block.payload.len() == block.original => {
            block.payload.to_vec()
        }
        PULSE => decode(block.payload, block.original)?,
        STORED => return Err(Error::InvalidMatch),
        value => return Err(Error::UnsupportedMethod(value)),
    };
    inverse(&mut decoded, block.predictor)?;
    let actual = crc32(&decoded);
    if actual != block.checksum {
        return Err(Error::ChecksumMismatch {
            expected: block.checksum,
            actual,
        });
    }
    Ok(decoded)
}
pub fn decompress_slice(input: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    if input.len() < HEADER || &input[..4] != MAGIC {
        return Err(Error::BadMagic);
    }
    if !(1..=9).contains(&input[4]) {
        return Err(Error::InvalidLevel(input[4]));
    }
    let block_size = u32::from_le_bytes(input[8..12].try_into().unwrap()) as usize;
    if !(MIN_BLOCK..=MAX_BLOCK).contains(&block_size) {
        return Err(Error::InvalidBlockSize(block_size));
    }
    let mut cursor = HEADER;
    let mut total = 0usize;
    let mut descriptors = Vec::new();
    loop {
        if cursor + BH > input.len() {
            return Err(Error::Truncated);
        }
        let header = &input[cursor..cursor + BH];
        cursor += BH;
        let method = header[0];
        let predictor = header[1];
        let original = u32::from_le_bytes(header[2..6].try_into().unwrap()) as usize;
        let packed = u32::from_le_bytes(header[6..10].try_into().unwrap()) as usize;
        let checksum = u32::from_le_bytes(header[10..14].try_into().unwrap());
        if method == END {
            if predictor != 0 || original != 0 || packed != 0 || checksum != 0 {
                return Err(Error::InvalidMatch);
            }
            if cursor != input.len() {
                return Err(Error::TrailingData);
            }
            break;
        }
        if original == 0 || original > block_size || packed > original {
            return Err(Error::InvalidBlockSize(original));
        }
        total = total.checked_add(original).ok_or(Error::InputTooLarge)?;
        if total > limit {
            return Err(Error::OutputLimitExceeded {
                requested: total,
                limit,
            });
        }
        let end = cursor.checked_add(packed).ok_or(Error::InputTooLarge)?;
        if end > input.len() {
            return Err(Error::Truncated);
        }
        descriptors.push(BlockDescriptor {
            method,
            predictor,
            original,
            payload: &input[cursor..end],
            checksum,
        });
        cursor = end;
    }
    let count = descriptors.len();
    let workers = thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(count.max(1));
    let decoded = thread::scope(|scope| {
        let mut handles = Vec::new();
        for worker in 0..workers {
            let descriptors = &descriptors;
            handles.push(scope.spawn(move || {
                let mut local = Vec::new();
                for index in (worker..count).step_by(workers) {
                    local.push((index, decode_block(descriptors[index])));
                }
                local
            }));
        }
        let mut ordered: Vec<Option<Result<Vec<u8>, Error>>> = (0..count).map(|_| None).collect();
        for handle in handles {
            for (index, block) in handle.join().expect("XCA decoder worker panicked") {
                ordered[index] = Some(block);
            }
        }
        ordered
    });
    let mut output = Vec::with_capacity(total);
    for block in decoded {
        let mut block = block.expect("every XCA block was scheduled")?;
        output.append(&mut block);
    }
    Ok(output)
}
struct EncodedBlock {
    method: u8,
    predictor: u8,
    original: u32,
    payload: Vec<u8>,
    checksum: u32,
}
fn encode_block(input: &[u8], level: u8, fast: &mut FastWorkspace) -> EncodedBlock {
    let predictor = if level <= 2 { NONE_P } else { choose(input) };
    let packed = if predictor == NONE_P {
        encode(input, level, fast)
    } else {
        let transformed = transform(input, predictor);
        encode(&transformed, level, fast)
    };
    if packed.len() < input.len() {
        EncodedBlock {
            method: PULSE,
            predictor,
            original: input.len() as u32,
            payload: packed,
            checksum: crc32(input),
        }
    } else {
        EncodedBlock {
            method: STORED,
            predictor: NONE_P,
            original: input.len() as u32,
            payload: input.to_vec(),
            checksum: crc32(input),
        }
    }
}
fn write_block<W: Write>(writer: &mut W, block: &EncodedBlock) {
    writer
        .write_all(&[block.method, block.predictor])
        .expect("writing to memory cannot fail");
    writer
        .write_all(&block.original.to_le_bytes())
        .expect("writing to memory cannot fail");
    writer
        .write_all(&(block.payload.len() as u32).to_le_bytes())
        .expect("writing to memory cannot fail");
    writer
        .write_all(&block.checksum.to_le_bytes())
        .expect("writing to memory cannot fail");
    writer
        .write_all(&block.payload)
        .expect("writing to memory cannot fail");
}
pub fn compress_stream<R: Read, W: Write>(
    r: &mut R,
    w: &mut W,
    opt: CompressionOptions,
) -> Result<StreamStats, Error> {
    valid(opt)?;
    w.write_all(MAGIC).map_err(ioe)?;
    w.write_all(&[opt.level, 1, 0, 0]).map_err(ioe)?;
    w.write_all(&(opt.block_size as u32).to_le_bytes())
        .map_err(ioe)?;
    let mut st = StreamStats {
        output_bytes: HEADER as u64,
        ..Default::default()
    };
    let mut b = vec![0; opt.block_size];
    let mut fast = FastWorkspace::new();
    loop {
        let n = read_block(r, &mut b)?;
        if n == 0 {
            break;
        }
        let input = &b[..n];
        let pred = if opt.level <= 2 {
            NONE_P
        } else {
            choose(input)
        };
        let packed = if pred == NONE_P {
            encode(input, opt.level, &mut fast)
        } else {
            let transformed = transform(input, pred);
            encode(&transformed, opt.level, &mut fast)
        };
        let (m, p, sp) = if packed.len() < input.len() {
            (PULSE, packed.as_slice(), pred)
        } else {
            (STORED, input, NONE_P)
        };
        w.write_all(&[m, sp]).map_err(ioe)?;
        w.write_all(&(n as u32).to_le_bytes()).map_err(ioe)?;
        w.write_all(&(p.len() as u32).to_le_bytes()).map_err(ioe)?;
        w.write_all(&crc32(input).to_le_bytes()).map_err(ioe)?;
        w.write_all(p).map_err(ioe)?;
        st.input_bytes += n as u64;
        st.output_bytes += (BH + p.len()) as u64;
        st.blocks = st.blocks.checked_add(1).ok_or(Error::InputTooLarge)?
    }
    w.write_all(&[END, 0]).map_err(ioe)?;
    w.write_all(&[0; 12]).map_err(ioe)?;
    st.output_bytes += BH as u64;
    Ok(st)
}
pub fn decompress_stream<R: Read, W: Write>(
    r: &mut R,
    w: &mut W,
    limit: usize,
) -> Result<StreamStats, Error> {
    let mut h = [0; HEADER];
    exact(r, &mut h)?;
    if &h[..4] != MAGIC {
        return Err(Error::BadMagic);
    }
    if !(1..=9).contains(&h[4]) {
        return Err(Error::InvalidLevel(h[4]));
    }
    let bs = u32::from_le_bytes(h[8..12].try_into().unwrap()) as usize;
    if !(MIN_BLOCK..=MAX_BLOCK).contains(&bs) {
        return Err(Error::InvalidBlockSize(bs));
    }
    let mut st = StreamStats {
        input_bytes: HEADER as u64,
        ..Default::default()
    };
    loop {
        let mut h = [0; BH];
        exact(r, &mut h)?;
        st.input_bytes += BH as u64;
        let m = h[0];
        let pred = h[1];
        let n = u32::from_le_bytes(h[2..6].try_into().unwrap()) as usize;
        let pn = u32::from_le_bytes(h[6..10].try_into().unwrap()) as usize;
        let crc = u32::from_le_bytes(h[10..14].try_into().unwrap());
        if m == END {
            if pred != 0 || n != 0 || pn != 0 || crc != 0 {
                return Err(Error::InvalidMatch);
            }
            break;
        }
        if n == 0 || n > bs || pn > n {
            return Err(Error::InvalidBlockSize(n));
        }
        let total = (st.output_bytes as usize)
            .checked_add(n)
            .ok_or(Error::InputTooLarge)?;
        if total > limit {
            return Err(Error::OutputLimitExceeded {
                requested: total,
                limit,
            });
        }
        let mut p = vec![0; pn];
        exact(r, &mut p)?;
        st.input_bytes += pn as u64;
        let mut data = match m {
            STORED if pred == NONE_P && p.len() == n => p,
            PULSE => decode(&p, n)?,
            STORED => return Err(Error::InvalidMatch),
            x => return Err(Error::UnsupportedMethod(x)),
        };
        inverse(&mut data, pred)?;
        let actual = crc32(&data);
        if actual != crc {
            return Err(Error::ChecksumMismatch {
                expected: crc,
                actual,
            });
        }
        w.write_all(&data).map_err(ioe)?;
        st.output_bytes += n as u64;
        st.blocks = st.blocks.checked_add(1).ok_or(Error::InputTooLarge)?
    }
    Ok(st)
}
pub fn frame_info(i: &[u8]) -> Result<FrameInfo, Error> {
    if i.len() < HEADER || &i[..4] != MAGIC {
        return Err(Error::BadMagic);
    }
    let bs = u32::from_le_bytes(i[8..12].try_into().unwrap()) as usize;
    if !(MIN_BLOCK..=MAX_BLOCK).contains(&bs) {
        return Err(Error::InvalidBlockSize(bs));
    }
    let (mut c, mut blocks, mut original) = (HEADER, 0u32, 0usize);
    while c + BH <= i.len() {
        let m = i[c];
        let pred = i[c + 1];
        let n = u32::from_le_bytes(i[c + 2..c + 6].try_into().unwrap()) as usize;
        let pn = u32::from_le_bytes(i[c + 6..c + 10].try_into().unwrap()) as usize;
        c += BH;
        if m == END {
            if pred != 0 || n != 0 || pn != 0 || c != i.len() {
                return Err(Error::TrailingData);
            }
            return Ok(FrameInfo {
                version: 4,
                method: Method::Adaptive,
                level: i[4],
                original_size: original,
                frame_size: i.len(),
                checksum: true,
                blocks,
            });
        }
        if n == 0 || n > bs || pn > n {
            return Err(Error::InvalidBlockSize(n));
        }
        if !matches!(m, STORED | PULSE) || pred > XOR {
            return Err(Error::UnsupportedMethod(m));
        }
        c = c.checked_add(pn).ok_or(Error::InputTooLarge)?;
        if c > i.len() {
            return Err(Error::Truncated);
        }
        original = original.checked_add(n).ok_or(Error::InputTooLarge)?;
        blocks = blocks.checked_add(1).ok_or(Error::InputTooLarge)?
    }
    Err(Error::Truncated)
}
fn choose(i: &[u8]) -> u8 {
    let n = i.len().min(8192);
    if n < 64 {
        return NONE_P;
    }
    let s = &i[..n];
    let raw = score(s, NONE_P);
    let d = score(s, DELTA);
    let x = score(s, XOR);
    let margin = n / 64;
    if d > raw + margin && d >= x {
        DELTA
    } else if x > raw + margin {
        XOR
    } else {
        NONE_P
    }
}
fn predicted(input: &[u8], position: usize, predictor: u8) -> u8 {
    if position == 0 || predictor == NONE_P {
        input[position]
    } else if predictor == DELTA {
        input[position].wrapping_sub(input[position - 1])
    } else {
        input[position] ^ input[position - 1]
    }
}
fn score(input: &[u8], predictor: u8) -> usize {
    if input.len() < 3 {
        return 0;
    }
    let mut table = vec![NONE; 4096];
    let mut result = 0;
    for position in 0..input.len() - 2 {
        let a = predicted(input, position, predictor);
        let b = predicted(input, position + 1, predictor);
        let c = predicted(input, position + 2, predictor);
        let hash =
            ((a as usize).wrapping_mul(251) ^ (b as usize).wrapping_mul(31) ^ c as usize) & 4095;
        let previous = table[hash];
        if previous != NONE
            && predicted(input, previous, predictor) == a
            && predicted(input, previous + 1, predictor) == b
            && predicted(input, previous + 2, predictor) == c
        {
            result += 1;
        }
        table[hash] = position;
    }
    result
}
fn transform(i: &[u8], pred: u8) -> Vec<u8> {
    if pred == NONE_P {
        return i.to_vec();
    }
    if i.is_empty() {
        return Vec::new();
    }
    let mut o = Vec::with_capacity(i.len());
    o.push(i[0]);
    o.extend(i.windows(2).map(|p| {
        if pred == DELTA {
            p[1].wrapping_sub(p[0])
        } else {
            p[1] ^ p[0]
        }
    }));
    o
}
fn inverse(d: &mut [u8], pred: u8) -> Result<(), Error> {
    match pred {
        NONE_P => return Ok(()),
        DELTA => {
            for i in 1..d.len() {
                d[i] = d[i].wrapping_add(d[i - 1])
            }
        }
        XOR => {
            for i in 1..d.len() {
                d[i] ^= d[i - 1]
            }
        }
        _ => return Err(Error::UnsupportedMethod(pred)),
    }
    Ok(())
}
fn hash(i: &[u8], p: usize) -> usize {
    ((i[p] as usize).wrapping_mul(251) ^ (i[p + 1] as usize).wrapping_mul(31) ^ i[p + 2] as usize)
        & (HS - 1)
}
fn depth(l: u8) -> usize {
    [0, 1, 2, 4, 8, 16, 24, 40, 64, 96][l as usize]
}
fn insert(i: &[u8], p: usize, h: &mut [usize], prev: &mut [usize]) {
    if p + 3 > i.len() {
        return;
    }
    let x = hash(i, p);
    prev[p] = h[x];
    h[x] = p
}
fn find(i: &[u8], p: usize, h: &[usize], prev: &[usize], md: usize) -> (usize, usize) {
    if p + MIN_M > i.len() {
        return (0, 0);
    }
    let mut c = h[hash(i, p)];
    let (mut bd, mut bl, mut tries) = (0, 0, 0);
    while c != NONE && tries < md {
        let d = p - c;
        if d > MAX_D {
            break;
        }
        let max = MAX_M.min(i.len() - p);
        let mut l = 0;
        while l < max && i[c + l] == i[p + l] {
            l += 1;
        }
        if l > bl && l >= MIN_M {
            bl = l;
            bd = d;
            if l == max {
                break;
            }
        }
        c = prev[c];
        tries += 1
    }
    (bd, bl)
}
fn common_prefix(i: &[u8], a: usize, b: usize, max: usize) -> usize {
    let mut length = 0;
    while length + 8 <= max {
        let left = &i[a + length..a + length + 8];
        let right = &i[b + length..b + length + 8];
        if left == right {
            length += 8;
        } else {
            while length < max && i[a + length] == i[b + length] {
                length += 1;
            }
            return length;
        }
    }
    while length < max && i[a + length] == i[b + length] {
        length += 1;
    }
    length
}
fn encode(i: &[u8], level: u8, fast: &mut FastWorkspace) -> Vec<u8> {
    if level <= 3 {
        encode_fast(i, level, fast)
    } else {
        encode_dense(i, level)
    }
}
fn encode_fast(i: &[u8], level: u8, workspace: &mut FastWorkspace) -> Vec<u8> {
    let mut output = Vec::with_capacity(i.len());
    workspace.begin_block();
    let (mut position, mut literal_start) = (0, 0);
    while position < i.len() {
        let mut found = (0, 0);
        if position + MIN_M <= i.len() {
            if let Some(candidate) = workspace.get(hash(i, position)) {
                let distance = position - candidate;
                if distance <= MAX_D {
                    let max = MAX_M.min(i.len() - position);
                    let length = common_prefix(i, candidate, position, max);
                    if length >= MIN_M {
                        found = (distance, length);
                    }
                }
            }
        }
        if found.1 >= MIN_M {
            literals(&mut output, &i[literal_start..position]);
            match_token(&mut output, found.0, found.1);
            let step = if level == 1 { 8 } else { 4 };
            let mut at = position;
            while at < position + found.1 {
                if at + 3 <= i.len() {
                    workspace.set(hash(i, at), at);
                }
                at += step;
            }
            position += found.1;
            literal_start = position;
        } else {
            if position + 3 <= i.len() {
                workspace.set(hash(i, position), position);
            }
            position += 1;
            if position - literal_start == 128 {
                literals(&mut output, &i[literal_start..position]);
                literal_start = position;
            }
        }
    }
    literals(&mut output, &i[literal_start..]);
    output
}
fn encode_dense(i: &[u8], level: u8) -> Vec<u8> {
    let mut o = Vec::with_capacity(i.len());
    let mut h = vec![NONE; HS];
    let mut prev = vec![NONE; i.len()];
    let (mut p, mut lit) = (0, 0);
    while p < i.len() {
        let (d, l) = find(i, p, &h, &prev, depth(level));
        if l >= MIN_M {
            literals(&mut o, &i[lit..p]);
            match_token(&mut o, d, l);
            let step = if level <= 3 { 4 } else { 1 };
            let mut x = p;
            while x < p + l {
                insert(i, x, &mut h, &mut prev);
                x += step
            }
            p += l;
            lit = p
        } else {
            insert(i, p, &mut h, &mut prev);
            p += 1;
            if p - lit == 128 {
                literals(&mut o, &i[lit..p]);
                lit = p
            }
        }
    }
    literals(&mut o, &i[lit..]);
    o
}
fn literals(o: &mut Vec<u8>, mut d: &[u8]) {
    while !d.is_empty() {
        let n = d.len().min(128);
        o.push((n - 1) as u8);
        o.extend_from_slice(&d[..n]);
        d = &d[n..]
    }
}
fn match_token(o: &mut Vec<u8>, d: usize, l: usize) {
    if l <= 67 {
        o.push(0x80 | ((l - 4) as u8))
    } else {
        o.push(0xc0);
        put_var(o, l as u32)
    }
    o.extend_from_slice(&(d as u16).to_le_bytes())
}
fn decode(i: &[u8], expected: usize) -> Result<Vec<u8>, Error> {
    let mut o = Vec::with_capacity(expected);
    let mut p = 0;
    while o.len() < expected {
        let tag = *i.get(p).ok_or(Error::Truncated)?;
        p += 1;
        if tag < 0x80 {
            let n = tag as usize + 1;
            if p + n > i.len() || o.len() + n > expected {
                return Err(Error::Truncated);
            }
            o.extend_from_slice(&i[p..p + n]);
            p += n
        } else {
            let l = if tag < 0xc0 {
                (tag as usize & 0x3f) + 4
            } else if tag == 0xc0 {
                get_var(i, &mut p)? as usize
            } else {
                return Err(Error::InvalidMatch);
            };
            if l < MIN_M || p + 2 > i.len() {
                return Err(Error::InvalidMatch);
            }
            let d = u16::from_le_bytes([i[p], i[p + 1]]) as usize;
            p += 2;
            if d == 0 || d > o.len() || o.len().saturating_add(l) > expected {
                return Err(Error::InvalidMatch);
            }
            let mut remaining = l;
            while remaining > 0 {
                let chunk = remaining.min(d);
                let start = o.len() - d;
                o.extend_from_within(start..start + chunk);
                remaining -= chunk;
            }
        }
    }
    if p != i.len() {
        return Err(Error::TrailingData);
    }
    Ok(o)
}
fn put_var(o: &mut Vec<u8>, mut v: u32) {
    while v >= 0x80 {
        o.push((v as u8) | 0x80);
        v >>= 7
    }
    o.push(v as u8)
}
fn get_var(i: &[u8], p: &mut usize) -> Result<u32, Error> {
    let (mut v, mut shift) = (0, 0);
    loop {
        let b = *i.get(*p).ok_or(Error::Truncated)?;
        *p += 1;
        if shift >= 32 {
            return Err(Error::InvalidMatch);
        }
        v |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            return Ok(v);
        }
        shift += 7
    }
}
fn valid(o: CompressionOptions) -> Result<(), Error> {
    if !(1..=9).contains(&o.level) {
        return Err(Error::InvalidLevel(o.level));
    }
    if !(MIN_BLOCK..=MAX_BLOCK).contains(&o.block_size) {
        return Err(Error::InvalidBlockSize(o.block_size));
    }
    Ok(())
}
fn read_block<R: Read>(r: &mut R, b: &mut [u8]) -> Result<usize, Error> {
    let mut n = 0;
    while n < b.len() {
        match r.read(&mut b[n..]).map_err(ioe)? {
            0 => break,
            x => n += x,
        }
    }
    Ok(n)
}
fn exact<R: Read>(r: &mut R, b: &mut [u8]) -> Result<(), Error> {
    r.read_exact(b).map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            Error::Truncated
        } else {
            ioe(e)
        }
    })
}
fn ioe(e: std::io::Error) -> Error {
    Error::Io(e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_mixed() {
        let d: Vec<u8> = (0..100_000)
            .map(|i| if i % 17 < 8 { (i % 251) as u8 } else { b'A' })
            .collect();
        for l in 1..=9 {
            let e = compress_slice(&d, l).unwrap();
            assert_eq!(decompress_slice(&e, d.len()).unwrap(), d)
        }
    }
    #[test]
    fn long_match_is_compact() {
        let d = vec![7; 200_000];
        let e = compress_slice(&d, 5).unwrap();
        assert!(e.len() < 200);
        assert_eq!(decompress_slice(&e, d.len()).unwrap(), d)
    }
    #[test]
    fn predictor_helps_counter() {
        let d: Vec<u8> = (0..8192).map(|i| i as u8).collect();
        let e = compress_slice(&d, 5).unwrap();
        assert_eq!(e[HEADER + 1], DELTA)
    }
}
