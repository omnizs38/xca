# XCA7 Long-Range Pulse

XCA7 adds a global deduplication layer to the independently implemented XCA6 Split Pulse codec. It does not call or wrap another compression library.

## Adaptive pipeline

1. Levels 1–3 retain fixed 256 KiB Turbo blocks.
2. Levels 4–9 look for exact periodic repetition across the complete input.
3. Non-periodic inputs are evaluated with content-defined chunking from 64 to 512 KiB.
4. Fingerprint matches are verified byte for byte.
5. Repeated chunks become four-byte backward references.
6. Unique chunks use stored, raw Pulse, whole-stream Huffman, or Split Pulse representation.
7. Content-defined chunking is rejected when referenced coverage is below 1%, preserving fixed-block behavior on ordinary and incompressible data.

## Why it matters

XCA6 reset its dictionary at every block and could not exploit repetition separated by megabytes. XCA7 reference blocks can reuse an earlier chunk anywhere in the archive. Exact repeated backups, VM images, logs, source trees, and concatenated snapshots no longer need to encode each copy independently.

## Integrity and determinism

Chunk fingerprints never establish equality by themselves. Every candidate is compared byte for byte before encoding. Each reference carries the original block size and CRC-32, points strictly backward, and is revalidated during decoding. Deterministic chunking, first-occurrence selection, and ordered block output produce identical archives for identical inputs.

Independent blocks are decoded in parallel. References are then resolved in archive order directly into the final output buffer. Streaming decompression retains block history only when the XCA7 reference flag is present; ordinary streams remain bounded-memory.

## Split Pulse

Unique blocks continue to separate command tags, literals, long lengths, and distances. Each stream independently selects raw or canonical Huffman storage. A four-entry move-to-front cache gives common distances compact codes.

## Strict validation

The test suite covers all levels, deterministic output, size boundaries, truncation, corruption, entropy limits, Split Pulse validation, long-range references, forward-reference rejection, checksums, and legacy XCA4/XCA5/XCA6 decoding.

## Novelty statement

The XCA framing, Pulse commands, predictor selector, history policy, adaptive representation selection, Split Pulse composition, and integration of periodic/content-defined long-range references were designed for this project. Dictionary matching, hashing, content-defined chunking, canonical Huffman coding, move-to-front caches, and deduplication are established techniques. Benchmark claims remain corpus- and configuration-specific.
