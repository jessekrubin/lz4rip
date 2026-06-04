# lz4rip

Rust LZ4 compression. Originally derived from [lz4_flex](https://github.com/PSeitz/lz4_flex).

All compression and decompression logic is `#[forbid(unsafe_code)]`. The remaining
unsafe (19 blocks in 2 internal modules) performs unchecked memory copies whose
bounds are proven by safe-region margins computed in the algorithm code. No `unsafe`
is exposed in the public API.

```toml
lz4rip = "0.1"
```

## Block format

```rust
use lz4rip::block::{compress_prepend_size, decompress_size_prepended};

let input: &[u8] = b"Hello people, what's up?";
let compressed = compress_prepend_size(input);
let uncompressed = decompress_size_prepended(&compressed).unwrap();
assert_eq!(input, uncompressed);
```

See [`examples/quickstart.rs`](examples/quickstart.rs) for dictionary compression and
frame format (streaming) examples.

## Performance

![LZ4 Pipeline Summary](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/summary.svg)

<details>
<summary>Per-file breakdown (16 Silesia corpus files)</summary>

![LZ4 Pipeline Detail](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/pipeline.svg)
</details>

<details>
<summary>Dictionary compression (1 KB JSON, 2 KB dict)</summary>

![LZ4 Dict 2K](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/dict2k.svg)
</details>

<details>
<summary>Apple M4 (aarch64)</summary>

![LZ4 Pipeline Summary](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/aarch64/summary.svg)
![LZ4 Pipeline Detail](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/aarch64/pipeline.svg)
</details>

## Design

Divergences from C lz4 and lz4_flex that explain the performance difference. See [DESIGN.md](DESIGN.md) for details.

- Aggressive skip acceleration (8 vs C lz4's 64 misses before stepping)
- Generational hash table (16-bit for small inputs, 32-bit for large)
- 5-byte PRIME5 hash (vs C lz4's 4-byte KNUTH)
- Compile-time specialization over hash table type, dict mode, and sink type
- No HC/OPT/MID. Use zstd for ratio.

## Why Rust matters here

C lz4's "safe" decompression API (`LZ4_decompress_safe` and variants) has had
memory safety bugs that Rust's type system and bounds checking prevent by
construction:

| CVE / Fix | Bug | Rust prevents because |
|-----------|-----|----------------------|
| [CVE-2021-3520](https://nvd.nist.gov/vuln/detail/CVE-2021-3520) (CVSS 9.8) | Negative `outputSize` wraps to huge `size_t`, heap overflow in `LZ4_decompress_safe_partial` | `usize` is unsigned, slice lengths can't be negative |
| [CVE-2014-4715](https://nvd.nist.gov/vuln/detail/CVE-2014-4715) | Integer overflow in literal-run accumulator on 32-bit causes memory corruption | Slice indexing panics on OOB regardless of overflow |
| [CVE-2022-49078](https://nvd.nist.gov/vuln/detail/CVE-2022-49078) | `LZ4_decompress_safe_partial` OOB read on corrupted data (Linux kernel's embedded copy) | Slice indexing panics on OOB |
| [PR #1753](https://github.com/lz4/lz4/pull/1753) (2026) | OOB read in `read_variable_length()`: byte read before bounds check | `.get()` checks before read |
| [PR #1733](https://github.com/lz4/lz4/pull/1733) | Negative `dictSize` cast to `size_t` bypasses offset validation in `LZ4_decompress_safe_usingDict` | `usize` can't be negative |
| [PR #1737](https://github.com/lz4/lz4/pull/1737) | `read_variable_length` overflow undetected on 64-bit (guard was `sizeof(length) < 8`) | OOB access after overflow caught by bounds check |
| [PR #1737](https://github.com/lz4/lz4/pull/1737) | Match-length overflow check misordered: `MINMATCH` added after the wraparound test | Slice index from wrapped value caught by bounds check |
| [PR #1737](https://github.com/lz4/lz4/pull/1737) | `frameRemainingSize` unsigned underflow wraps to huge value in `LZ4F_decompress` | Debug panic on underflow; OOB caught by bounds check in release |
| [e72d4423](https://github.com/lz4/lz4/commit/e72d44230093) | Fast decode loop skips dictionary bounds check, reads up to 64 KB before output buffer | `copy_within` bounded by slice length |
| [725cb0aa](https://github.com/lz4/lz4/commit/725cb0aafdf7) | `LZ4_decompress_safe_partial` reads past end of input buffer | Slice indexing panics on OOB |
| [#929](https://github.com/lz4/lz4/issues/929) | Negative input size causes memory access violation in `LZ4_decompress_safe_partial` | `usize` can't be negative |
| [#792](https://github.com/lz4/lz4/issues/792) | `read_variable_length` accumulator width is platform-dependent (`unsigned`), overflows on 16-bit | Explicit `usize` type, no platform-dependent width surprise |
| [dfc431fb](https://github.com/lz4/lz4/commit/dfc431fb3d03) | NULL pointer arithmetic UB in dictionary setup (e.g. `NULL + 0`) | No null references in Rust |
| [539c783c](https://github.com/lz4/lz4/commit/539c783c98f1) | NULL `dictBase` pointer arithmetic UB when no dictionary is provided | No null references in Rust |

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for benchmarking, fuzzing, and feature flag details.
