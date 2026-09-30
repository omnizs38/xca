# XCA — eXtended Compression Algorithm

XCA 0.12.2 is an independently implemented, dependency-free lossless compression library and CLI written in Rust. It writes XCA8 and decodes XCA4 through XCA8.

XCA8 has one unified adaptive compression profile. There are no user-selectable XCA levels.

## Unified profile

For every input, XCA automatically applies the same decision pipeline:

- global exact-period detection;
- content-defined long-range deduplication when useful;
- sampled raw/delta/XOR predictor selection;
- one balanced hash-chain match parser;
- adaptive stored, Pulse, whole-stream Huffman, or Split Pulse representation;
- per-stream raw/Huffman selection;
- CRC-32 and strict structural validation.

This removes tuning ambiguity: the same `compress` operation is used for small files, binaries, logs, snapshots, and repeated corpora.

## XCA 0.12.2 highlights

- new XCA8 unified-profile format;
- removed levels from the Rust API, C API, CLI, benchmark harness, and stream options;
- one deterministic balanced search policy;
- long-range references across the complete in-memory input;
- adaptive fixed-block fallback when dedup coverage is below 1%;
- XCA6 Split Pulse for unique blocks;
- backward decoding compatibility with XCA4–XCA7;
- direct memory-mapped file input and decompression output on Windows and Unix;
- allocation-free `decompress_into` for caller-owned memory;
- adaptive parallel output assembly directly into files and external buffers;
- validation-only `check` without constructing the complete restored output;
- simple Rust file helpers plus C, C++, and Python integration layers;
- atomic file output that preserves an existing destination if compression or decompression fails;
- same-file detection for paths, symbolic links, and hard links;
- native-CPU strict release builds with full LTO;
- collision-free split library/CLI build scripts for MSVC;
- 22 strict adversarial, determinism, reference, corruption, file, and boundary tests;
- verified XCA/Zstandard/LZ4/LZMA2 PowerShell benchmark suite;
- no third-party runtime or compression dependencies.

See [ALGORITHM.md](ALGORITHM.md) and [FORMAT.md](FORMAT.md).

## Rust

```toml
[dependencies]
xca = { git = "https://github.com/omnizs38/xca", branch = "main" }
```

```rust
let compressed = xca::compress(b"data data data");
let restored = xca::decompress(&compressed)?;

let mut caller_owned = vec![0; b"data data data".len()];
xca::decompress_into(&compressed, &mut caller_owned)?;

xca::compress_file("input.bin", "output.xca")?;
xca::decompress_file("output.xca", "restored.bin")?;
```

## C

```c
XcaBuffer compressed;
int32_t result = xca_compress(data, length, &compressed);
xca_buffer_free(&compressed);
```

See [INTEGRATION.md](INTEGRATION.md) for complete Rust, C, C++, Python, generic C ABI, and subprocess examples.

## CLI

```bash
./scripts/build-release.sh
./target/release/xca compress input.bin output.xca
./target/release/xca decompress output.xca restored.bin
./target/release/xca check output.xca
./target/release/xca info output.xca
./target/release/xca analyze output.xca
./target/release/xca bench input.bin 7
```

On Windows, use `powershell -ExecutionPolicy Bypass -File .\scripts\build-release.ps1`.
Add `-Native` for a machine-optimized binary. Building library and CLI targets separately also avoids Cargo's same-name MSVC PDB collision.

`xca info` reports `profile: unified`. `xca analyze` reports Split Pulse and long-range reference coverage.

## Streaming

`CompressionOptions` contains only `block_size`. `compress_stream` uses the same unified block codec but keeps independent fixed blocks because arbitrary backward references require retained input history. In-memory `compress` enables global deduplication.

## Strict cross-engine benchmark

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\strict-test.ps1 `
    -BenchmarkInput .\benchmark-large.bin `
    -Iterations 7
```

The benchmark produces one `XCA Unified` row and compares it with explicitly named Zstandard, LZ4, and LZMA2 configurations. Every decompressed result is verified with SHA-256.

## Cleanup

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\cleanup-tests.ps1 -Force
```

Add `-DeleteBuild` to also run `cargo clean`.

## Current priorities

1. improve entropy table reuse;
2. improve CDC speed on high-entropy inputs;
3. add permanent binary compatibility vectors and fuzzing;
4. evaluate direct compressed-output assembly;
5. evaluate a bounded ANS backend selected automatically per stream.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
