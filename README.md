# XCA — eXtended Compression Algorithm

XCA is a dependency-free lossless compression library and CLI written in Rust. Version 0.2 is a working embeddable codec with a stable public API, a C ABI, corruption detection, bounded decompression, and backward decoding support for XCA1 files.

> XCA is usable, but it is not yet proven to outperform LZMA/LZMA2 or LZ4. Any such claim must come from reproducible measurements on a named workload.

## What is implemented

- adaptive stored/LZ encoding: incompressible input is not expanded by the codec payload;
- 64 KiB sliding window with level-dependent hash-chain match search;
- overlapping match decoding for repeated data;
- CRC-32 corruption detection;
- configurable decompression output limit to resist allocation bombs;
- strict validation of lengths, matches, truncated input, and trailing bytes;
- XCA2 metadata inspection without decompression;
- decoding compatibility with XCA1 stored and RLE frames;
- Rust `rlib`, native static library, dynamic library, and C header;
- CLI, tests, CI, benchmark harness, and no third-party runtime dependencies.

## Rust integration

Add the repository as a dependency while the crate is pre-release:

```toml
[dependencies]
xca = { git = "https://github.com/omnizs38/xca", tag = "v0.2.0" }
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

Until the `v0.2.0` tag is published, use `branch = "main"` instead of `tag = "v0.2.0"`.

## C and C++ integration

Build the C ABI and copy the generated library plus `include/xca.h` into the host project:

```bash
cargo build --release --features c-api
```

Typical outputs are `target/release/libxca.a`, `libxca.so`, `libxca.dylib`, or `xca.dll`, depending on the target.

```c
#include "xca.h"
#include <assert.h>
#include <string.h>

int main(void) {
    const uint8_t input[] = "data data data data";
    XcaBuffer compressed = {0};
    XcaBuffer restored = {0};

    assert(xca_compress(input, sizeof(input), 6, &compressed) == 0);
    assert(xca_decompress(compressed.data, compressed.len, &restored) == 0);
    assert(restored.len == sizeof(input));
    assert(memcmp(input, restored.data, restored.len) == 0);

    xca_free(compressed.data, compressed.len);
    xca_free(restored.data, restored.len);
    return 0;
}
```

## CLI

```bash
cargo build --release
cargo test --all-features

xca compress input.bin output.xca 6
xca decompress output.xca restored.bin
xca check output.xca
xca info output.xca
```

Levels range from 1 (fastest search) to 9 (deepest search). The decoder does not need the level.

## XCA2 frame

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | ASCII magic `XCA2` |
| 4 | 1 | method: `0` stored, `2` LZ |
| 5 | 1 | encoder level |
| 6 | 1 | flags; bit 0 enables CRC-32 |
| 7 | 1 | reserved |
| 8 | 8 | original size, little-endian `u64` |
| 16 | 8 | payload size, little-endian `u64` |
| 24 | 4 | CRC-32 of original data |
| 28 | N | encoded payload |

LZ payloads contain groups of up to eight tokens. Each group starts with a flag byte, least-significant bit first. A zero bit represents one literal byte. A one bit represents a two-byte little-endian distance followed by a one-byte match length. Valid distances are 1–65,535 and valid lengths are 3–255.

## Security model

Treat compressed files as untrusted input. `decompress` limits output to 1 GiB. Applications with smaller records should call `decompress_with_limit`. XCA validates every match and verifies CRC-32, but it has not yet received an independent security audit.

## Roadmap

1. chunked streaming frames and dictionary reuse;
2. canonical Huffman or rANS entropy coding;
3. domain transforms for logs, JSON, source code, and time series;
4. parallel block compression and optional SIMD;
5. fuzzing, corpus compatibility vectors, and independent format review;
6. reproducible comparison against LZMA, LZMA2, LZ4, and Zstandard.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
