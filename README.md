# XCA — eXtended Compression Algorithm

XCA4 Pulse is an independently implemented, dependency-free lossless compression library and CLI written in Rust.

> XCA4 is a working experimental codec. It is not yet proven to beat LZ4 or LZMA universally; performance claims require reproducible corpus benchmarks.

## XCA4 highlights

- new Pulse literal-run, short-match, and long-match command stream;
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
```

## Current priorities

1. reproducible benchmark corpus and comparisons with LZ4, LZMA2, and Zstandard;
2. independent entropy backend for literal streams;
3. generation-stamped hash tables and parallel block scheduling;
4. fuzzing, compatibility vectors, and external format review;
5. SIMD acceleration where profiling demonstrates value.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
