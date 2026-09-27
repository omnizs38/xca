# Contributing to XCA

Thank you for helping build XCA.

## Development workflow

1. Open an issue describing the workload and expected trade-off.
2. Keep changes small and measurable.
3. Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.
4. Add round-trip tests for codec changes and rejection tests for malformed input.
5. Include benchmark commands, corpus details, and before/after results for performance claims.

## Design rules

- Correctness and deterministic decoding come first.
- Never claim universal superiority from one corpus.
- New frame features must be versioned and documented.
- Decoders must reject malformed streams without unbounded allocation or panics.
- Avoid dependencies and `unsafe` unless their measured value clearly exceeds their maintenance and audit cost.

By contributing, you agree that your contribution is licensed under GPL-3.0-only.
