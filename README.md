# lz4rip

Rust LZ4 compression. 5-8% faster than C lz4 end-to-end (compress + 1 GB/s transfer + decompress, geomean across 16 corpus files).
Originally derived from [lz4_flex](https://github.com/PSeitz/lz4_flex).

```toml
lz4rip = "0.2"
```

## Performance

![LZ4 Pipeline Summary](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/summary.svg)

<details>
<summary>x86_64 details (per-file, size sweep, dictionary)</summary>

![LZ4 Pipeline Detail](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/pipeline.svg)
![LZ4 Size Sweep](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/sweep.svg)
![LZ4 Dict 2K](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/dict2k.svg)
</details>

<details>
<summary>aarch64 (Apple M4)</summary>

![LZ4 Pipeline Summary](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/aarch64/summary.svg)
![LZ4 Pipeline Detail](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/aarch64/pipeline.svg)
![LZ4 Dict 2K](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/aarch64/dict2k.svg)
</details>

## Block format

```rust
use lz4rip::block::{compress, decompress_into, get_maximum_output_size};

let input = b"Hello people, what's up?";
let compressed = compress(input);

let mut output = vec![0u8; input.len()];
let n = decompress_into(&compressed, &mut output).unwrap();
assert_eq!(&output[..n], input);
```

The `_into` variants write into a caller-provided buffer. The plain variants
allocate. `compress_prepend_size` / `decompress_size_prepended` frame the
compressed payload with a little-endian u32 length prefix for self-describing
messages.

```rust
// One-shot (allocating)
fn compress(input: &[u8]) -> Vec<u8>;
fn decompress(input: &[u8], uncompressed_size: usize) -> Result<Vec<u8>>;

// One-shot (into caller buffer)
fn compress_into(input: &[u8], output: &mut [u8]) -> Result<usize>;
fn decompress_into(input: &[u8], output: &mut [u8]) -> Result<usize>;

// Size-prepended convenience
fn compress_prepend_size(input: &[u8]) -> Vec<u8>;
fn decompress_size_prepended(input: &[u8]) -> Result<Vec<u8>>;
```

### Dictionary compression

Pre-seed the compressor and decompressor with shared context for better ratios
on small messages (e.g. JSON records, log lines).

```rust
use lz4rip::block::{Compressor, Decompressor, get_maximum_output_size};

let dict = b"shared context bytes...";
let mut comp = Compressor::with_dict(dict);
let decomp = Decompressor::with_dict(dict);

let input = b"context bytes appear in messages";
let mut buf = vec![0u8; get_maximum_output_size(input.len())];
let n = comp.compress_into(input, &mut buf).unwrap();

let output = decomp.decompress(&buf[..n], input.len()).unwrap();
assert_eq!(&output[..], input);
```

## Frame format

The frame format (feature `frame`, on by default) wraps block compression in the
standard LZ4 frame container with checksums, content size, and streaming support.
`FrameEncoder` and `FrameDecoder` implement `Write` and `Read`.

```rust
use lz4rip::frame::{FrameEncoder, FrameDecoder};
use std::io::{Write, Read};

// Compress
// FrameEncoder::with_dictionary(wtr, dict, dict_id) for dictionary support
let mut encoder = FrameEncoder::new(Vec::new());
encoder.write_all(b"Hello frame format!").unwrap();
let compressed = encoder.finish().unwrap();

// Decompress
let mut decoder = FrameDecoder::new(&compressed[..]);
let mut output = String::new();
decoder.read_to_string(&mut output).unwrap();
assert_eq!(output, "Hello frame format!");
```

## Design

Divergences from C lz4 and lz4_flex that explain the performance difference. See [DESIGN.md](DESIGN.md) for details.

- Aggressive skip acceleration (8 vs C lz4's 64 misses before stepping)
- Generational hash table (16-bit for small inputs, 32-bit for large)
- 5-byte PRIME5 hash (vs C lz4's 4-byte KNUTH)
- Compile-time specialization over hash table type, dict mode, and sink type
- No HC/OPT/MID. Use zstd for ratio.

## Safety

All compression and decompression logic is `#[forbid(unsafe_code)]`. See
[SAFETY.md](SAFETY.md) for the unsafe boundary details and a catalog of C lz4
memory safety bugs that Rust prevents by construction.

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for benchmarking, fuzzing, and feature flag details.
