# XCA8 binary format

All integers are unsigned and little-endian. XCA 0.14 writes XCA8 and accepts legacy XCA4–XCA7 streams.

## Stream header

The 12-byte header contains:

1. four-byte magic `XCA8`;
2. one-byte profile field, always `0` for the unified profile;
3. one-byte flags field;
4. two reserved zero bytes;
5. maximum uncompressed block size as `u32`.

Flag bit 0 indicates per-block CRC-32 and is required. Flag bit 1 indicates possible long-range reference blocks. Other bits are rejected.

Legacy XCA4–XCA7 streams retain their historical level byte and are decoded for compatibility only. XCA8 has no compression levels.

## Block header

Each 14-byte header contains method, predictor, original size (`u32`), payload size (`u32`), and CRC-32. Method `255` followed by thirteen zero bytes terminates the stream.

Methods:

- `0`: stored bytes;
- `1`: Pulse command stream;
- `2`: canonical-Huffman-coded Pulse stream;
- `3`: Split Pulse;
- `4`: long-range reference.

Predictors: `0` none, `1` wrapping delta, `2` XOR.

## Pulse and entropy

Pulse represents literal runs, short matches, and long matches. Split Pulse independently stores command tags, literal bytes, long-match lengths, and distance codes. Each split stream automatically selects raw or canonical Huffman storage. Distances use a four-entry move-to-front cache.

## Long-range references

A reference payload is a little-endian `u32` index of an earlier block with identical reconstructed bytes. References must point backward, use predictor zero, match the referenced size, and pass their own CRC-32.

The encoder evaluates exact periodicity and content-defined chunks between 64 and 512 KiB. Fingerprints accelerate lookup only; byte-for-byte equality is mandatory. Content-defined chunking is retained only when references cover at least 1% of the input.

The decoder bounds entropy allocations and validates profile, flags, methods, predictors, stream boundaries, references, matches, checksums, terminators, and trailing data.
