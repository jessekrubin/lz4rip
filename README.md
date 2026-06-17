# lz4rip

Fast, memory-safe LZ4 compression for Rust. On par with C lz4 throughput, with all algorithm logic under `#[forbid(unsafe_code)]`.

Originally derived from [lz4_flex](https://github.com/PSeitz/lz4_flex).

```toml
lz4rip = "0.5"
```

## Why lz4rip

- **C lz4 speed, safe by construction.** Unsafe is isolated to two files for raw memory ops. See [SAFETY.md](SAFETY.md).
- **8 KB hash tables.** Half the L1d footprint of C lz4 and lz4_flex.
- **Built-in dictionary training.** `DictTrainer` learns a dictionary from your data. No external tools needed.
- **Hot-loop friendly.** Epoch-based table reuse skips clearing between calls for small inputs.
- **`no_std` and no-alloc ready.** Block format works without `std` or even `alloc`. Hash tables live on the stack when `alloc` is off (~8 KB per call). Frame format requires `std`.

See [DESIGN.md](DESIGN.md) for how it all works.

## Performance

![LZ4 Pipeline Summary](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/summary.svg)

<details>
<summary>x86_64 details (per-file, size sweep, dictionary)</summary>

![LZ4 Pipeline Detail](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/pipeline.svg)
![LZ4 Size Sweep](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/sweep.svg)
![LZ4 Dict 2K](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/dict2k.svg)
</details>

<details>
<summary>x86_64 structured data (JSON/XML, with and without dictionary)</summary>

![LZ4 Structured Dict 2K](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/structured/dict2k.svg)
![LZ4 Structured No Dict](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/x86_64/structured/no_dict.svg)
</details>

<details>
<summary>aarch64 (Apple M4)</summary>

![LZ4 Pipeline Summary](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/aarch64/summary.svg)
![LZ4 Pipeline Detail](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/aarch64/pipeline.svg)
![LZ4 Dict 2K](https://raw.githubusercontent.com/paddor/lz4rip/main/doc/charts/aarch64/dict2k.svg)
</details>

## Block format

```rust
use lz4rip::block::{compress, decompress_into};

let input = b"Hello people, what's up?";
let compressed = compress(input);

let mut output = vec![0u8; input.len()];
let n = decompress_into(&compressed, &mut output).unwrap();
assert_eq!(&output[..n], input);
```

The `_into` variants write into a caller-provided buffer. The plain variants
allocate and require the `alloc` feature.

### No-alloc / embedded

All `_into` functions and the `Compressor`/`Decompressor` structs work without
`alloc`. Hash tables are stack-allocated (~8 KB per compress call).

```toml
[dependencies]
lz4rip = { version = "0.5", default-features = false }
```

```rust
use lz4rip::block::{compress_into, decompress_into, get_maximum_output_size};

let input = b"Hello people, what's up?";
let mut compressed = [0u8; 256];
let n = compress_into(input, &mut compressed).unwrap();

let mut output = [0u8; 64];
let m = decompress_into(&compressed[..n], &mut output).unwrap();
assert_eq!(&output[..m], input);
```

One-shot dictionary compression is also available without `alloc`:

```rust
use lz4rip::block::{compress_into_with_dict, decompress_into_with_dict, get_maximum_output_size};

let dict = b"shared context bytes...";
let input = b"context bytes appear in messages";
let mut compressed = [0u8; 256];
let n = compress_into_with_dict(input, &mut compressed, dict).unwrap();

let mut output = [0u8; 64];
let m = decompress_into_with_dict(&compressed[..n], &mut output, dict).unwrap();
assert_eq!(&output[..m], input);
```

### Dictionary compression

Pre-seed the compressor and decompressor with shared context for better ratios
on small messages. `Compressor` borrows the dictionary (no heap copy).

```rust
use lz4rip::block::{Compressor, Decompressor, get_maximum_output_size};

let dict = b"shared context bytes...";
let mut comp = Compressor::with_dict(dict);
let decomp = Decompressor::new(dict);

let input = b"context bytes appear in messages";
let mut buf = vec![0u8; get_maximum_output_size(input.len())];
let n = comp.compress_into(input, &mut buf).unwrap();

let output = decomp.decompress(&buf[..n], input.len()).unwrap();
assert_eq!(&output[..], input);
```

### Dictionary training

Build a dictionary from sample data using the built-in COVER trainer.
Requires the `alloc` feature.

```rust
use lz4rip::block::DictTrainer;

let mut trainer = DictTrainer::new(2048); // max dict size in bytes
for sample in &samples {
    trainer.add_sample(sample);
}
let dict = trainer.train();
// Use with Compressor::with_dict(&dict) / Decompressor::new(&dict)
```

## Frame format

The frame format (feature `frame`, on by default) wraps block compression in the
standard LZ4 frame container with checksums, content size, and streaming support.

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

## Safety

[SAFETY.md](SAFETY.md) documents the unsafe boundary and catalogs C lz4 memory safety bugs that Rust prevents by construction.

## Development

[DEVELOPMENT.md](DEVELOPMENT.md) covers benchmarking, fuzzing, and feature flags.
