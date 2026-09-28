# XCA3 binary format

All integers are unsigned and little-endian.

## Stream header (12 bytes)

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | ASCII `XCA3` |
| 4 | 1 | encoder level, 1–9 |
| 5 | 1 | format flags; currently `1` |
| 6 | 2 | reserved, zero |
| 8 | 4 | maximum uncompressed block size |

## Block header (13 bytes)

| Offset | Size | Field |
|---:|---:|---|
| 0 | 1 | method |
| 1 | 4 | original block size |
| 5 | 4 | payload size |
| 9 | 4 | CRC-32 of original block |

Methods: `0` stored, `1` RLE, `2` XCA-LZ, `3` delta + XCA-LZ, `4` XOR + XCA-LZ. Method `255` followed by twelve zero bytes is the stream terminator.

Payload size must not exceed original size. Blocks are independently decodable and checksummed.
