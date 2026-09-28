# XCA3 algorithm

XCA3 is an independently implemented adaptive block-compression algorithm. It does not call, wrap, or embed LZMA, LZ4, DEFLATE, Zstandard, Brotli, or another compression library. The format, block selector, predictor combinations, framing, validation, and implementation belong to this project.

“Independent” does not mean ignoring established information theory. Dictionary matching, run-length encoding, delta prediction, and XOR prediction are general techniques. XCA combines them in its own deterministic format and implementation.

## Pipeline

1. Divide input into independently decodable blocks (256 KiB by default).
2. Build five candidates per block: stored bytes, byte RLE, XCA hash-chain dictionary encoding, delta prediction plus XCA dictionary encoding, and XOR prediction plus XCA dictionary encoding.
3. Select the smallest candidate. Ties favor the simpler pipeline evaluated earlier.
4. Store the method identifier, original and encoded lengths, and CRC-32 for each block.
5. Close the stream with a deterministic end marker.

This adapts locally: text, repeated bytes, counters, telemetry, and mixed binary regions in one file may use different pipelines.

## Dictionary representation

XCA uses a 65,535-byte backward distance and matches of 3–255 bytes. A 65,536-entry three-byte hash table points to history chains. Compression level controls the maximum candidates inspected per position.

Tokens are grouped in sets of eight. Each group starts with a flag byte, least-significant bit first:

- `0`: one literal byte;
- `1`: little-endian `u16` distance plus `u8` match length.

Overlapping copies are legal and decoded byte by byte.

## Predictors

Delta prediction stores the first byte followed by wrapping byte differences. XOR prediction stores the first byte followed by XOR differences. Both are reversible modulo 256. Their output passes through the XCA dictionary encoder.

## Resource model

Blocks are bounded between 4 KiB and 16 MiB. The decoder validates every length and match before writing output. Callers set an aggregate output limit. Streaming memory is proportional to one block rather than the entire input.

## Current limitations

XCA3 does not yet entropy-code literals or token fields. A future format version may add an independently implemented canonical entropy backend, dictionary reuse, parallel block scheduling, and seek indexes.
