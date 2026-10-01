use crate::{crc32, huffman, Error, FrameInfo, Method};
use std::cell::Cell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
const MAGIC_V4: &[u8; 4] = b"XCA4";
const MAGIC_V5: &[u8; 4] = b"XCA5";
const MAGIC_V6: &[u8; 4] = b"XCA6";
const MAGIC_V7: &[u8; 4] = b"XCA7";
const MAGIC_V8: &[u8; 4] = b"XCA8";
const HEADER: usize = 12;
const BH: usize = 14;
const END: u8 = 255;
const STORED: u8 = 0;
const PULSE: u8 = 1;
const PULSE_HUFFMAN: u8 = 2;
const PULSE_SPLIT: u8 = 3;
const REFERENCE: u8 = 4;
const NONE_P: u8 = 0;
const DELTA: u8 = 1;
const XOR: u8 = 2;
const MIN_BLOCK: usize = 4096;
const MAX_BLOCK: usize = 16 * 1024 * 1024;
const DEFAULT_BLOCK: usize = 256 * 1024;
const CDC_MIN: usize = 64 * 1024;
const CDC_MAX: usize = 512 * 1024;
const CDC_MASK: u64 = (1 << 18) - 1;
const HS: usize = 1 << 16;
const MAX_D: usize = u16::MAX as usize;
const MIN_M: usize = 4;
const MAX_M: usize = u16::MAX as usize;
const UNIFIED_DEPTH: usize = 16;
const NONE_POSITION: u32 = u32::MAX;
static THREAD_LIMIT: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static CALL_THREAD_LIMIT: Cell<Option<usize>> = const { Cell::new(None) };
}

pub fn set_thread_limit(limit: usize) {
    THREAD_LIMIT.store(limit, Ordering::Relaxed);
}

fn worker_count(tasks: usize) -> usize {
    let available = thread::available_parallelism().map_or(1, |value| value.get());
    let limit = CALL_THREAD_LIMIT
        .get()
        .unwrap_or_else(|| THREAD_LIMIT.load(Ordering::Relaxed));
    let requested = if limit == 0 {
        available
    } else {
        available.min(limit)
    };
    requested.min(tasks.max(1))
}

pub(crate) fn with_thread_limit<T>(limit: usize, operation: impl FnOnce() -> T) -> T {
    let previous = CALL_THREAD_LIMIT.replace(Some(limit));
    struct Reset(Option<usize>);
    impl Drop for Reset {
        fn drop(&mut self) {
            CALL_THREAD_LIMIT.set(self.0);
        }
    }
    let _reset = Reset(previous);
    operation()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressionOptions {
    pub block_size: usize,
}
impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArchiveAnalysis {
    pub original_bytes: u64,
    pub archive_bytes: u64,
    pub blocks: u32,
    pub stored_blocks: u32,
    pub pulse_blocks: u32,
    pub split_pulse_blocks: u32,
    pub entropy_blocks: u32,
    pub reference_blocks: u32,
    pub referenced_bytes: u64,
    pub predictor_none_blocks: u32,
    pub predictor_delta_blocks: u32,
    pub predictor_xor_blocks: u32,
    pub literal_commands: u64,
    pub literal_bytes: u64,
    pub short_match_commands: u64,
    pub long_match_commands: u64,
    pub matched_bytes: u64,
    pub distance_total: u64,
}
#[derive(Default)]
struct DistanceCache {
    values: [usize; 4],
    len: usize,
}
impl DistanceCache {
    fn promote(&mut self, distance: usize) {
        let existing = self.values[..self.len]
            .iter()
            .position(|&value| value == distance);
        let stop = existing.unwrap_or(self.len.min(3));
        for index in (1..=stop).rev() {
            self.values[index] = self.values[index - 1];
        }
        self.values[0] = distance;
        if existing.is_none() {
            self.len = (self.len + 1).min(4);
        }
    }

    fn encode(&mut self, output: &mut Vec<u8>, distance: usize) {
        if let Some(index) = self.values[..self.len]
            .iter()
            .position(|&value| value == distance)
        {
            output.push(index as u8);
            self.promote(distance);
        } else {
            put_var(output, (distance as u32) + 4);
            self.promote(distance);
        }
    }

    fn decode(&mut self, input: &[u8], position: &mut usize) -> Result<usize, Error> {
        let first = *input.get(*position).ok_or(Error::Truncated)?;
        *position += 1;
        let distance = if first <= 3 {
            let index = first as usize;
            if index >= self.len {
                return Err(Error::InvalidMatch);
            }
            self.values[index]
        } else {
            let encoded = get_var_first(input, position, first)?;
            if encoded < 5 {
                return Err(Error::InvalidMatch);
            }
            encoded as usize - 4
        };
        if distance == 0 || distance > MAX_D {
            return Err(Error::InvalidMatch);
        }
        self.promote(distance);
        Ok(distance)
    }
}
pub fn compress_slice(input: &[u8]) -> Result<Vec<u8>, Error> {
    let options = CompressionOptions::default();
    valid(options)?;
    let (ranges, references) = plan_blocks(input, options.block_size);
    let count = ranges.len();
    let encoded_block_size = ranges
        .iter()
        .map(|(start, end)| end - start)
        .max()
        .unwrap_or(options.block_size)
        .max(MIN_BLOCK);
    let workers = worker_count(count);
    let mut blocks = if workers == 1 {
        let mut workspace = MatchWorkspace::new();
        ranges
            .iter()
            .enumerate()
            .map(|(index, &(start, end))| match references[index] {
                Some(target) => reference_block(end - start, target),
                None => encode_block(&input[start..end], &mut workspace),
            })
            .collect::<Vec<_>>()
    } else {
        thread::scope(|scope| {
            let mut handles = Vec::new();
            let ranges = &ranges;
            let references = &references;
            for worker in 0..workers {
                handles.push(scope.spawn(move || {
                    let mut local = Vec::new();
                    let mut workspace = MatchWorkspace::new();
                    for index in (worker..count).step_by(workers) {
                        let (start, end) = ranges[index];
                        let block = match references[index] {
                            Some(target) => reference_block(end - start, target),
                            None => encode_block(&input[start..end], &mut workspace),
                        };
                        local.push((index, block));
                    }
                    local
                }));
            }
            let mut ordered: Vec<Option<EncodedBlock>> = (0..count).map(|_| None).collect();
            for handle in handles {
                for (index, block) in handle.join().map_err(|_| Error::WorkerPanicked)? {
                    ordered[index] = Some(block);
                }
            }
            Ok::<_, Error>(ordered.into_iter().map(Option::unwrap).collect::<Vec<_>>())
        })?
    };
    for index in 0..blocks.len() {
        if blocks[index].method == REFERENCE {
            let target =
                u32::from_le_bytes(blocks[index].payload[..4].try_into().unwrap()) as usize;
            blocks[index].checksum = blocks[target].checksum;
        }
    }
    let capacity = HEADER + BH + blocks.iter().map(|b| BH + b.payload.len()).sum::<usize>();
    let has_references = blocks.iter().any(|block| block.method == REFERENCE);
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(MAGIC_V8);
    output.extend_from_slice(&[0, 1 | u8::from(has_references) << 1, 0, 0]);
    output.extend_from_slice(&(encoded_block_size as u32).to_le_bytes());
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
        PULSE => decode(block.payload, block.original, false)?,
        PULSE_HUFFMAN => {
            let pulse = huffman::decode_limited(block.payload, pulse_limit(block.original))?;
            decode(&pulse, block.original, false)?
        }
        PULSE_SPLIT => split_decode(block.payload, block.original)?,
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
    decode_frame(input, limit, None, true)?.ok_or(Error::InvalidMatch)
}

pub fn decompress_into_slice(
    input: &[u8],
    output: &mut [u8],
    limit: usize,
) -> Result<usize, Error> {
    decode_frame(input, limit, Some(output), true)?;
    Ok(output.len())
}

pub fn verify_slice(input: &[u8], limit: usize) -> Result<usize, Error> {
    let total = frame_info(input)?.original_size;
    decode_frame(input, limit, None, false)?;
    Ok(total)
}

fn parse_frame(
    input: &[u8],
    limit: usize,
) -> Result<(Vec<BlockDescriptor<'_>>, Vec<usize>, usize), Error> {
    if input.len() < HEADER
        || &input[..4] != MAGIC_V4
            && &input[..4] != MAGIC_V5
            && &input[..4] != MAGIC_V6
            && &input[..4] != MAGIC_V7
            && &input[..4] != MAGIC_V8
    {
        return Err(Error::BadMagic);
    }
    if !valid_profile(&input[..4], input[4]) {
        return Err(Error::InvalidLevel(input[4]));
    }
    if !valid_flags(input[5..8].try_into().unwrap()) {
        return Err(Error::InvalidEntropyData);
    }
    let block_size = u32::from_le_bytes(input[8..12].try_into().unwrap()) as usize;
    if !(MIN_BLOCK..=MAX_BLOCK).contains(&block_size) {
        return Err(Error::InvalidBlockSize(block_size));
    }

    let mut cursor = HEADER;
    let mut total = 0usize;
    let mut descriptors = Vec::new();
    let mut offsets = vec![0usize];
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
        if !matches!(
            method,
            STORED | PULSE | PULSE_HUFFMAN | PULSE_SPLIT | REFERENCE
        ) {
            return Err(Error::UnsupportedMethod(method));
        }
        if predictor > XOR {
            return Err(Error::UnsupportedMethod(predictor));
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
        if method == REFERENCE && input[5] & 2 == 0 {
            return Err(Error::InvalidMatch);
        }
        descriptors.push(BlockDescriptor {
            method,
            predictor,
            original,
            payload: &input[cursor..end],
            checksum,
        });
        offsets.push(total);
        cursor = end;
    }
    Ok((descriptors, offsets, total))
}

fn reference_target(descriptors: &[BlockDescriptor<'_>], index: usize) -> Result<usize, Error> {
    let descriptor = descriptors[index];
    if descriptor.predictor != NONE_P || descriptor.payload.len() != 4 {
        return Err(Error::InvalidMatch);
    }
    let target = u32::from_le_bytes(descriptor.payload.try_into().unwrap()) as usize;
    if target >= index {
        return Err(Error::InvalidMatch);
    }
    if descriptors[target].original != descriptor.original {
        return Err(Error::LengthMismatch {
            expected: descriptor.original,
            actual: descriptors[target].original,
        });
    }
    if descriptors[target].checksum != descriptor.checksum {
        return Err(Error::ChecksumMismatch {
            expected: descriptor.checksum,
            actual: descriptors[target].checksum,
        });
    }
    Ok(target)
}

fn copy_reference(output: &mut [u8], offsets: &[usize], index: usize, target: usize) {
    let destination_start = offsets[index];
    let destination_end = offsets[index + 1];
    let source_start = offsets[target];
    let source_end = offsets[target + 1];
    let (before, destination) = output.split_at_mut(destination_start);
    destination[..destination_end - destination_start]
        .copy_from_slice(&before[source_start..source_end]);
}

fn decode_direct(
    descriptors: &[BlockDescriptor<'_>],
    offsets: &[usize],
    output: &mut [u8],
) -> Result<(), Error> {
    let workers = worker_count(descriptors.len());
    let mut index = 0usize;
    while index < descriptors.len() {
        if descriptors[index].method == REFERENCE {
            let target = reference_target(descriptors, index)?;
            copy_reference(output, offsets, index, target);
            index += 1;
            continue;
        }

        let mut end = index + 1;
        while end < descriptors.len()
            && descriptors[end].method != REFERENCE
            && end - index < workers
        {
            end += 1;
        }
        let decoded = if end - index == 1 {
            vec![decode_block(descriptors[index])?]
        } else {
            thread::scope(|scope| {
                let mut handles = Vec::with_capacity(end - index);
                for descriptor in &descriptors[index..end] {
                    handles.push(scope.spawn(move || decode_block(*descriptor)));
                }
                let mut blocks = Vec::with_capacity(handles.len());
                for handle in handles {
                    blocks.push(handle.join().map_err(|_| Error::WorkerPanicked)??);
                }
                Ok::<_, Error>(blocks)
            })?
        };
        for (block_index, block) in (index..end).zip(decoded) {
            output[offsets[block_index]..offsets[block_index + 1]].copy_from_slice(&block);
        }
        index = end;
    }
    Ok(())
}

fn verify_blocks(descriptors: &[BlockDescriptor<'_>]) -> Result<(), Error> {
    for (index, descriptor) in descriptors.iter().copied().enumerate() {
        if descriptor.method == REFERENCE {
            reference_target(descriptors, index)?;
        } else {
            decode_block(descriptor)?;
        }
    }
    Ok(())
}

fn decode_frame(
    input: &[u8],
    limit: usize,
    destination: Option<&mut [u8]>,
    assemble: bool,
) -> Result<Option<Vec<u8>>, Error> {
    let (descriptors, offsets, total) = parse_frame(input, limit)?;
    if let Some(output) = destination.as_ref() {
        if output.len() != total {
            return Err(Error::LengthMismatch {
                expected: total,
                actual: output.len(),
            });
        }
    }
    if !assemble {
        verify_blocks(&descriptors)?;
        return Ok(None);
    }
    if let Some(output) = destination {
        decode_direct(&descriptors, &offsets, output)?;
        return Ok(None);
    }
    let mut output = vec![0u8; total];
    decode_direct(&descriptors, &offsets, &mut output)?;
    Ok(Some(output))
}

fn plan_blocks(input: &[u8], block_size: usize) -> (Vec<(usize, usize)>, Vec<Option<usize>>) {
    let fixed = fixed_ranges(input.len(), block_size);
    if input.len() < CDC_MIN * 2 {
        return (fixed.clone(), vec![None; fixed.len()]);
    }

    let candidate = if let Some(period) = repeated_period(input) {
        periodic_ranges(input.len(), period, block_size)
    } else {
        content_defined_ranges(input)
    };
    let (candidate_references, candidate_bytes) = find_references(input, &candidate);
    if candidate_bytes >= input.len() / 100 {
        (candidate, candidate_references)
    } else {
        let (fixed_references, _) = find_references(input, &fixed);
        (fixed, fixed_references)
    }
}

fn fixed_ranges(length: usize, block_size: usize) -> Vec<(usize, usize)> {
    (0..length.div_ceil(block_size))
        .map(|index| {
            let start = index * block_size;
            (start, (start + block_size).min(length))
        })
        .collect()
}

fn repeated_period(input: &[u8]) -> Option<usize> {
    (2..=64).rev().find_map(|copies| {
        if input.len().is_multiple_of(copies) {
            let period = input.len() / copies;
            (period >= CDC_MIN && input[period..] == input[..input.len() - period])
                .then_some(period)
        } else {
            None
        }
    })
}

fn periodic_ranges(length: usize, period: usize, block_size: usize) -> Vec<(usize, usize)> {
    let base = fixed_ranges(period, block_size);
    let copies = length / period;
    let mut ranges = Vec::with_capacity(base.len() * copies);
    for copy in 0..copies {
        let offset = copy * period;
        ranges.extend(
            base.iter()
                .map(|&(start, end)| (start + offset, end + offset)),
        );
    }
    ranges
}

fn content_defined_ranges(input: &[u8]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut hash = 0u64;
    for (position, &byte) in input.iter().enumerate() {
        hash = hash
            .rotate_left(1)
            .wrapping_add(GEAR_TABLE[byte as usize])
            .wrapping_mul(0x9e37_79b1);
        let length = position + 1 - start;
        if length >= CDC_MIN && (hash & CDC_MASK == 0 || length >= CDC_MAX) {
            ranges.push((start, position + 1));
            start = position + 1;
            hash = 0;
        }
    }
    if start < input.len() {
        ranges.push((start, input.len()));
    }
    ranges
}

const fn gear_value(byte: u8) -> u64 {
    let mut value = byte as u64 + 0x9e37_79b9_7f4a_7c15;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
const fn gear_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut index = 0;
    while index < table.len() {
        table[index] = gear_value(index as u8);
        index += 1;
    }
    table
}
const GEAR_TABLE: [u64; 256] = gear_table();

fn find_references(input: &[u8], ranges: &[(usize, usize)]) -> (Vec<Option<usize>>, usize) {
    let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
    let mut references = vec![None; ranges.len()];
    let mut referenced_bytes = 0usize;
    for (index, &(start, end)) in ranges.iter().enumerate() {
        let data = &input[start..end];
        let fingerprint = block_fingerprint(data);
        let target = buckets.get(&fingerprint).and_then(|candidates| {
            candidates.iter().copied().find(|&candidate| {
                let (other_start, other_end) = ranges[candidate];
                input[other_start..other_end] == *data
            })
        });
        if let Some(target) = target {
            references[index] = Some(target);
            referenced_bytes += data.len();
        } else {
            buckets.entry(fingerprint).or_default().push(index);
        }
    }
    (references, referenced_bytes)
}

fn block_fingerprint(data: &[u8]) -> u64 {
    let mut hash = 0x6a09_e667_f3bc_c909u64 ^ data.len() as u64;
    let (chunks, remainder) = data.as_chunks::<8>();
    for chunk in chunks {
        let value = u64::from_le_bytes(*chunk);
        hash ^= value.wrapping_mul(0x9e37_79b1_85eb_ca87);
        hash = hash.rotate_left(27).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    }
    for &byte in remainder {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash ^ (hash >> 29)
}

struct EncodedBlock {
    method: u8,
    predictor: u8,
    original: u32,
    payload: Vec<u8>,
    checksum: u32,
}
fn reference_block(original: usize, target: usize) -> EncodedBlock {
    EncodedBlock {
        method: REFERENCE,
        predictor: NONE_P,
        original: original as u32,
        payload: (target as u32).to_le_bytes().to_vec(),
        checksum: 0,
    }
}
fn encode_block(input: &[u8], workspace: &mut MatchWorkspace) -> EncodedBlock {
    let predictor = choose(input);
    let packed = if predictor == NONE_P {
        encode(input, workspace)
    } else {
        let transformed = transform(input, predictor);
        encode(&transformed, workspace)
    };
    let split = split_encode(&packed, input.len());
    let (method, payload) = match split {
        Some(encoded) if encoded.len() < packed.len() => (PULSE_SPLIT, encoded),
        _ => match huffman::encode(&packed) {
            Some(encoded) if encoded.len() < packed.len() => (PULSE_HUFFMAN, encoded),
            _ => (PULSE, packed),
        },
    };
    if payload.len() < input.len() {
        EncodedBlock {
            method,
            predictor,
            original: input.len() as u32,
            payload,
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
    w.write_all(MAGIC_V8).map_err(ioe)?;
    w.write_all(&[0, 1, 0, 0]).map_err(ioe)?;
    w.write_all(&(opt.block_size as u32).to_le_bytes())
        .map_err(ioe)?;
    let mut st = StreamStats {
        output_bytes: HEADER as u64,
        ..Default::default()
    };
    let mut b = vec![0; opt.block_size];
    let mut workspace = MatchWorkspace::new();
    loop {
        let n = read_block(r, &mut b)?;
        if n == 0 {
            break;
        }
        let input = &b[..n];
        let pred = choose(input);
        let packed = if pred == NONE_P {
            encode(input, &mut workspace)
        } else {
            let transformed = transform(input, pred);
            encode(&transformed, &mut workspace)
        };
        let split = split_encode(&packed, input.len());
        let (m, owned) = match split {
            Some(encoded) if encoded.len() < packed.len() => (PULSE_SPLIT, encoded),
            _ => match huffman::encode(&packed) {
                Some(encoded) if encoded.len() < packed.len() => (PULSE_HUFFMAN, encoded),
                _ => (PULSE, packed),
            },
        };
        let sp = pred;
        let (m, p, sp) = if owned.len() < input.len() {
            (m, owned.as_slice(), sp)
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
    if &h[..4] != MAGIC_V4
        && &h[..4] != MAGIC_V5
        && &h[..4] != MAGIC_V6
        && &h[..4] != MAGIC_V7
        && &h[..4] != MAGIC_V8
    {
        return Err(Error::BadMagic);
    }
    if !valid_profile(&h[..4], h[4]) {
        return Err(Error::InvalidLevel(h[4]));
    }
    if !valid_flags(h[5..8].try_into().unwrap()) {
        return Err(Error::InvalidEntropyData);
    }
    if h[5] & 2 != 0 {
        return Err(Error::StreamingReferencesUnsupported);
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
            PULSE => decode(&p, n, false)?,
            PULSE_HUFFMAN => {
                let pulse = huffman::decode_limited(&p, pulse_limit(n))?;
                decode(&pulse, n, false)?
            }
            PULSE_SPLIT => split_decode(&p, n)?,
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
    let mut trailing = [0u8; 1];
    if r.read(&mut trailing).map_err(ioe)? != 0 {
        return Err(Error::TrailingData);
    }
    Ok(st)
}
pub fn analyze_archive(input: &[u8]) -> Result<ArchiveAnalysis, Error> {
    // Keep diagnostics structurally identical to normal decoding. The second
    // pass below gathers token statistics without reconstructing output.
    parse_frame(input, usize::MAX)?;
    if input.len() < HEADER
        || &input[..4] != MAGIC_V4
            && &input[..4] != MAGIC_V5
            && &input[..4] != MAGIC_V6
            && &input[..4] != MAGIC_V7
            && &input[..4] != MAGIC_V8
    {
        return Err(Error::BadMagic);
    }
    if !valid_profile(&input[..4], input[4]) {
        return Err(Error::InvalidLevel(input[4]));
    }
    if !valid_flags(input[5..8].try_into().unwrap()) {
        return Err(Error::InvalidEntropyData);
    }
    let mut result = ArchiveAnalysis {
        archive_bytes: input.len() as u64,
        ..ArchiveAnalysis::default()
    };
    let mut cursor = HEADER;
    let mut originals = Vec::<usize>::new();
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
        if method == END {
            if original != 0 || packed != 0 || cursor != input.len() {
                return Err(Error::TrailingData);
            }
            break;
        }
        let end = cursor.checked_add(packed).ok_or(Error::InputTooLarge)?;
        if end > input.len() {
            return Err(Error::Truncated);
        }
        result.blocks += 1;
        result.original_bytes += original as u64;
        match predictor {
            NONE_P => result.predictor_none_blocks += 1,
            DELTA => result.predictor_delta_blocks += 1,
            XOR => result.predictor_xor_blocks += 1,
            value => return Err(Error::UnsupportedMethod(value)),
        }
        if method == REFERENCE {
            if input[5] & 2 == 0 || predictor != NONE_P || packed != 4 {
                return Err(Error::InvalidMatch);
            }
            let target = u32::from_le_bytes(input[cursor..end].try_into().unwrap()) as usize;
            if target >= originals.len() || originals[target] != original {
                return Err(Error::InvalidMatch);
            }
            result.reference_blocks += 1;
            result.referenced_bytes += original as u64;
        } else if method == STORED {
            result.stored_blocks += 1;
            result.literal_commands += 1;
            result.literal_bytes += original as u64;
        } else if matches!(method, PULSE | PULSE_HUFFMAN | PULSE_SPLIT) {
            result.pulse_blocks += 1;
            if method == PULSE_SPLIT {
                result.split_pulse_blocks += 1;
            }
            if matches!(method, PULSE_HUFFMAN | PULSE_SPLIT) {
                result.entropy_blocks += 1;
            }
            let decoded_entropy;
            let decoded_split;
            let payload = if method == PULSE_HUFFMAN {
                decoded_entropy =
                    huffman::decode_limited(&input[cursor..end], pulse_limit(original))?;
                decoded_entropy.as_slice()
            } else if method == PULSE_SPLIT {
                decoded_split = split_expand(&input[cursor..end], original)?;
                decoded_split.as_slice()
            } else {
                &input[cursor..end]
            };
            let mut at = 0;
            let mut produced = 0usize;
            while produced < original {
                let tag = *payload.get(at).ok_or(Error::Truncated)?;
                at += 1;
                if tag < 0x80 {
                    let length = tag as usize + 1;
                    if at + length > payload.len() || produced + length > original {
                        return Err(Error::Truncated);
                    }
                    result.literal_commands += 1;
                    result.literal_bytes += length as u64;
                    at += length;
                    produced += length;
                } else {
                    let length = if tag < 0xc0 {
                        result.short_match_commands += 1;
                        (tag as usize & 0x3f) + 4
                    } else if tag == 0xc0 {
                        result.long_match_commands += 1;
                        get_var(payload, &mut at)? as usize
                    } else {
                        return Err(Error::InvalidMatch);
                    };
                    if produced + length > original {
                        return Err(Error::InvalidMatch);
                    }
                    if at + 2 > payload.len() {
                        return Err(Error::InvalidMatch);
                    }
                    let distance = u16::from_le_bytes([payload[at], payload[at + 1]]) as usize;
                    at += 2;
                    if distance == 0 || distance > produced {
                        return Err(Error::InvalidMatch);
                    }
                    result.matched_bytes += length as u64;
                    result.distance_total += distance as u64;
                    produced += length;
                }
            }
            if at != payload.len() {
                return Err(Error::TrailingData);
            }
        } else {
            return Err(Error::UnsupportedMethod(method));
        }
        originals.push(original);
        cursor = end;
    }
    Ok(result)
}
pub fn frame_info(input: &[u8]) -> Result<FrameInfo, Error> {
    let (descriptors, _, original_size) = parse_frame(input, usize::MAX)?;
    for (index, descriptor) in descriptors.iter().enumerate() {
        if descriptor.method == REFERENCE {
            reference_target(&descriptors, index)?;
        }
    }
    let magic = &input[..4];
    Ok(FrameInfo {
        version: if magic == MAGIC_V8 {
            8
        } else if magic == MAGIC_V7 {
            7
        } else if magic == MAGIC_V6 {
            6
        } else if magic == MAGIC_V5 {
            5
        } else {
            4
        },
        method: Method::Adaptive,
        legacy_level: (magic != MAGIC_V8).then_some(input[4]),
        original_size,
        frame_size: input.len(),
        checksum: true,
        blocks: u32::try_from(descriptors.len()).map_err(|_| Error::InputTooLarge)?,
    })
}

fn choose(i: &[u8]) -> u8 {
    let n = i.len().min(8192);
    if n < 64 {
        return NONE_P;
    }
    let s = &i[..n];
    let mut table = [u16::MAX; 4096];
    let raw = score(s, NONE_P, &mut table);
    let d = score(s, DELTA, &mut table);
    let x = score(s, XOR, &mut table);
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
fn score(input: &[u8], predictor: u8, table: &mut [u16; 4096]) -> usize {
    if input.len() < 3 {
        return 0;
    }
    table.fill(u16::MAX);
    let mut result = 0;
    for position in 0..input.len() - 2 {
        let a = predicted(input, position, predictor);
        let b = predicted(input, position + 1, predictor);
        let c = predicted(input, position + 2, predictor);
        let hash =
            ((a as usize).wrapping_mul(251) ^ (b as usize).wrapping_mul(31) ^ c as usize) & 4095;
        let previous = table[hash];
        if previous != u16::MAX
            && predicted(input, previous as usize, predictor) == a
            && predicted(input, previous as usize + 1, predictor) == b
            && predicted(input, previous as usize + 2, predictor) == c
        {
            result += 1;
        }
        table[hash] = position as u16;
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
struct MatchWorkspace {
    heads: Vec<u32>,
    previous: Vec<u32>,
}
impl MatchWorkspace {
    fn new() -> Self {
        Self {
            heads: vec![NONE_POSITION; HS],
            previous: Vec::new(),
        }
    }

    fn reset(&mut self, input_len: usize) {
        debug_assert!(input_len <= u32::MAX as usize);
        self.heads.fill(NONE_POSITION);
        self.previous.resize(input_len, NONE_POSITION);
    }
}
fn insert(i: &[u8], p: usize, h: &mut [u32], prev: &mut [u32]) {
    if p + 3 > i.len() {
        return;
    }
    let x = hash(i, p);
    prev[p] = h[x];
    h[x] = p as u32;
}
fn match_length(i: &[u8], left: usize, right: usize, max: usize) -> usize {
    let mut length = 0;
    while length + 8 <= max {
        let a = u64::from_ne_bytes(i[left + length..left + length + 8].try_into().unwrap());
        let b = u64::from_ne_bytes(i[right + length..right + length + 8].try_into().unwrap());
        if a != b {
            break;
        }
        length += 8;
    }
    while length < max && i[left + length] == i[right + length] {
        length += 1;
    }
    length
}
fn find(i: &[u8], p: usize, h: &[u32], prev: &[u32], md: usize) -> (usize, usize) {
    if p + MIN_M > i.len() {
        return (0, 0);
    }
    let mut c = h[hash(i, p)];
    let (mut bd, mut bl, mut tries) = (0, 0, 0);
    while c != NONE_POSITION && tries < md {
        let candidate = c as usize;
        let d = p - candidate;
        if d > MAX_D {
            break;
        }
        let max = MAX_M.min(i.len() - p);
        if bl < max && i[candidate + bl] != i[p + bl] {
            c = prev[candidate];
            tries += 1;
            continue;
        }
        let length = match_length(i, candidate, p, max);
        if length > bl && length >= MIN_M {
            bl = length;
            bd = d;
            if length == max {
                break;
            }
        }
        c = prev[candidate];
        tries += 1;
    }
    (bd, bl)
}
fn encode(i: &[u8], workspace: &mut MatchWorkspace) -> Vec<u8> {
    let mut o = Vec::with_capacity(i.len());
    workspace.reset(i.len());
    let h = &mut workspace.heads;
    let prev = &mut workspace.previous;
    let (mut p, mut lit) = (0, 0);
    while p < i.len() {
        let (d, l) = find(i, p, h, prev, UNIFIED_DEPTH);
        if l >= MIN_M {
            literals(&mut o, &i[lit..p]);
            match_token(&mut o, d, l);
            let mut x = p;
            while x < p + l {
                insert(i, x, h, prev);
                x += 1;
            }
            p += l;
            lit = p;
        } else {
            insert(i, p, h, prev);
            p += 1;
            if p - lit == 128 {
                literals(&mut o, &i[lit..p]);
                lit = p;
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
fn decode(i: &[u8], expected: usize, _pulse_v2: bool) -> Result<Vec<u8>, Error> {
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
            if l < MIN_M {
                return Err(Error::InvalidMatch);
            }
            if p + 2 > i.len() {
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
fn split_encode(pulse: &[u8], expected: usize) -> Option<Vec<u8>> {
    let mut tags = Vec::new();
    let mut literals_stream = Vec::new();
    let mut lengths = Vec::new();
    let mut distances_stream = Vec::new();
    let mut cache = DistanceCache::default();
    let mut position = 0usize;
    let mut produced = 0usize;
    while produced < expected {
        let tag = *pulse.get(position)?;
        position += 1;
        tags.push(tag);
        if tag < 0x80 {
            let length = tag as usize + 1;
            let end = position.checked_add(length)?;
            if end > pulse.len() || produced.checked_add(length)? > expected {
                return None;
            }
            literals_stream.extend_from_slice(&pulse[position..end]);
            position = end;
            produced += length;
        } else {
            let length = if tag < 0xc0 {
                (tag as usize & 0x3f) + 4
            } else if tag == 0xc0 {
                let start = position;
                let value = get_var(pulse, &mut position).ok()? as usize;
                lengths.extend_from_slice(&pulse[start..position]);
                value
            } else {
                return None;
            };
            if length < MIN_M
                || produced.checked_add(length)? > expected
                || position + 2 > pulse.len()
            {
                return None;
            }
            let distance = u16::from_le_bytes([pulse[position], pulse[position + 1]]) as usize;
            position += 2;
            if distance == 0 || distance > produced {
                return None;
            }
            cache.encode(&mut distances_stream, distance);
            produced += length;
        }
    }
    if position != pulse.len() {
        return None;
    }
    let mut output = Vec::new();
    output.extend_from_slice(b"SPL1");
    append_split_stream(&mut output, &tags);
    append_split_stream(&mut output, &literals_stream);
    append_split_stream(&mut output, &lengths);
    append_split_stream(&mut output, &distances_stream);
    Some(output)
}
fn append_split_stream(output: &mut Vec<u8>, stream: &[u8]) {
    if let Some(data) = huffman::encode(stream) {
        if data.len() < stream.len() {
            output.push(1);
            output.extend_from_slice(&(data.len() as u32).to_le_bytes());
            output.extend_from_slice(&data);
            return;
        }
    }
    output.push(0);
    output.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    output.extend_from_slice(stream);
}
fn split_expand(input: &[u8], expected: usize) -> Result<Vec<u8>, Error> {
    if input.len() < 4 || &input[..4] != b"SPL1" {
        return Err(Error::InvalidEntropyData);
    }
    let mut position = 4usize;
    let tags = read_split_stream(input, &mut position, expected)?;
    let literals_stream = read_split_stream(input, &mut position, expected)?;
    let lengths = read_split_stream(input, &mut position, expected)?;
    let distances_stream = read_split_stream(input, &mut position, expected)?;
    if position != input.len() {
        return Err(Error::TrailingData);
    }

    let mut pulse = Vec::new();
    let mut tag_position = 0usize;
    let mut literal_position = 0usize;
    let mut length_position = 0usize;
    let mut distance_position = 0usize;
    let mut produced = 0usize;
    let mut cache = DistanceCache::default();
    while produced < expected {
        let tag = *tags.get(tag_position).ok_or(Error::Truncated)?;
        tag_position += 1;
        pulse.push(tag);
        if tag < 0x80 {
            let length = tag as usize + 1;
            let end = literal_position
                .checked_add(length)
                .ok_or(Error::InputTooLarge)?;
            if end > literals_stream.len() || produced.saturating_add(length) > expected {
                return Err(Error::Truncated);
            }
            pulse.extend_from_slice(&literals_stream[literal_position..end]);
            literal_position = end;
            produced += length;
        } else {
            let length = if tag < 0xc0 {
                (tag as usize & 0x3f) + 4
            } else if tag == 0xc0 {
                let start = length_position;
                let value = get_var(&lengths, &mut length_position)? as usize;
                pulse.extend_from_slice(&lengths[start..length_position]);
                value
            } else {
                return Err(Error::InvalidMatch);
            };
            let distance = cache.decode(&distances_stream, &mut distance_position)?;
            pulse.extend_from_slice(&(distance as u16).to_le_bytes());
            if length < MIN_M || distance > produced || produced.saturating_add(length) > expected {
                return Err(Error::InvalidMatch);
            }
            produced += length;
        }
    }
    if tag_position != tags.len()
        || literal_position != literals_stream.len()
        || length_position != lengths.len()
        || distance_position != distances_stream.len()
    {
        return Err(Error::TrailingData);
    }
    Ok(pulse)
}
fn split_decode(input: &[u8], expected: usize) -> Result<Vec<u8>, Error> {
    if input.len() < 4 || &input[..4] != b"SPL1" {
        return Err(Error::InvalidEntropyData);
    }
    let mut position = 4usize;
    let tags = read_split_stream(input, &mut position, expected)?;
    let literals_stream = read_split_stream(input, &mut position, expected)?;
    let lengths = read_split_stream(input, &mut position, expected)?;
    let distances_stream = read_split_stream(input, &mut position, expected)?;
    if position != input.len() {
        return Err(Error::TrailingData);
    }

    let mut output = Vec::with_capacity(expected);
    let mut tag_position = 0usize;
    let mut literal_position = 0usize;
    let mut length_position = 0usize;
    let mut distance_position = 0usize;
    let mut cache = DistanceCache::default();
    while output.len() < expected {
        let tag = *tags.get(tag_position).ok_or(Error::Truncated)?;
        tag_position += 1;
        if tag < 0x80 {
            let length = tag as usize + 1;
            let end = literal_position
                .checked_add(length)
                .ok_or(Error::InputTooLarge)?;
            if end > literals_stream.len() || output.len().saturating_add(length) > expected {
                return Err(Error::Truncated);
            }
            output.extend_from_slice(&literals_stream[literal_position..end]);
            literal_position = end;
        } else {
            let length = if tag < 0xc0 {
                (tag as usize & 0x3f) + 4
            } else if tag == 0xc0 {
                get_var(&lengths, &mut length_position)? as usize
            } else {
                return Err(Error::InvalidMatch);
            };
            let distance = cache.decode(&distances_stream, &mut distance_position)?;
            if length < MIN_M
                || distance > output.len()
                || output.len().saturating_add(length) > expected
            {
                return Err(Error::InvalidMatch);
            }
            let mut remaining = length;
            while remaining > 0 {
                let chunk = remaining.min(distance);
                let start = output.len() - distance;
                output.extend_from_within(start..start + chunk);
                remaining -= chunk;
            }
        }
    }
    if tag_position != tags.len()
        || literal_position != literals_stream.len()
        || length_position != lengths.len()
        || distance_position != distances_stream.len()
    {
        return Err(Error::TrailingData);
    }
    Ok(output)
}
fn read_split_stream(input: &[u8], position: &mut usize, limit: usize) -> Result<Vec<u8>, Error> {
    let method = *input.get(*position).ok_or(Error::Truncated)?;
    *position += 1;
    let end_header = position.checked_add(4).ok_or(Error::InputTooLarge)?;
    let size_bytes = input.get(*position..end_header).ok_or(Error::Truncated)?;
    let size = u32::from_le_bytes(size_bytes.try_into().unwrap()) as usize;
    *position = end_header;
    let end = position.checked_add(size).ok_or(Error::InputTooLarge)?;
    let payload = input.get(*position..end).ok_or(Error::Truncated)?;
    *position = end;
    match method {
        0 => Ok(payload.to_vec()),
        1 => huffman::decode_limited(payload, limit),
        _ => Err(Error::InvalidEntropyData),
    }
}
fn pulse_limit(expected: usize) -> usize {
    expected
        .saturating_add(expected.div_ceil(128))
        .saturating_add(1024)
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
fn get_var_first(i: &[u8], p: &mut usize, first: u8) -> Result<u32, Error> {
    let mut value = (first & 0x7f) as u32;
    if first & 0x80 == 0 {
        return Ok(value);
    }
    let mut shift = 7;
    loop {
        let byte = *i.get(*p).ok_or(Error::Truncated)?;
        *p += 1;
        if shift >= 32 {
            return Err(Error::InvalidMatch);
        }
        value |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
    }
}
fn valid(o: CompressionOptions) -> Result<(), Error> {
    if !(MIN_BLOCK..=MAX_BLOCK).contains(&o.block_size) {
        return Err(Error::InvalidBlockSize(o.block_size));
    }
    Ok(())
}
fn valid_profile(magic: &[u8], profile: u8) -> bool {
    if magic == MAGIC_V8 {
        profile == 0
    } else {
        (1..=9).contains(&profile)
    }
}
fn valid_flags(flags: [u8; 3]) -> bool {
    flags[0] & 1 == 1 && flags[0] & !3 == 0 && flags[1] == 0 && flags[2] == 0
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
        let e = compress_slice(&d).unwrap();
        assert_eq!(decompress_slice(&e, d.len()).unwrap(), d)
    }
    #[test]
    fn long_match_is_compact() {
        let d = vec![7; 200_000];
        let e = compress_slice(&d).unwrap();
        assert!(e.len() < 200);
        assert_eq!(decompress_slice(&e, d.len()).unwrap(), d)
    }
    #[test]
    fn predictor_helps_counter() {
        let d: Vec<u8> = (0..8192).map(|i| i as u8).collect();
        let e = compress_slice(&d).unwrap();
        assert_eq!(e[HEADER + 1], DELTA)
    }

    #[test]
    fn strict_deterministic_round_trip_matrix() {
        let sizes = [
            0usize, 1, 3, 4, 31, 127, 128, 129, 4095, 4096, 65_535, 65_536,
        ];
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        for size in sizes {
            let mut data = vec![0u8; size];
            for byte in &mut data {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = (state >> 24) as u8;
            }
            let first = compress_slice(&data).unwrap();
            let second = compress_slice(&data).unwrap();
            assert_eq!(first, second, "non-deterministic output for {size} bytes");
            assert_eq!(
                decompress_slice(&first, data.len()).unwrap(),
                data,
                "round-trip failure for {size} bytes"
            );
        }
    }

    #[test]
    fn every_truncated_prefix_is_rejected() {
        let data = b"strict truncation vector ".repeat(2048);
        let archive = compress_slice(&data).unwrap();
        for end in 0..archive.len() {
            assert!(
                decompress_slice(&archive[..end], data.len()).is_err(),
                "accepted truncated prefix ending at {end}"
            );
        }
    }

    #[test]
    fn payload_corruption_is_rejected() {
        let data: Vec<u8> = (0..200_000)
            .map(|index| ((index * 17 + index / 97) & 0xff) as u8)
            .collect();
        let archive = compress_slice(&data).unwrap();
        let payload_start = HEADER + BH;
        let payload_end = archive.len() - BH;
        for position in (payload_start..payload_end.saturating_sub(1)).step_by(31) {
            let mut damaged = archive.clone();
            damaged[position] ^= 0x80;
            assert!(
                decompress_slice(&damaged, data.len()).is_err(),
                "accepted corruption at byte {position}"
            );
        }
    }

    #[test]
    fn split_pulse_is_selected_and_validated() {
        let mut data = Vec::new();
        for index in 0..20_000 {
            data.extend_from_slice(
                format!(
                    "{{\"id\":{index},\"level\":\"INFO\",\"value\":{}}}\n",
                    index % 97
                )
                .as_bytes(),
            );
        }
        let archive = compress_slice(&data).unwrap();
        assert_eq!(&archive[..4], MAGIC_V8);
        assert_eq!(archive[HEADER], PULSE_SPLIT);
        assert_eq!(decompress_slice(&archive, data.len()).unwrap(), data);

        let mut damaged = archive;
        damaged[HEADER + BH] ^= 1;
        assert!(decompress_slice(&damaged, data.len()).is_err());
    }

    #[test]
    fn long_range_repetition_uses_references() {
        let base: Vec<u8> = (0..900_000)
            .map(|index| ((index * 29 + index / 101) & 0xff) as u8)
            .collect();
        let mut data = Vec::with_capacity(base.len() * 8);
        for _ in 0..8 {
            data.extend_from_slice(&base);
        }
        let archive = compress_slice(&data).unwrap();
        let analysis = analyze_archive(&archive).unwrap();
        assert!(analysis.reference_blocks > 0);
        assert!(analysis.referenced_bytes > data.len() as u64 * 3 / 4);
        assert_eq!(decompress_slice(&archive, data.len()).unwrap(), data);
    }

    #[test]
    fn invalid_forward_reference_is_rejected() {
        let base = b"long range reference validation ".repeat(20_000);
        let mut data = base.clone();
        data.extend_from_slice(&base);
        let mut archive = compress_slice(&data).unwrap();
        let mut cursor = HEADER;
        let mut block = 0u32;
        loop {
            let method = archive[cursor];
            let packed =
                u32::from_le_bytes(archive[cursor + 6..cursor + 10].try_into().unwrap()) as usize;
            cursor += BH;
            if method == REFERENCE {
                archive[cursor..cursor + 4].copy_from_slice(&block.to_le_bytes());
                assert!(decompress_slice(&archive, data.len()).is_err());
                break;
            }
            assert_ne!(method, END);
            cursor += packed;
            block += 1;
        }
    }

    #[test]
    fn reference_checksum_mismatch_is_rejected_without_rescanning() {
        let base = b"reference checksum validation ".repeat(24_000);
        let mut data = base.clone();
        data.extend_from_slice(&base);
        let mut archive = compress_slice(&data).unwrap();
        let mut cursor = HEADER;
        loop {
            let method = archive[cursor];
            let packed =
                u32::from_le_bytes(archive[cursor + 6..cursor + 10].try_into().unwrap()) as usize;
            if method == REFERENCE {
                archive[cursor + 10] ^= 0x80;
                assert!(matches!(
                    decompress_slice(&archive, data.len()),
                    Err(Error::ChecksumMismatch { .. })
                ));
                break;
            }
            assert_ne!(method, END);
            cursor += BH + packed;
        }
    }

    #[test]
    fn thread_limit_does_not_change_archive_bytes() {
        let mut data = Vec::new();
        for index in 0..80_000 {
            data.extend_from_slice(
                format!(
                    "{{\"id\":{index},\"bucket\":{},\"message\":\"stable\"}}\n",
                    index % 97
                )
                .as_bytes(),
            );
        }
        set_thread_limit(1);
        let single = compress_slice(&data).unwrap();
        set_thread_limit(2);
        let parallel = compress_slice(&data).unwrap();
        set_thread_limit(0);
        assert_eq!(single, parallel);
        assert_eq!(decompress_slice(&parallel, data.len()).unwrap(), data);
    }

    #[test]
    fn adversarial_cdc_shifts_and_near_duplicates_round_trip() {
        let base: Vec<u8> = (0..700_000)
            .map(|index| ((index * 41 + index / 127) & 0xff) as u8)
            .collect();
        let mut data = Vec::new();
        for shift in [1usize, 15, 255, 4095] {
            data.extend(std::iter::repeat_n(shift as u8, shift));
            data.extend_from_slice(&base);
            let mut changed = base.clone();
            for index in (0..changed.len()).step_by(65_521) {
                changed[index] ^= 0x5a;
            }
            data.extend_from_slice(&changed);
        }
        let archive = compress_slice(&data).unwrap();
        assert_eq!(decompress_slice(&archive, data.len()).unwrap(), data);
    }

    #[test]
    fn periodicity_and_distance_boundaries_round_trip() {
        for copies in [63usize, 64, 65, 127] {
            let mut data = b"periodic-boundary".repeat(copies);
            data.extend_from_slice(b"tail");
            let archive = compress_slice(&data).unwrap();
            assert_eq!(decompress_slice(&archive, data.len()).unwrap(), data);
        }
        for distance in [65_534usize, 65_535, 65_536] {
            let mut data = vec![0xa5; distance];
            let repeated = data[..4096].to_vec();
            data.extend_from_slice(&repeated);
            let archive = compress_slice(&data).unwrap();
            assert_eq!(decompress_slice(&archive, data.len()).unwrap(), data);
        }
    }

    #[test]
    fn slice_buffer_and_stream_decoders_agree() {
        let data = b"differential decoder path ".repeat(30_000);
        let archive = compress_slice(&data).unwrap();
        let slice = decompress_slice(&archive, data.len()).unwrap();
        let mut caller_owned = vec![0; data.len()];
        decompress_into_slice(&archive, &mut caller_owned, data.len()).unwrap();
        let mut stream_archive = Vec::new();
        compress_stream(
            &mut data.as_slice(),
            &mut stream_archive,
            CompressionOptions::default(),
        )
        .unwrap();
        let mut stream = Vec::new();
        decompress_stream(&mut stream_archive.as_slice(), &mut stream, data.len()).unwrap();
        assert_eq!(slice, data);
        assert_eq!(caller_owned, data);
        assert_eq!(stream, data);
    }
}
