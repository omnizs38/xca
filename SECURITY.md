# Security policy

## Untrusted archives

Treat decoded size as attacker-controlled. Rust callers should use `DecodeOptions` or the
`*_with_limit` functions. C callers should use the limit-aware ABI. Python and C++ default
to a 256 MiB limit. Metadata inspection should use `frame_info_with_limit` or
`xca_decompressed_size_with_limit` before allocating caller-owned output.

`decompress_stream` accepts only independent-block streams produced by `compress_stream`.
It rejects global-reference archives so memory remains bounded by the configured block size.
It stops at the frame boundary; use `decompress_stream_exact` when trailing data must be rejected.
Parallel in-memory decoding also caps each batch's decoded-block working set.

Memory-mapped input files must not be truncated or replaced while an operation is running.
Applications that cannot guarantee file stability should read the input into owned memory.
On Unix, temporary output files are created with mode `0600`; existing destination permissions are restored before replacement.

## Reporting

Report suspected vulnerabilities privately through GitHub's security advisory interface.
Include the affected version, platform, reproducer, and expected resource limit.
