# Changelog

## [Unreleased]

## [0.7.0] - 2026-06-18

### Breaking

- `Compressor<'a>` renamed to `CompressorRef<'a>` (no-alloc, borrows dictionary).
- `Decompressor<'a>` renamed to `DecompressorRef<'a>` (no-alloc, borrows dictionary).

### Added

- `Compressor` (no lifetime parameter, requires `alloc`): owns dictionary as `Vec<u8>`, restoring the v0.5 ergonomic API. Wraps `CompressorRef` internally with one contained `unsafe` for the self-referential borrow.
- `Decompressor` (no lifetime parameter, requires `alloc`): owns dictionary as `Vec<u8>`, delegates to `DecompressorRef` per call. Zero `unsafe`.

## [0.6.0] - 2026-06-18

### Breaking

- `Compressor` is now `Compressor<'a>`: borrows the dictionary (`&'a [u8]`) instead of cloning into `Vec<u8>`. `Compressor::new()` returns `Compressor<'static>`. Code that stored `Compressor` in a struct without a lifetime parameter needs updating.
- `Decompressor` is now `Decompressor<'a>`: borrows the dictionary instead of cloning. Same migration as `Compressor`.

### Added

- No-alloc support for block compress and decompress. Hash tables are stack-allocated (~8 KB) when the `alloc` feature is off. All `_into` functions, `Compressor`, and `Decompressor` work without `alloc`.
- `alloc` feature flag on `lz4rip`, `lz4rip-encode`, and `lz4rip-decode`. `std` implies `alloc`. Without `alloc`, `compress()`/`decompress()` (Vec-returning), and `DictTrainer` are unavailable.
- `compress_into_with_dict(input, output, dict)` free function for one-shot dictionary compression without a `Compressor` struct.
- CI: `cargo check --no-default-features` for encode, decode, and facade crates.

## [0.5.2] - 2026-06-17

- Fixed nightly clippy: removed duplicated `#[forbid(unsafe_code)]` attributes in `lz4rip-encode` and `lz4rip-decode` (redundant with crate-level `#[forbid]` on `mod` declarations), added `Default` impl for `HashTableU32`

## [0.5.1] - 2026-06-16

- Fixed version in README dependency example

## [0.5.0] - 2026-06-16

### Changed

- Split into workspace: `lz4rip-core`, `lz4rip-encode`, `lz4rip-decode`. Encoder and decoder are separate LLVM compilation units, eliminating the LTO poisoning class of regressions (dead encoder code shifting register allocation in the decoder). Public API unchanged.
- Decompress: `likely()` hint on fast-path gate. Previously a dead end due to register allocation interference when encoder and decoder shared a module; the crate split makes it effective. 4-17% decompress improvement across Silesia corpus, gap vs C lz4 drops from 10-33% to 2-21%.

## [0.4.0] - 2026-06-11

### Breaking

- Removed legacy frame decoding (`LZ4F_LEGACY_MAGIC_NUMBER`, `FrameInfo::is_legacy_frame()`, `BlockSize::Max8MB`). Only standard LZ4 frames (magic `0x184D2204`) are supported.

### Changed

- Hash tables are 8 KB across the board: 4K×u16 (8 KB) for inputs below 64 KB, 2K×u32 (8 KB) above. Half the 16 KB that C lz4 and lz4_flex use.
- `Compressor` uses epoch-based table reuse for inputs up to 8 KB, skipping the hash table clear between calls
- `Compressor::with_dict` uses dual 2K×u16 tables (8 KB total): a cleared main table and a read-only pristine table probed on main-table miss. Falls back to single-table path when dict+input exceeds u16 range.
- `Compressor` internals restructured as `Plain`/`Dict` enum, each variant holds only the tables it needs
- `compress_internal` is self-contained (no `Option<&T>` dict_table parameter in the hot loop). Separate `compress_with_dict_table` for the dual-table dict path.

### Added

- `DictTrainer`: COVER-based dictionary trainer for block compression. Collects samples, selects high-frequency segments, outputs a raw dict for `Compressor::with_dict`

## [0.3.1] - 2026-06-08

- Added `categories = ["compression"]` to Cargo.toml metadata

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
