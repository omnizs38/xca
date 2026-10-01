# Security policy

## Untrusted archives

Treat decoded size as attacker-controlled. Rust callers should use `DecodeOptions` or the
`*_with_limit` functions. C callers should use the limit-aware ABI. Python and C++ default
to a 256 MiB limit.

`decompress_stream` accepts only independent-block streams produced by `compress_stream`.
It rejects global-reference archives so memory remains bounded by the configured block size.

Memory-mapped input files must not be truncated or replaced while an operation is running.
Applications that cannot guarantee file stability should read the input into owned memory.

## Reporting

Report suspected vulnerabilities privately through GitHub's security advisory interface.
Include the affected version, platform, reproducer, and expected resource limit.
