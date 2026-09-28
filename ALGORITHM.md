# XCA6 Split Pulse

XCA6 is an independently implemented adaptive lossless codec. It does not wrap or call another compression library.

## Pipeline

Each block is sampled to choose raw, wrapping-delta, or XOR prediction. Pulse then finds literal runs and dictionary matches. Levels 1–3 retain the direct-hash Turbo path. Levels 4–9 evaluate Split Pulse and fall back to whole-stream Huffman or raw Pulse when splitting is not beneficial. Incompressible blocks use stored mode.

## Split Pulse

XCA5 entropy-coded the interleaved command stream with one byte model. XCA6 separates command tags, literals, long lengths, and distances. Each stream receives its own frequency model and independently chooses raw or canonical Huffman storage. This prevents literal bytes from diluting command and distance statistics.

Match distances use a four-entry move-to-front cache. Repeated recent distances become one-byte cache references; new distances use `distance + 4` varints. The decoder reconstructs output directly from the four streams without rebuilding the interleaved Pulse buffer.

## Match parser

- literal commands carry 1–128 bytes;
- short matches carry lengths 4–67;
- long matches represent up to 65,535 bytes;
- fast levels use direct sparse history;
- higher levels inspect progressively deeper hash chains.

## Safety

XCA6 validates stream header flags, block sizes, methods, predictors, entropy code spaces, decoded entropy limits, split-stream boundaries, distance-cache references, varints, match bounds, aggregate output limits, terminators, trailing data, and per-block CRC-32. XCA6 decoders accept XCA4 and XCA5 archives.

The strict test matrix covers every compression level, deterministic output, boundary sizes, pseudorandom data, all truncated prefixes of a compact archive, sampled payload corruption, Split Pulse corruption, entropy allocation limits, checksums, and output limits.

## Novelty statement

The XCA framing, Pulse command format, sampled predictor selector, level-dependent history policy, adaptive representation choice, and Split Pulse composition were designed for this project. Dictionary matching, hashing, delta/XOR prediction, canonical Huffman coding, move-to-front caches, and variable-length integers are established techniques. Universal superiority is not claimed without reproducible independent benchmarks.
