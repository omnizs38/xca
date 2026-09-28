# XCA — eXtended Compression Algorithm

XCA 0.10.0 is an independently implemented, dependency-free lossless compression library and CLI written in Rust. It writes XCA7 and decodes XCA4 through XCA7.

> XCA is experimental. No codec wins every corpus and metric. Use the included verified cross-engine benchmark for representative data.

## XCA 0.10.0 highlights

- global long-range deduplication across the complete in-memory input;
- exact periodicity detection for repeated snapshots and concatenated corpora;
- content-defined chunks from 64 to 512 KiB for shifted repetition;
- collision-safe byte-for-byte verification before references are emitted;
- compact four-byte backward block references;
- adaptive fixed-block fallback when dedup coverage is below 1%;
- XCA6 Split Pulse for every unique block;
- parallel independent-block decoding and direct ordered reference resolution;
- reference-specific CRC, length, direction, method, and flag validation;
- expanded 17-test adversarial suite;
- strict XCA/Zstandard/LZ4/LZMA2 PowerShell benchmark with SHA-256 verification;
- no third-party runtime or compression dependencies.

See [ALGORITHM.md](ALGORITHM.md) and [FORMAT.md](FORMAT.md).

## Rust

```toml
[dependencies]
xca = { git = "https://github.com/omnizs38/xca", branch = "main" }
```

```rust
let compressed = xca::compress_with_level(b"data data data", 5)?;
let restored = xca::decompress(&compressed)?;
```

`compress_with_level` enables global deduplication at levels 4–9. `compress_stream` keeps fixed independent blocks because arbitrary backward references require retained history.

## CLI

```bash
cargo build --release
xca compress input.bin output.xca 5
xca decompress output.xca restored.bin
xca check output.xca
xca info output.xca
xca analyze output.xca
xca bench input.bin 7 5
```

`xca analyze` reports Split Pulse usage plus `reference_blocks`, `referenced_bytes`, and `referenced_percent`.

## Strict cross-engine benchmark

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\strict-test.ps1 `
    -BenchmarkInput .\benchmark-large.bin `
    -Iterations 7
```

Benchmark multiple corpora directly:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\benchmark-all.ps1 `
    -InputPath .\text.tar, .\binaries.tar, .\logs.jsonl, .\random.bin `
    -Iterations 7
```

The harness requires XCA, Zstandard, 7-Zip, and 7-Zip ZS by default. It performs warm-up runs, reports medians, records environment metadata, and verifies every decompressed output with SHA-256.

## Cleanup

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\cleanup-tests.ps1 -Force
```

Add `-DeleteBuild` to also run `cargo clean`.

## Current priorities

1. accelerate reference-heavy decompression and CRC reuse;
2. improve CDC speed on high-entropy inputs;
3. add permanent binary compatibility vectors and fuzzing;
4. improve level-1 parsing and LZ4-class latency;
5. evaluate a bounded ANS entropy backend for unique blocks.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
