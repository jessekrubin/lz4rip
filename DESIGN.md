# Design

Divergences from C lz4 and lz4_flex. Each section explains what changed, why, and the measured tradeoff.

## Skip acceleration

When the compressor fails to find a match, it skips ahead with increasing step size. The step grows by 1 every `1 << INCREASE_STEPSIZE_BITSHIFT` consecutive misses.

| | C lz4 | lz4rip |
|---|---|---|
| `INCREASE_STEPSIZE_BITSHIFT` | 6 | 3 |
| First skip at | 64 misses | 8 misses |

Measured impact (Silesia corpus, x86):
- Incompressible data (sao): 3.4x faster compression
- Literary text (dickens): 11% faster compression
- Ratio cost: ~8 percentage points on 10 MB text

The value 3 was selected by a systematic parameter sweep (`benches/param_sweep.py`) over 72 combinations of bitshift, hash table sizes, and hash byte counts across the full corpus. `bs3` ranks #1 or #2 at 1 GB/s transfer.

Crossover analysis against `bs6` (C lz4's default) at various transfer bandwidths:

| Transfer | bs3 vs bs6 | Winner |
|---|---|---|
| 1 MB/s | +2.4% | bs6 |
| 10 MB/s | +1.8% | bs6 |
| 50 MB/s | -0.1% | bs3 |
| 1 GB/s | -8.4% | bs3 |
| infinite | -10.6% | bs3 |

Below ~50 MB/s, the 8pp ratio difference dominates and bs6 is 1-2% faster end-to-end. Above that, bs3's compression speed advantage takes over. For memory-to-memory, IPC, local storage, and datacenter networking, bs3 is the right choice.

## Hash tables

Two hash table implementations, both 8 KB, selected by input size:

| Table | Entries | Value width | Footprint | Used when |
|---|---|---|---|---|
| `HashTableU32U16` | 4K | 16-bit | 8 KB | dict + input < 64 KB |
| `HashTableU32` | 2K | 32-bit | 8 KB | larger inputs |

Consistent 8 KB L1d footprint. Half the 16 KB that C lz4 (`LZ4_hash4`) and lz4_flex use.

`Compressor::with_dict` uses two `HashTableU32U16` tables (8 KB total): a cleared main table and a read-only pristine table probed on main-table miss. Falls back to the single-table free-function path when dict+input exceeds u16 range.

`Compressor` without dict uses epoch-based table reuse for inputs up to 8 KB: instead of clearing the hash table between calls, it advances a stream offset so stale entries fall outside `MAX_DISTANCE` and are rejected by the distance check.

Selection happens at the call site in `compress_into_sink_with_dict` in `crates/encode/src/compress.rs`. Both types implement the `HashTable` trait so the core loop is generic.

## 5-byte hash

On 64-bit targets, C lz4 hashes 4 input bytes with a 32-bit KNUTH multiplicative constant. lz4rip reads 5 bytes (via an 8-byte native-endian load, shifted) and hashes with a 64-bit PRIME5 constant. The extra byte reduces collisions across 2K-4K entry tables.

The PRIME5 constant is endianness-aware: different values for little-endian and big-endian targets in `crates/encode/src/hashtable.rs`, since the hash input comes from native-endian reads.

Hash shifts are derived from the table size: `>> (64 - ilog2(HASHTABLE_SIZE))`. `HashTableU32U16` uses `>> 52` (4K entries), `HashTableU32` uses `>> 53` (2K entries).

On 32-bit targets, both hash tables fall back to a 4-byte KNUTH multiplicative hash, matching C lz4's approach.

## Compile-time specialization

Two separate `#[inline(never)]` compression hot loops in `crates/encode/src/compress.rs`:

`compress_internal` handles single-table paths (free-function API and `CompressorRef` without dict). Generic over four axes:

| Parameter | Variants | Effect |
|---|---|---|
| `T: HashTable` | `HashTableU32U16`, `HashTableU32` | Table size and value width |
| `USE_DICT: bool` | true, false | Dictionary lookup code |
| `HAS_OFFSET: bool` | true, false | Offset arithmetic for dict positions |
| `S: Sink` | `SliceSink`, `VerifiedSliceSink` | Bounds-checked vs pre-verified writes |

When `USE_DICT=false`, all dictionary code is dead and eliminated by LLVM. When `HAS_OFFSET=false`, offset is a compile-time zero. LLVM specializes each call site independently without excessive code duplication.

`compress_with_dict_table` handles the dual-table dict path (`CompressorRef::with_dict`). Generic over `T: HashTable` and `S: Sink`. Takes both a cleared main table and a read-only pristine table, probing the pristine table on main-table miss.

`decompress_internal` in `crates/decode/src/decompress.rs` is generic over `USE_DICT: bool` and `S: Sink` (only `SliceSink` in practice). Fast path: unchecked reads via `primitives.rs` in the safe region. Slow path: bounds-checked near buffer end.

## Forward hashing

The match-search loop in `compress_internal` computes the hash of the next candidate position while checking the current one. This hides hash computation latency behind the branch misprediction penalty of the match check. The pattern is:

```
hash(pos+1) → check match at pos → if miss, use pre-computed hash
```

C lz4 uses the same technique. lz4_flex does not.

## Unsafe boundary

All compression and decompression logic is `#[forbid(unsafe_code)]`. Unsafe is isolated in four internal modules across two crates (16 blocks total):

- `crates/encode/src/hashtable.rs` (2 blocks): `count_same_bytes_unchecked`, `get_batch_unchecked`. Each has `debug_assert` guards on bounds.
- `crates/encode/src/verified_sink.rs` (2 blocks): `VerifiedSliceSink` performs unchecked writes after a one-time upfront capacity check at the compression entry point.
- `crates/encode/src/compressor.rs` (1 block): `from_raw_parts` extending the dict slice lifetime for the self-referential `Compressor` wrapper.
- `crates/decode/src/primitives.rs` (11 blocks): unchecked memory reads (`read_byte_unchecked`, `read_u16_unchecked`), wild copies (`wild_copy_16`, `wild_copy_literals`, `wild_copy_match_8`/`_16`/`_32`, `wild_match_copy_18`), `copy_within_nonoverlap`, `copy_within_overlapping`, `copy_from_src`. Each has `debug_assert` guards on bounds.

The safe-region margin computation in `decompress_internal` determines how far from buffer ends the fast path can operate. Inside the margin, unchecked reads and wild copies in `primitives.rs` are provably in-bounds. Outside it, the slow path uses `.get()` with explicit error returns.

## Dictionary compression

Dictionary initialization in `init_dict` hashes every 3rd byte of the dictionary, not every byte. This reduces setup cost while maintaining reasonable match coverage.

`Compressor::with_dict` hashes the dictionary once into a read-only `HashTableU32U16` (4 KB). Each `compress_into` call clears the 4 KB main table and probes the pristine table on miss. The `Compressor` is structured as a `Plain`/`Dict` enum so each variant holds only the tables it needs.

Dictionaries larger than 64 KB (`WINDOW_SIZE`) are trimmed to the last 64 KB.

## Crate split

The workspace has four crates: `lz4rip` (facade, re-exports + frame format), `lz4rip-core` (shared types: `Sink`, `SliceSink`, `fastcpy`, error types), `lz4rip-encode` (block compression), `lz4rip-decode` (block decompression).

Encoding and decoding are in separate crates so LLVM compiles them in separate codegen units. This eliminates a class of LTO-induced regressions where dead code in one path shifts register allocation in the other, causing measurable throughput changes (1.8x observed before the split). The facade crate re-exports the public API so downstream users see a single `lz4rip` dependency.

## Scope

LZ4-HC, LZ4-OPT, and LZ4-MID are permanently out of scope. These are higher-ratio, lower-throughput compression modes. Use zstd ([zrip](https://github.com/paddor/zrip)) when ratio matters.

lz4rip implements LZ4 block format and LZ4 frame format (streaming, behind `frame` feature flag). The block format is the performance-critical path.
