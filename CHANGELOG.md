# Changelog

## 0.11.1 — 2026-09-28

- Reused the already validated target checksum for long-range reference blocks instead of rescanning identical bytes.
- Removed the second full-memory pass from reference-heavy in-memory and streaming decompression.
- Added a strict reference-checksum mismatch test, bringing the suite to 18 tests.
- Improved local 16-copy decode throughput from roughly 372 MiB/s to 1,960 MiB/s without changing the XCA8 format or archive size.

## 0.11.0 — 2026-09-28

- Introduced XCA8 with one unified adaptive compression profile.
- Removed XCA levels from the CLI, Rust API, C API, stream options, and benchmark harness.
- Replaced fast/dense level branches with one balanced 16-candidate hash-chain parser.
- Added profile value zero for XCA8 while preserving XCA4–XCA7 decoding compatibility.
- Updated `xca info` to report `profile: unified`.
- Updated strict benchmarks to emit one `XCA Unified` result.

## 0.10.0 — 2026-09-28

- Introduced the XCA7 long-range deduplication format.
- Added exact periodicity detection and adaptive content-defined chunking.
- Added collision-safe backward reference blocks with independent CRC validation.
- Added direct ordered reference resolution and reference-aware streaming decode.
- Preserved fixed blocks when dedup coverage is below 1% and preserved the level 1–3 Turbo path.
- Added analyzer metrics for reference blocks and referenced byte coverage.
- Expanded the strict suite to 17 tests, including long-range round trips and forward-reference rejection.
- Reduced a local 64-copy development corpus from 29.17% with XCA6 level 5 to 0.46% with XCA7 level 5.

## 0.9.1 — 2026-09-28

- Fixed `benchmark-all.ps1` under Windows PowerShell when native codecs write progress to stderr while strict error handling is enabled.
- Native codec success is now determined by exit status; strict PowerShell errors remain enabled for the rest of the harness.

## 0.9.0 — 2026-09-28

- Introduced the XCA6 Split Pulse format.
- Separated tags, literals, long lengths, and distances into independently coded streams.
- Added a four-entry move-to-front distance cache and compact explicit-distance varints.
- Added direct Split Pulse decoding and adaptive fallback to XCA5 entropy or raw Pulse.
- Added strict bounds for entropy-decoded allocations and header flag validation.
- Expanded the suite to 15 tests covering deterministic output, boundary sizes, truncation, corruption, and resource limits.
- Added strict PowerShell runners for verified XCA/Zstandard/LZ4/LZMA2 comparisons with CSV, JSON, and environment metadata.
- Improved local level-5 archive size by 10.7% relative to XCA 0.8.0 on the development corpus.

## 0.8.0 — 2026-09-28

- Introduced the XCA5 format with adaptive per-block entropy coding.
- Added an independent canonical Huffman backend for Pulse payloads.
- Added a 12-bit first-level decode table with validated trie fallback.
- Preserved the Turbo path for levels 1–3 and XCA4 decoding compatibility.
- Added direct entropy round-trip and truncation tests.
- Improved local level-5 archive size by about 13% relative to XCA 0.7.1 on the development corpus.

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
