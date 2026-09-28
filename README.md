# XCA — eXtended Compression Algorithm

XCA is a dependency-free lossless compression library and CLI written in Rust. Version 0.3 contains the independently implemented XCA3 adaptive block codec, a public Rust API, bounded-memory streaming, a C ABI, corruption detection, and strict resource limits.

> XCA is usable, but it is not yet proven to outperform LZMA/LZMA2 or LZ4. Any such claim must come from reproducible measurements on a named workload.

XCA does not call, wrap, or embed any external compression algorithm or compression library. See [ALGORITHM.md](ALGORITHM.md) for the design and [FORMAT.md](FORMAT.md) for the binary specification.

## What is implemented

- per-block selection among stored, RLE, XCA-LZ, delta + XCA-LZ, and XOR + XCA-LZ;
- independently implemented framing, predictors, match finder, token stream, and decoder;
- bounded-memory streaming compression and decompression;
- 64 KiB dictionary window with level-dependent hash-chain search;
- overlapping match decoding for repeated data;
- CRC-32 verification for every block;
- configurable aggregate output limits;
- strict validation of lengths, matches, truncation, and trailing bytes;
- Rust `rlib`, native static library, dynamic library, and C header;
- CLI, tests, CI, benchmark harness, and no third-party runtime dependencies.

## Rust integration

```toml
[dependencies]
xca = { git = "https://github.com/omnizs38/xca", branch = "main" }
```

```rust
fn main() -> Result<(), xca::Error> {
    let source = b"data data data data";
    let encoded = xca::compress_with_level(source, 6)?;
    let decoded = xca::decompress_with_limit(&encoded, 16 * 1024 * 1024)?;
    assert_eq!(decoded, source);
    Ok(())
}
```

For large inputs, use `compress_stream` and `decompress_stream` with any types implementing `Read` and `Write`.

## C and C++ integration

```bash
cargo build --release --features c-api
```

Copy the generated static or dynamic library and `include/xca.h` into the host project. Returned buffers must be released with `xca_free`.

## CLI

```bash
cargo build --release
cargo test --all-features

xca compress input.bin output.xca 6
xca decompress output.xca restored.bin
xca check output.xca
xca info output.xca
```

Levels range from 1 (fast search) to 9 (deep search). The decoder does not need the level.

## XCA3 format

XCA3 is a multi-block streaming format. Every block declares its selected pipeline, original size, payload size, and CRC-32. Blocks are independently decodable, and a deterministic marker terminates the stream. See [FORMAT.md](FORMAT.md).

## Security model

Treat compressed files as untrusted input. `decompress` limits output to 1 GiB. Applications with smaller records should call `decompress_with_limit`. XCA validates every match and verifies CRC-32, but it has not yet received an independent security audit.

## Roadmap

1. independent canonical entropy coding for literals and token fields;
2. dictionary reuse and optional seek indexes;
3. additional transforms for logs, JSON, source code, and time series;
4. parallel block compression and optional SIMD;
5. fuzzing, compatibility vectors, and independent format review;
6. reproducible comparison against LZMA, LZMA2, LZ4, and Zstandard.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
