# XCA7 binary format

All integers are unsigned and little-endian. XCA 0.10 writes XCA7 and accepts legacy XCA4, XCA5, and XCA6 streams.

## Stream header

The 12-byte header contains the four-byte magic, compression level, flags, two reserved zero bytes, and maximum uncompressed block size.

Flag bit 0 indicates per-block CRC-32 and is required. Flag bit 1 indicates that the archive may contain long-range reference blocks. Other bits are rejected.

## Block header

Each 14-byte header contains method, predictor, original size (`u32`), payload size (`u32`), and CRC-32. Method `255` followed by thirteen zero bytes terminates the stream.

Methods:

- `0`: stored bytes;
- `1`: XCA4 Pulse command stream;
- `2`: canonical-Huffman-coded Pulse stream;
- `3`: XCA6 Split Pulse;
- `4`: XCA7 long-range reference.

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

A Split Pulse payload begins with `SPL1`, followed by independently raw- or Huffman-coded streams for command tags, literal bytes, long-match length varints, and distance codes. Distances use a four-entry move-to-front cache; codes `0..3` reference cache entries and other values are LEB128 representations of `distance + 4`.

## Method 4 long-range reference

The payload is a little-endian `u32` index of an earlier block whose reconstructed bytes are identical. References must point backward, use predictor zero, have a four-byte payload, match the referenced block length, and pass their own CRC-32 check.

The XCA7 encoder first detects exact whole-input periodicity. Otherwise it evaluates content-defined chunks between 64 and 512 KiB, with an average target near 256 KiB. Chunk fingerprints are only lookup accelerators: byte-for-byte equality is required before a reference is emitted. Content-defined chunking is selected only when references cover at least 1% of the input; otherwise XCA retains fixed blocks.

The decoder bounds entropy allocations, validates all references and stream boundaries, and rejects forward references, invalid flags, malformed codes, invalid matches, trailing data, and checksum failures.
