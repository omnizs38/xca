# XCA — eXtended Compression Algorithm

XCA is an experimental lossless compression framework written in Rust. Its long-term objective is to outperform LZMA/LZMA2 and LZ4 on carefully defined workloads by selecting domain-aware transforms, match finding, and entropy coding adaptively.

> **Status: early prototype.** The current codec is a safe, versioned baseline with adaptive raw/RLE selection. It does **not** yet outperform mature compressors. Performance claims will only be made with reproducible benchmark data.

## Why XCA

There is no universally best compressor. Compression ratio, throughput, memory, latency, and data type conflict. XCA therefore treats “better” as a measurable Pareto objective rather than an unsupported universal claim.

Initial target workloads:

- structured logs and repetitive text;
- JSON and source code;
- time-series blocks with predictable deltas;
- mixed binary corpora where fast incompressibility detection matters.

## Current features

- dependency-free Rust library and CLI;
- versioned `XCA1` frame format;
- adaptive choice between verbatim storage and byte RLE;
- bounded run decoding and decoded-length validation;
- round-trip and malformed-input tests;
- CI on Linux, macOS, and Windows.

## Build and use

```bash
cargo build --release
cargo test

./target/release/xca compress input.bin output.xca
./target/release/xca decompress output.xca restored.bin
./target/release/xca check output.xca
```

## Roadmap

1. **Corpus and harness** — deterministic benchmarks against `xz --format=lzma`, `xz` (LZMA2), and `lz4`; report ratio, encode/decode throughput, peak memory, and tool versions.
2. **Block analyzer** — entropy estimates, repetition scores, delta suitability, and fast incompressibility detection.
3. **Transforms** — delta/zigzag, record-aware preprocessing, executable filters, and optional dictionary training.
4. **Match finder** — hash chains first; binary trees or suffix structures only where measurements justify them.
5. **Entropy backend** — canonical Huffman baseline, then rANS/FSE-style coding with normalized tables.
6. **Adaptive pipeline** — per-block mode selection with explicit signaling overhead and deterministic decoding.
7. **Hardening** — fuzzing, corruption checks, resource limits, format specification, and compatibility vectors.
8. **Optimization** — profiling-led SIMD and parallel block processing without unsafe code unless benchmarks justify a narrowly audited implementation.

## Definition of success

XCA will be described as better than a baseline only for a published corpus and preset when it wins on at least one axis without unacceptable regression on the others. Example profiles:

- **Ratio profile:** smaller output than LZMA2 with bounded memory and documented time cost.
- **Balanced profile:** better compression than LZ4 while retaining high encode/decode throughput.
- **Domain profile:** materially better ratio and/or throughput on a named structured-data corpus.

Every result must include hardware, OS, compiler version, corpus hash, command lines, warm-up policy, repetitions, and median plus dispersion. The benchmark script in `scripts/benchmark.sh` is the starting point, not proof of superiority.

## Format (prototype)

| Field | Size | Meaning |
|---|---:|---|
| Magic | 4 bytes | ASCII `XCA1` |
| Method | 1 byte | `0` = stored, `1` = RLE |
| Original length | 8 bytes | little-endian `u64` |
| Payload | remaining bytes | method-specific data |

The format is experimental and may change before `1.0`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). New codec ideas must arrive with tests and benchmark evidence.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
