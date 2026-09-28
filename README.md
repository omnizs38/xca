# XCA — eXtended Compression Algorithm

XCA 0.8.0 is an independently implemented, dependency-free lossless compression library and CLI written in Rust. It writes the XCA5 format and remains able to decode XCA4 archives.

> XCA is an experimental codec. It is not claimed to beat LZ4, Zstandard, or LZMA2 on every workload; compare codecs on representative data with reproducible settings.

## XCA 0.8.0 highlights

- new XCA5 adaptive entropy format;
- new independent canonical Huffman backend for Pulse command streams;
- per-block choice among stored, Pulse, and Pulse+Huffman representations;
- 12-bit first-level Huffman decode table with validated trie fallback for long codes;
- levels 1–3 retain the low-latency Turbo path; levels 4–9 may use entropy coding when it reduces size;
- XCA4 backward decoding compatibility;
- `xca analyze` token-level diagnostics and `xca bench` CPU-only measurements;
- parallel in-memory block compression and decompression with deterministic ordering;
- table-driven CRC-32, strict malformed-stream validation, and bounded output;
- adaptive raw, wrapping-delta, and XOR predictors;
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

1. split entropy streams and lower match-distance overhead;
2. reproducible corpus comparisons with LZ4, LZMA2, and Zstandard;
3. fuzzing, compatibility vectors, and external format review;
4. SIMD acceleration where profiling demonstrates value.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
