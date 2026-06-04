# Changelog

## [Unreleased]

## [0.1.0] - 2026-06-05

- Originally derived from PSeitz/lz4_flex
- Remove all unsafe code paths (safe-encode/safe-decode features)
- VerifiedSliceSink: pre-verified capacity eliminates per-write bounds checks
- Hash table: 5-byte hash, 8K U16 entries, unchecked lookups
- Decompress: offset>=8 fast path with copy_nonoverlapping
- Dictionary support for blocks and frames
- forbid(unsafe_code) on compress, decompress, sink, frame modules
- Benchmark suite with pipeline chart generation
