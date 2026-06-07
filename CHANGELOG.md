# Changelog

## [Unreleased]

## [0.3.0] - 2026-06-07

### Breaking

- Removed `compress_prepend_size` and `decompress_size_prepended` (free functions and `Compressor`/`Decompressor` methods). The 4-byte-size-prefixed block format was non-standard. Callers who need a size prefix can prepend it themselves.
- Removed `uncompressed_size` helper from `block` module.

## [0.2.1] - 2026-06-05

- README: moved performance charts to top, added sweep chart, moved CVE table to SAFETY.md
- Bench: pipeline_chart filters inputs to displayed codecs (fixes stray json_1k.json on aarch64)
- Charts: unified widths (summary/pipeline/sweep 850px, dict2k 400px)

## [0.2.0] - 2026-06-05

- Decompress: inline wildcopy for literals (32B chunks) and matches (tiered 8/16/32B), replacing memmove calls
- Decompress: fast-path-v2 handles short-literal/long-match tokens inline, avoiding the fully bounds-checked slow path
- Decompress: unified 16B literal / 18B match copy width on both x86_64 and aarch64 (aarch64 geomean vs C lz4: 1.61x -> 1.07x)
- Decompress: removed dead `wild_copy_32` and `wild_match_copy_32` (aarch64-only, replaced by unified paths)
- Bench: added lz4_flex unsafe (v0.11, crates.io) as fourth benchmark codec
- Bench: added `--sweep` mode for input-size sweep charts (64B-1MB synthetic JSON, with/without dict)
- README: replaced compress_prepend_size example, added function overview, dictionary and frame examples

## [0.1.0] - 2026-06-05

- Originally derived from PSeitz/lz4_flex
- Remove all unsafe code paths (safe-encode/safe-decode features)
- VerifiedSliceSink: pre-verified capacity eliminates per-write bounds checks
- Hash table: 5-byte hash, 8K U16 entries, unchecked lookups
- Decompress: offset>=8 fast path with copy_nonoverlapping
- Dictionary support for blocks and frames
- forbid(unsafe_code) on compress, decompress, sink, frame modules
- Benchmark suite with pipeline chart generation
