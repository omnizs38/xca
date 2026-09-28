# XCA5 Pulse Entropy

XCA5 is a project-original composition of independently implemented components. It does not wrap or call another compression library.

## Pipeline

Each block is sampled to choose raw, wrapping-delta, or XOR prediction. Pulse then emits literal runs and dictionary matches. Levels 1–3 use the direct-hash Turbo path. Levels 4–9 also build a canonical byte-Huffman representation of the Pulse stream and select it only when the complete entropy payload is smaller. Incompressible blocks use stored mode.

This adaptive selection creates three independent block representations:

1. stored bytes for incompressible input;
2. raw Pulse for low overhead;
3. Pulse Entropy for skewed command and literal distributions.

## Pulse parser

- literal commands carry 1–128 bytes with one tag;
- short matches carry lengths 4–67 and a 16-bit distance;
- long matches use a variable-length integer and represent up to 65,535 bytes;
- fast levels use sparse/direct history, while higher levels inspect deeper chains.

## Entropy backend

The entropy stage counts every byte in the Pulse payload, constructs a bounded Huffman tree, converts it to deterministic canonical codes, and writes only the 256 code lengths plus the original payload size. Decoding uses a 12-bit first-level table for common codes and a validated compact trie for longer codes. The implementation has no external codec dependency.

## Safety and compatibility

Blocks are independently checksummed with CRC-32. The decoder validates sizes, predictor and method IDs, canonical codes, entropy transitions, command tags, varints, distances, match lengths, aggregate output limits, terminators, trailing data, and checksums. XCA5 decoders also accept XCA4 streams; XCA4 decoders do not understand method 2.

## Novelty statement

The XCA framing, Pulse command format, sampled predictor selector, level-dependent history policy, and adaptive Pulse/entropy selection were designed for this project. Dictionary matching, hashing, delta/XOR prediction, canonical Huffman coding, and variable-length integers are established techniques. No claim of academic novelty or universal superiority is made without prior-art review and reproducible independent benchmarks.
