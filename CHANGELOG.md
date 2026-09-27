# Changelog

## 0.2.0 — 2026-09-27

- Replaced the prototype encoder with a hash-chain LZ codec and adaptive stored mode.
- Added the XCA2 self-describing frame format and CRC-32 verification.
- Added bounded decompression and strict malformed-stream validation.
- Added frame metadata inspection.
- Added backward decoding support for XCA1 stored and RLE frames.
- Added optional C ABI, static/dynamic library outputs, and a C header.
- Expanded round-trip, corruption, compatibility, and resource-limit tests.

## 0.1.0 — 2026-09-27

- Added the initial XCA1 raw/RLE research prototype, CLI, tests, and benchmark harness.
