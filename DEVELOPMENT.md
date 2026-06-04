# Development

## Benchmarks

Set the CPU governor to `performance` before benchmarking to prevent frequency scaling from skewing results:

```sh
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

Pin to a single core with `taskset -c 0` to avoid cross-core migration noise.

Generate the pipeline chart:
```sh
taskset -c 0 cargo run --release --example lz4rip_bench 2>/dev/null > bench_results.json
LZ4RIP_HW_EXTRAS="performance governor,turbo off" python3 benches/plot_bench.py bench_results.json doc/charts
```

The chart subtitle shows CPU model, governor, and turbo state. On systems where
`/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor` and turbo sysfs entries
are not available, set `LZ4RIP_HW_EXTRAS` to supply those labels manually.

Rerun only lz4rip (other implementations are cached in `~/.cache/lz4rip/`):
```sh
taskset -c 0 cargo run --release --example lz4rip_bench -- --impl lz4rip 2>/dev/null > bench_results.json
```

## Miri

Checks unsafe code for undefined behavior (Stacked Borrows violations, use-after-free, etc.). Run on unit tests only; integration tests include 10 MB corpus data which is too slow under interpretation.

```sh
MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test --lib
```

`-Zmiri-disable-isolation` is needed because proptest calls `getcwd`.

## Fuzzing

Requires nightly:
```sh
cargo +nightly fuzz run fuzz_roundtrip
cargo +nightly fuzz run fuzz_roundtrip_frame
cargo +nightly fuzz run fuzz_decomp_corrupt_block
cargo +nightly fuzz run fuzz_decomp_corrupt_frame
cargo +nightly fuzz run fuzz_decomp_no_output_leak
cargo +nightly fuzz run fuzz_roundtrip_cpp_compress
```

## Feature flags

- `frame`: LZ4 frame format support. Implies `std`. Enabled by default.
- `std`: standard library dependency. Enabled by default.
- `nightly`: enables nightly-only `#[optimize(size)]` on cold paths. Not required for correctness or performance on stable.
