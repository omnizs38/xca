# XCA — eXtended Compression Algorithm

XCA 0.9.0 is an independently implemented, dependency-free lossless compression library and CLI written in Rust. It writes XCA6 and decodes XCA4, XCA5, and XCA6 archives.

> XCA is experimental. No codec wins every corpus and metric. Use the included strict cross-engine benchmark instead of relying on universal claims.

## XCA 0.9.0 highlights

- new XCA6 Split Pulse backend;
- independent streams for command tags, literals, long lengths, and distances;
- per-stream raw/canonical-Huffman selection;
- four-entry move-to-front distance cache with compact explicit-distance varints;
- direct Split Pulse decoding without reconstructing an intermediate command stream;
- adaptive fallback to XCA5 whole-stream entropy, raw Pulse, or stored blocks;
- strict entropy allocation bounds and stream-header validation;
- deterministic parallel compression/decompression;
- expanded 15-test adversarial and boundary matrix;
- strict PowerShell harness comparing XCA, Zstandard, LZ4, and LZMA2 with median timings and SHA-256 verification;
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

Use `compress_stream` and `decompress_stream` for bounded-memory I/O.

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

## Strict validation and cross-engine benchmark

Run formatting, strict Clippy, all tests, release build, and a verified benchmark:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\strict-test.ps1 `
    -BenchmarkInput .\benchmark-large.bin `
    -Iterations 7
```

Benchmark several representative corpora in one run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\benchmark-all.ps1 `
    -InputPath .\text.tar, .\binaries.tar, .\logs.jsonl, .\random.bin `
    -Iterations 7
```

The harness requires XCA, Zstandard, 7-Zip, and 7-Zip ZS by default. It performs one warm-up, reports median throughput, records environment metadata, and verifies every decompressed result with SHA-256. Results are written to CSV and JSON.

## Cleanup

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\cleanup-tests.ps1 -Force
```

Add `-DeleteBuild` to also run `cargo clean`.

## Current priorities

1. reduce Split Pulse table construction overhead;
2. improve high-level parsing efficiency and level-9 value;
3. add fuzzing and permanent compatibility vectors;
4. evaluate bounded asymmetric numeral coding as an optional backend;
5. use SIMD only where profiling demonstrates an end-to-end benefit.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
