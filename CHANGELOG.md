# Changelog

## 0.3.0 — 2026-09-27

- Introduced the fully independent XCA3 adaptive block codec.
- Removed the legacy encoder/decoder core from the active implementation.
- Added bounded-memory streaming compression and decompression.
- Added per-block selection among stored, RLE, XCA-LZ, delta-predictive XCA-LZ, and XOR-predictive XCA-LZ pipelines.
- Added per-block CRC-32, deterministic termination, strict block limits, and multi-block metadata.
- Added multi-block, predictive-transform, corruption, limit, and all-level round-trip tests.

## 0.2.0 — 2026-09-27

- Added the XCA2 hash-chain prototype, CRC-32, bounded decompression, C ABI, and integration examples.

## 0.1.0 — 2026-09-27

- Added the initial XCA1 raw/RLE research prototype, CLI, tests, and benchmark harness.
