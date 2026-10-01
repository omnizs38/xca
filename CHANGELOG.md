# Changelog

## 0.13.0 — 2026-10-02

- Added a cross-platform benchmark runner with pinned tool and corpus downloads, SHA-256 validation, safe local extraction, offline caching, fixed single/automatic CPU profiles, raw timings, median/IQR summaries, environment metadata, and baseline-ref comparisons.
- Replaced platform-specific benchmark logic with thin PowerShell and shell wrappers around the shared Python runner.
- Added slicing-by-8 CRC-32, a precomputed CDC gear table, frequency-based Huffman bit accounting, compact reusable `u32` match workspaces, fast match comparison, and a no-thread small-input path.
- Reused target checksums while encoding long-range reference blocks.
- Added `set_thread_limit` and CLI `XCA_THREADS` support for controlled comparisons.
- Preserved byte-identical XCA8 output versus 0.12.2 across synthetic mixed, random, periodic, numeric, and near-duplicate corpora.
- Added CDC-shift, near-duplicate, periodicity, distance-boundary, thread-determinism, and decoder-path differential tests plus a compression round-trip fuzz target.
- Clarified that caller-owned decode avoids a second full-size output allocation but can still allocate internal block workspaces.

## 0.12.2 — 2026-09-30

- Changed file compression and decompression to write through a same-directory temporary file and atomically replace the destination only after successful flush and validation.
- Preserved existing destination files when an archive is corrupt or an output operation fails.
- Extended same-file protection to detect hard links using platform file identities in addition to canonical paths.
- Added regression coverage for destination preservation and hard-link aliases.
- Added a libFuzzer target for bounded frame parsing, validation, and decompression.
- Pinned third-party GitHub Actions to full commit SHAs and added a scheduled fuzz smoke test.

## 0.12.1 — 2026-09-30

- Fixed C API failure paths so output buffers are always cleared and can be safely released after an error.
- Fixed in-place file decompression corrupting its mapped input by rejecting input and output paths that resolve to the same file.
- Added regression coverage for failed C API calls and same-file decompression.

## 0.12.0 — 2026-09-28

- Added dependency-free memory-mapped file input and output on Windows and Unix.
- Added Rust and C APIs for decoding directly into caller-owned memory.
- Updated the C++ and Python bindings to use the direct-output decode path.
- Changed CLI decompression to assemble blocks directly into the mapped destination file, eliminating the full restored-buffer allocation and file-write copy.
- Changed CLI compression to read directly from mapped input and preallocate the archive file.
- Added validation-only `check` that does not assemble the complete restored output.
- Enabled full LTO and native-CPU strict benchmark builds.
- Added split-target release scripts that eliminate the Cargo MSVC PDB filename warning without renaming the public `xca` library or executable.
- Added direct-output correctness and size-mismatch coverage, bringing the suite to 20 tests.
- Improved local 78.1 MiB end-to-end decompression from 961 to 2,442 MiB/s and compression from 338 to 429 MiB/s while preserving identical archives.

## 0.11.2 — 2026-09-28

- Added adaptive parallel final-output assembly for large reference-heavy archives while retaining a low-overhead sequential path below 64 MiB.
- Added Rust `compress_file` and `decompress_file` helpers with byte-count statistics.
- Enabled the stable C ABI by default and added version, error-string, and buffer-lifecycle functions.
- Added a header-only C++ wrapper and a dependency-free Python `ctypes` binding.
- Added ready-to-run Rust, C, and Python examples plus a language-neutral integration guide.
- Added file-helper round-trip coverage, bringing the suite to 19 tests.

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
