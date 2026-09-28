# XCA8 Unified Adaptive Compression

XCA8 combines the XCA long-range deduplication, Split Pulse, predictor, dictionary, and entropy components into one deterministic profile. It does not expose compression levels and does not call another compression library.

## Decision pipeline

1. Detect exact whole-input periodicity.
2. Otherwise evaluate content-defined chunks for shifted long-range repetition.
3. Keep content-defined chunks only when at least 1% of the input can be referenced.
4. Select raw, wrapping-delta, or XOR prediction per unique block.
5. Parse matches with one balanced 16-candidate hash-chain policy.
6. Evaluate raw Pulse, canonical-Huffman Pulse, and Split Pulse.
7. Store incompressible blocks directly.
8. Emit backward references for byte-identical chunks.

All choices are automatic and deterministic. There is no speed/ratio level parameter.

## Long-range deduplication

Exact repeated chunks become four-byte backward references. Hash collisions cannot corrupt output because candidate chunks are compared byte for byte. Every reference has independent size and direction validation. Its checksum must equal the already validated target checksum, avoiding a redundant full-block CRC scan while preserving corruption detection.

## Split Pulse

Unique blocks separate command tags, literals, long lengths, and distances. Each stream independently chooses raw or canonical Huffman storage. Recent distances use a four-entry move-to-front cache.

## Parallelism and streaming

Unique blocks are compressed and decoded in parallel with deterministic ordering. References are resolved directly into the final output. Small outputs use low-overhead sequential assembly; outputs of at least 64 MiB are partitioned into disjoint ranges and assembled in parallel. The streaming encoder uses fixed independent blocks and the same unified block codec; in-memory compression additionally enables global deduplication.

## Strict validation

The suite covers unified-profile round trips, deterministic output, boundary sizes, truncation, payload corruption, entropy limits, Split Pulse, long-range references, forward-reference rejection, checksums, output limits, and XCA4–XCA7 compatibility.

## Novelty statement

The XCA framing, Pulse commands, predictor selector, adaptive representations, Split Pulse composition, and unified integration of periodic/content-defined references were designed for this project. Dictionary matching, hashing, canonical Huffman coding, content-defined chunking, move-to-front caches, and deduplication are established techniques. Benchmark conclusions remain corpus-specific.
