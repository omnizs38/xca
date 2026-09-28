use crate::Error;
use std::{cmp::Reverse, collections::BinaryHeap};

const SYMBOLS: usize = 256;
const HEADER_SIZE: usize = 4 + SYMBOLS;
const MAX_BITS: u8 = 24;

#[derive(Clone)]
struct TreeNode {
    left: Option<usize>,
    right: Option<usize>,
    symbol: Option<u8>,
}

pub(crate) fn encode(input: &[u8]) -> Option<Vec<u8>> {
    if input.is_empty() || input.len() > u32::MAX as usize {
        return None;
    }
    let mut frequencies = [0u64; SYMBOLS];
    for &byte in input {
        frequencies[byte as usize] += 1;
    }
    let lengths = code_lengths(&frequencies)?;
    let codes = canonical_codes(&lengths)?;
    let bit_count: usize = input.iter().map(|&b| lengths[b as usize] as usize).sum();
    let mut output = Vec::with_capacity(HEADER_SIZE + bit_count.div_ceil(8));
    output.extend_from_slice(&(input.len() as u32).to_le_bytes());
    output.extend_from_slice(&lengths);
    let mut buffer = 0u64;
    let mut buffered = 0u8;
    for &byte in input {
        let (code, length) = codes[byte as usize];
        buffer = (buffer << length) | code as u64;
        buffered += length;
        while buffered >= 8 {
            buffered -= 8;
            output.push((buffer >> buffered) as u8);
            buffer &= if buffered == 0 {
                0
            } else {
                (1u64 << buffered) - 1
            };
        }
    }
    if buffered != 0 {
        output.push((buffer << (8 - buffered)) as u8);
    }
    Some(output)
}

pub(crate) fn decode(input: &[u8]) -> Result<Vec<u8>, Error> {
    const TABLE_BITS: u8 = 12;
    const TABLE_SIZE: usize = 1 << TABLE_BITS;
    const INVALID: i32 = i32::MIN;

    if input.len() < HEADER_SIZE {
        return Err(Error::InvalidEntropyData);
    }
    let expected = u32::from_le_bytes(input[..4].try_into().unwrap()) as usize;
    let lengths: [u8; SYMBOLS] = input[4..HEADER_SIZE].try_into().unwrap();
    let codes = canonical_codes(&lengths).ok_or(Error::InvalidEntropyData)?;
    let mut trie = vec![DecodeNode::default()];
    for (symbol, &(code, length)) in codes.iter().enumerate() {
        if length == 0 {
            continue;
        }
        let mut node = 0usize;
        for shift in (0..length).rev() {
            let bit = ((code >> shift) & 1) as usize;
            let next = trie[node].child[bit];
            node = if next == usize::MAX {
                let created = trie.len();
                trie.push(DecodeNode::default());
                trie[node].child[bit] = created;
                created
            } else {
                next
            };
        }
        if trie[node].symbol.is_some() {
            return Err(Error::InvalidEntropyData);
        }
        trie[node].symbol = Some(symbol as u8);
    }

    // A wide first-level table resolves the common case with one lookup. Codes
    // longer than TABLE_BITS continue through the compact binary trie.
    let mut table = vec![INVALID; TABLE_SIZE];
    for (prefix, entry) in table.iter_mut().enumerate() {
        let mut node = 0usize;
        let mut consumed = 0u8;
        while consumed < TABLE_BITS {
            let bit = (prefix >> (TABLE_BITS - consumed - 1)) & 1;
            node = trie[node].child[bit];
            if node == usize::MAX {
                break;
            }
            consumed += 1;
            if let Some(symbol) = trie[node].symbol {
                *entry = ((consumed as i32) << 8) | symbol as i32;
                break;
            }
        }
        if *entry == INVALID && node != usize::MAX && consumed == TABLE_BITS {
            *entry = -(node as i32) - 1;
        }
    }

    let payload = &input[HEADER_SIZE..];
    let mut position = 0usize;
    let mut bits = 0u8;
    let mut buffer = 0u64;
    let mut output = Vec::with_capacity(expected);
    while output.len() < expected {
        while bits < TABLE_BITS && position < payload.len() {
            buffer = (buffer << 8) | payload[position] as u64;
            position += 1;
            bits += 8;
        }

        if bits >= TABLE_BITS {
            let prefix = ((buffer >> (bits - TABLE_BITS)) & (TABLE_SIZE as u64 - 1)) as usize;
            let entry = table[prefix];
            if entry == INVALID {
                return Err(Error::InvalidEntropyData);
            }
            if entry >= 0 {
                let length = (entry >> 8) as u8;
                bits -= length;
                output.push(entry as u8);
                continue;
            }

            bits -= TABLE_BITS;
            let mut node = (-entry - 1) as usize;
            loop {
                if let Some(symbol) = trie[node].symbol {
                    output.push(symbol);
                    break;
                }
                if bits == 0 {
                    if position >= payload.len() {
                        return Err(Error::Truncated);
                    }
                    buffer = (buffer << 8) | payload[position] as u64;
                    position += 1;
                    bits = 8;
                }
                bits -= 1;
                let bit = ((buffer >> bits) & 1) as usize;
                node = trie[node].child[bit];
                if node == usize::MAX {
                    return Err(Error::InvalidEntropyData);
                }
            }
            continue;
        }

        // Only the padded tail can reach this path.
        let mut node = 0usize;
        loop {
            if bits == 0 {
                return Err(Error::Truncated);
            }
            bits -= 1;
            let bit = ((buffer >> bits) & 1) as usize;
            node = trie[node].child[bit];
            if node == usize::MAX {
                return Err(Error::InvalidEntropyData);
            }
            if let Some(symbol) = trie[node].symbol {
                output.push(symbol);
                break;
            }
        }
    }
    Ok(output)
}

#[derive(Clone, Copy)]
struct DecodeNode {
    child: [usize; 2],
    symbol: Option<u8>,
}
impl Default for DecodeNode {
    fn default() -> Self {
        Self {
            child: [usize::MAX; 2],
            symbol: None,
        }
    }
}

fn code_lengths(frequencies: &[u64; SYMBOLS]) -> Option<[u8; SYMBOLS]> {
    let mut nodes = Vec::<TreeNode>::new();
    let mut heap = BinaryHeap::<Reverse<(u64, usize, usize)>>::new();
    let mut serial = 0usize;
    for (symbol, &frequency) in frequencies.iter().enumerate() {
        if frequency == 0 {
            continue;
        }
        let index = nodes.len();
        nodes.push(TreeNode {
            left: None,
            right: None,
            symbol: Some(symbol as u8),
        });
        heap.push(Reverse((frequency, serial, index)));
        serial += 1;
    }
    if heap.is_empty() {
        return None;
    }
    if heap.len() == 1 {
        let mut lengths = [0u8; SYMBOLS];
        let Reverse((_, _, index)) = heap.pop().unwrap();
        lengths[nodes[index].symbol.unwrap() as usize] = 1;
        return Some(lengths);
    }
    while heap.len() > 1 {
        let Reverse((left_frequency, _, left)) = heap.pop().unwrap();
        let Reverse((right_frequency, _, right)) = heap.pop().unwrap();
        let index = nodes.len();
        nodes.push(TreeNode {
            left: Some(left),
            right: Some(right),
            symbol: None,
        });
        heap.push(Reverse((left_frequency + right_frequency, serial, index)));
        serial += 1;
    }
    let Reverse((_, _, root)) = heap.pop().unwrap();
    let mut lengths = [0u8; SYMBOLS];
    let mut stack = vec![(root, 0u8)];
    while let Some((index, depth)) = stack.pop() {
        if depth > MAX_BITS {
            return None;
        }
        if let Some(symbol) = nodes[index].symbol {
            lengths[symbol as usize] = depth.max(1);
        } else {
            stack.push((nodes[index].left.unwrap(), depth + 1));
            stack.push((nodes[index].right.unwrap(), depth + 1));
        }
    }
    Some(lengths)
}

fn canonical_codes(lengths: &[u8; SYMBOLS]) -> Option<[(u32, u8); SYMBOLS]> {
    if lengths.iter().any(|&length| length > MAX_BITS) {
        return None;
    }
    let mut symbols: Vec<(u8, usize)> = lengths
        .iter()
        .enumerate()
        .filter_map(|(s, &l)| (l != 0).then_some((l, s)))
        .collect();
    symbols.sort_unstable();
    let mut result = [(0u32, 0u8); SYMBOLS];
    let mut code = 0u32;
    let mut previous = 0u8;
    for (length, symbol) in symbols {
        code = code.checked_shl((length - previous) as u32)?;
        if length < 32 && code >= (1u32 << length) {
            return None;
        }
        result[symbol] = (code, length);
        code = code.checked_add(1)?;
        previous = length;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn round_trip_skewed_bytes() {
        let mut input = vec![0u8; 20_000];
        for (index, byte) in input.iter_mut().enumerate() {
            if index % 11 == 0 {
                *byte = (index % 251) as u8;
            }
        }
        let encoded = encode(&input).expect("skewed data should be encodable");
        assert!(encoded.len() < input.len());
        assert_eq!(decode(&encoded).unwrap(), input);
    }

    #[test]
    fn round_trip_single_symbol() {
        let input = vec![b'X'; 4096];
        let encoded = encode(&input).unwrap();
        assert_eq!(decode(&encoded).unwrap(), input);
    }

    #[test]
    fn rejects_truncated_payload() {
        let input = b"entropy entropy entropy entropy".repeat(100);
        let mut encoded = encode(&input).unwrap();
        encoded.pop();
        assert!(decode(&encoded).is_err());
    }
}
