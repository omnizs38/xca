# XCA4 Pulse

XCA4 Pulse is a project-original, independently implemented adaptive lossless codec. It does not wrap or call another compression library.

## New design

XCA4 avoids XCA3's expensive strategy of fully encoding several candidates. Instead, it builds a small repetition sketch from at most 8 KiB of each block. The sketch selects raw, wrapping-delta, or XOR prediction before a single dictionary pass. This makes selection cheap and keeps compression close to one-pass operation.

Pulse uses a new command stream:

- literal-run commands carry 1–128 bytes with one tag;
- short matches carry lengths 4–67 and a 16-bit distance;
- long matches use a variable-length integer and can represent up to 65,535 bytes with one command;
- compression levels 1–3 sparsely update match history for speed, while higher levels inspect deeper chains.

Compared with XCA3, this removes the eight-token flag structure, reduces literal overhead, removes the 255-byte match ceiling, and avoids running five complete encoders per block.

## Adaptive sketch

The selector hashes three-byte sequences into a compact 4,096-entry sketch and measures confirmed repetition for raw, delta, and XOR samples. A predictor is selected only when it clears a safety margin over raw bytes. Incompressible blocks fall back to stored mode.

## Safety

Blocks remain independently checksummed with CRC-32. The decoder validates sizes, predictor IDs, command tags, varints, distances, match lengths, aggregate output limits, terminators, and trailing data before accepting a stream.

## Novelty statement

The Pulse command format, sampled predictor selector, sparse level-dependent history policy, framing, and implementation were designed for XCA. The underlying ideas of dictionary matching, hashing, delta prediction, XOR prediction, and variable-length integers are established techniques. No claim of academic novelty or universal superiority is made until prior-art review and reproducible benchmarks are complete.
