# Integrating XCA

XCA exposes the same unified codec through Rust, a stable C ABI, C++, Python `ctypes`, files, and the command line. No compression level must be selected.

## Rust memory API

```rust
let archive = xca::compress(source);
let restored = xca::decompress(&archive)?;
```

## Rust file API

```rust
let packed = xca::compress_file("input.bin", "output.xca")?;
let restored = xca::decompress_file("output.xca", "restored.bin")?;
```

Both return `FileStats { input_bytes, output_bytes }`.
The file helpers memory-map inputs and decode directly into the destination file,
avoiding a full-size intermediate output allocation and copy.

## Caller-owned output

```rust
let info = xca::frame_info(&archive)?;
let mut restored = vec![0; info.original_size];
xca::decompress_into(&archive, &mut restored)?;
```

Use `decompress_into_with_limit` when the application supplies its own decoded-size policy.

## C ABI

Build the shared and static libraries:

```bash
cargo build --release
```

Include `include/xca.h` and link the generated library:

- Windows: `target/release/xca.dll` or `xca.lib`;
- Linux: `target/release/libxca.so` or `libxca.a`;
- macOS: `target/release/libxca.dylib` or `libxca.a`.

```c
XcaBuffer archive = {0};
int32_t code = xca_compress(input, input_len, &archive);
if (code != 0) fprintf(stderr, "%s\n", xca_error_string(code));
xca_buffer_free(&archive);
```

All output memory is owned by XCA and must be released with `xca_buffer_free`. The function clears the pointer and length. `xca_version()` returns the linked library version.

High-throughput applications can avoid the library-owned decompression
allocation and subsequent copy:

```c
size_t restored_len = 0;
xca_decompressed_size(archive, archive_len, &restored_len);
uint8_t *restored = malloc(restored_len);
xca_decompress_into(archive, archive_len, restored, restored_len);
```

## C++

`include/xca.hpp` provides exception-based `std::vector<uint8_t>` wrappers:

```cpp
#include "xca.hpp"
auto archive = xca::compress(source);
auto restored = xca::decompress(archive);
```

## Python

Build the shared library, then import the dependency-free binding:

```bash
cargo build --release
python examples/python/basic.py
```

```python
from bindings import xca
archive = xca.compress(source)
restored = xca.decompress(archive)
```

Set `XCA_LIBRARY` to an absolute shared-library path when it is not under `target/release`.

## Any other language

Languages that can call a C ABI—C#, Go, Java/JNI, Kotlin/Native, Swift, Zig, Delphi, LuaJIT FFI, Ruby FFI, Node.js native addons, and others—can bind these functions from `xca.h`:

```c
const char *xca_version(void);
const char *xca_error_string(int32_t code);
int32_t xca_compress(const uint8_t *, size_t, XcaBuffer *);
int32_t xca_decompress(const uint8_t *, size_t, XcaBuffer *);
int32_t xca_decompressed_size(const uint8_t *, size_t, size_t *);
int32_t xca_decompress_into(const uint8_t *, size_t, uint8_t *, size_t);
void xca_buffer_free(XcaBuffer *);
```

The ABI uses only fixed-width integers, pointers, sizes, and a two-field buffer structure.

## CLI and subprocess integration

```bash
xca compress input.bin output.xca
xca decompress output.xca restored.bin
xca check output.xca
```

The CLI returns a non-zero process status on failure and can be embedded in scripts, build systems, backup tools, and server jobs.

## Compatibility

XCA 0.12.1 writes XCA8. The decoder accepts XCA4 through XCA8. New applications should treat the format as experimental until a stable 1.0 specification is published.
