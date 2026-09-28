# Changelog

## 0.7.1 — 2026-09-28

- Added `xca analyze <archive>` for token-level diagnostics.
- Reports stored/Pulse blocks, predictor usage, literal coverage, short/long match counts, matched coverage, and average match length/distance.
- Added strict validation while analyzing untrusted archives.

## 0.7.0 — 2026-09-28

- Added parallel block decompression for in-memory callers and the CLI.
- Added strict parallel descriptor parsing, ordered reconstruction, per-block CRC validation, and aggregate output limits.
- Added `xca bench <input> [iterations] [level]` to measure codec CPU throughput without output-file I/O.
- Preserved XCA4 format compatibility and encoded sizes.
- Improved local large-file end-to-end decompression by roughly 40%.
- Retained strict Clippy and all correctness tests.

## 0.6.0 — 2026-09-28

- Replaced bit-at-a-time CRC-32 with a compile-time table-driven implementation.
- Added reusable generation-stamped hash workspaces for Turbo workers.
- Removed predictor sample allocations and raw-input copies.
- Preserved encoded sizes and the XCA4 on-disk format.
- Improved local large-corpus level-1 compression by 28%, level-5 compression by 10%, and decompression by roughly 35%.
- Retained strict Clippy and all correctness tests.

## 0.5.0 — 2026-09-28

- Added parallel in-memory block compression with deterministic output ordering.
- Added a direct-hash Turbo encoder for levels 1–3.
- Skipped predictor analysis on the fastest levels.
- Accelerated overlapping match decoding with slice expansion.
- Preserved the XCA4 on-disk format and streaming compatibility.
- Added a safe PowerShell cleanup utility for benchmark artifacts.
- Retained strict Clippy, round-trip, corruption, and resource-limit checks.

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
