# XCA4 binary format

All integers are unsigned and little-endian.

## Stream header

The 12-byte header contains `XCA4`, compression level, flags, reserved bytes, and maximum uncompressed block size.

## Block header

Each 14-byte header contains method, predictor, original size (`u32`), payload size (`u32`), and CRC-32. Method `255` followed by thirteen zero bytes terminates the stream.

Methods: `0` stored, `1` Pulse. Predictors: `0` none, `1` wrapping delta, `2` XOR.

## Pulse commands

- `00xxxxxx` and `01xxxxxx`: literal run of `tag + 1` bytes, covering lengths 1–128.
- `10xxxxxx`: short match of `(tag & 0x3f) + 4` bytes followed by a little-endian `u16` distance.
- `11000000`: long match followed by an unsigned LEB128-style length and a little-endian `u16` distance.
- Remaining `11xxxxxx` tags are reserved and rejected.

Valid match distances are 1–65,535. Valid lengths are 4–65,535. Overlapping matches are decoded byte by byte.
