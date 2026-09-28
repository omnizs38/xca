# XCA6 binary format

All integers are unsigned and little-endian. XCA 0.9 writes XCA6 and accepts legacy XCA4 and XCA5 streams.

## Stream header

The 12-byte header contains the four-byte magic, compression level, checksum flag, two reserved zero bytes, and maximum uncompressed block size. XCA6 requires the flag/reserved bytes to be exactly `1, 0, 0`.

## Block header

Each 14-byte header contains method, predictor, original size (`u32`), payload size (`u32`), and CRC-32. Method `255` followed by thirteen zero bytes terminates the stream.

Methods:

- `0`: stored bytes;
- `1`: XCA4 Pulse command stream;
- `2`: canonical-Huffman-coded Pulse stream (XCA5);
- `3`: XCA6 Split Pulse.

Predictors: `0` none, `1` wrapping delta, `2` XOR.

## Pulse commands

- `00xxxxxx` and `01xxxxxx`: literal run of `tag + 1` bytes, covering lengths 1–128.
- `10xxxxxx`: short match of `(tag & 0x3f) + 4` bytes followed by a distance.
- `11000000`: long match followed by an unsigned LEB128 length and a distance.
- Remaining `11xxxxxx` tags are reserved and rejected.

Legacy methods store distances as little-endian `u16`. Valid distances are 1–65,535 and valid lengths are 4–65,535.

## Method 2 entropy payload

The payload starts with the uncompressed Pulse-stream size as `u32`, followed by 256 canonical Huffman code lengths and an MSB-first bitstream padded with zero bits. Code lengths are limited to 24 bits.

## Method 3 Split Pulse payload

A Split Pulse payload begins with `SPL1`, followed by four streams in this order:

1. command tags;
2. literal bytes;
3. long-match length varints;
4. distance codes.

Each stream has a one-byte storage method and `u32` payload length. Storage method `0` is raw and method `1` uses the canonical Huffman payload above. Selection is independent for every stream.

Distances use a four-entry move-to-front cache. Values `0..3` reference a cache entry. Other values are unsigned LEB128 representations of `distance + 4`. Every referenced or explicit distance is promoted to cache position zero.

The decoder bounds every entropy allocation by the block size and validates all stream boundaries, canonical codes, cache references, commands, reconstructed sizes, CRC values, terminators, and trailing data.
