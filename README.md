# XCA — eXtended Compression Algorithm

XCA 0.7.1 uses the XCA4 Pulse format and is an independently implemented, dependency-free lossless compression library and CLI written in Rust.

> XCA4 is a working experimental codec. It is not yet proven to beat LZ4 or LZMA universally; performance claims require reproducible corpus benchmarks.

## XCA 0.7.1 highlights

- `xca analyze` reports literal/match coverage, predictor usage, match lengths, and distances;
- parallel in-memory block decompression with deterministic output order;
- built-in `xca bench` command for CPU-only compression and decompression measurements;
- table-driven CRC-32 for substantially faster compression and decompression;
- generation-stamped per-thread hash workspaces with no per-block table clearing;
- allocation-free predictor scoring on sampled data;
- zero-copy raw input path when no predictor is selected;
- parallel in-memory block compression using all available CPU threads;
- dedicated direct-hash Turbo path for levels 1–3;
- faster overlap-copy decoding using geometric slice expansion;
- Pulse literal-run, short-match, and long-match command stream;
- matches up to 65,535 bytes instead of XCA3's 255-byte ceiling;
- sampled predictor sketch chooses raw, delta, or XOR mode without fully compressing every candidate;
- one dictionary pass per block, with sparse history updates at fast levels;
- automatic stored fallback for incompressible blocks;
- bounded-memory streaming over `Read` and `Write`;
- block CRC-32 and strict malformed-stream validation;
- Rust API, C ABI, static library, dynamic library, CLI, tests, and CI;
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

Use `compress_stream` and `decompress_stream` for large inputs.

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

## Cleanup

On Windows, preview and remove generated benchmark artifacts with `powershell -ExecutionPolicy Bypass -File scripts/cleanup-tests.ps1`. Add `-Force` to skip confirmation and `-DeleteBuild` to also run `cargo clean`.

## Current priorities

1. reproducible benchmark corpus and comparisons with LZ4, LZMA2, and Zstandard;
2. independent entropy backend for literal streams;
3. generation-stamped hash tables and parallel block scheduling;
4. fuzzing, compatibility vectors, and external format review;
5. SIMD acceleration where profiling demonstrates value.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
