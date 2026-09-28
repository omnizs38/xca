# Changelog

## 0.4.0 — 2026-09-27

- Introduced XCA4 Pulse, a new project-original command stream.
- Added cheap sampled predictor selection instead of five full candidate encodes.
- Added literal runs, compact short matches, and varint long matches up to 65,535 bytes.
- Added sparse history updates for fast levels and deeper searches for ratio-oriented levels.
- Preserved bounded streaming, stored fallback, block CRC-32, and strict validation.
- Added mixed-data, long-match, predictor, corruption, output-limit, and all-level tests.

## 0.3.0 — 2026-09-27

- Introduced the independent XCA3 adaptive block prototype and streaming format.

## 0.2.0 — 2026-09-27

- Added the XCA2 hash-chain prototype, CRC-32, bounded decompression, and C ABI.

## 0.1.0 — 2026-09-27

- Added the initial XCA1 raw/RLE research prototype.
