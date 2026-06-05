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

## Generational hash table

Two hash table implementations selected by input size:

| Table | Entries | Value width | Footprint | Used when |
|---|---|---|---|---|
| `HashTable4KU16` | 8K | 16-bit | 16 KB | dict + input < 64 KB |
| `HashTable4K` | 4K | 32-bit | 16 KB | larger inputs |

The 16-bit table fits in L1 cache and avoids 32-bit position tracking overhead for short messages. C lz4 uses a single 16 KB table (`LZ4_hash4`) for all input sizes.

Selection happens at the call site in `compress_into_with_dict` in `compress.rs`. Both types implement the `HashTable` trait so the core loop is generic.

## 5-byte hash

C lz4 hashes 4 input bytes with a 32-bit KNUTH multiplicative constant. lz4rip reads 5 bytes (via an 8-byte native-endian load, shifted) and hashes with a 64-bit PRIME5 constant. The extra byte reduces collisions across 4K-8K entry tables.

The PRIME5 constant is endianness-aware: different values for little-endian and big-endian targets in `hashtable.rs`, since the hash input comes from native-endian reads.

The `HashTable4KU16` type uses PRIME5 >> 51 to index 8K entries. `HashTable4K` uses PRIME5 >> 52 for 4K entries.

## Compile-time specialization

`compress_internal` in `compress.rs` is generic over four axes:

| Parameter | Variants | Effect |
|---|---|---|
| `T: HashTable` | `HashTable4KU16`, `HashTable4K` | Table size and value width |
| `USE_DICT: bool` | true, false | Dictionary lookup code |
| `HAS_OFFSET: bool` | true, false | Offset arithmetic for dict positions |
| `S: Sink` | `SliceSink`, `VerifiedSliceSink` | Bounds-checked vs pre-verified writes |

When `USE_DICT=false`, all dictionary code is dead and eliminated by LLVM. When `HAS_OFFSET=false`, offset is a compile-time zero. The function is `#[inline(never)]` so LLVM specializes each call site independently without excessive code duplication.

`decompress_internal` in `decompress.rs` is similarly generic over `USE_DICT` and sink type, with a fast path (unchecked reads in safe region) and slow path (bounds-checked near buffer end).

## Forward hashing

The match-search loop in `compress_internal` computes the hash of the next candidate position while checking the current one. This hides hash computation latency behind the branch misprediction penalty of the match check. The pattern is:

```
hash(pos+1) → check match at pos → if miss, use pre-computed hash
```

C lz4 uses the same technique. lz4_flex does not.

## Unsafe boundary

All compression and decompression logic is `#[forbid(unsafe_code)]`. Unsafe is isolated in two internal modules:

- `hashtable.rs`: unchecked memory reads (`read_u16_unchecked`, `read_u32_unchecked`, `read_byte_unchecked`), wild copies (`wild_copy_16`, `wild_copy_literals`, `wild_copy_match_8`/`_16`/`_32`, `wild_match_copy_18`), `copy_within_nonoverlap`, `count_same_bytes`. Each has `debug_assert` guards on bounds.
- `verified_sink.rs`: `VerifiedSliceSink` performs unchecked writes after a one-time upfront capacity check at the compression entry point.

The safe-region margin computation in `decompress_internal` determines how far from buffer ends the fast path can operate. Inside the margin, unchecked reads and wild copies are provably in-bounds. Outside it, the slow path uses `.get()` with explicit error returns.

## Dictionary compression

Dictionary initialization in `compress_into_with_dict` hashes every 3rd byte of the dictionary, not every byte. This reduces setup cost while maintaining reasonable match coverage.

`Compressor` caches the pre-hashed dictionary table and restores it via memcpy before each `compress_into` call. 16 KB memcpy per call, but avoids re-hashing.

Dictionaries larger than 64 KB (`WINDOW_SIZE`) are trimmed to the last 64 KB.

## Scope

LZ4-HC, LZ4-OPT, and LZ4-MID are permanently out of scope. These are higher-ratio, lower-throughput compression modes. Use zstd when ratio matters.

lz4rip implements LZ4 block format and LZ4 frame format (streaming, behind `frame` feature flag). The block format is the performance-critical path.
