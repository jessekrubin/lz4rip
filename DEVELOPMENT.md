# Development

## Benchmarks

Set the CPU governor to `performance` before benchmarking to prevent frequency scaling from skewing results:

```sh
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

Pin to a single core with `taskset -c 0` to avoid cross-core migration noise.

All chart commands require `LZ4RIP_HW_EXTRAS` to inject the governor/turbo subtitle
on systems where sysfs is unavailable (or when the governor was not changed before
running). Always set it:

```sh
export LZ4RIP_HW_EXTRAS="performance governor,turbo off"
```

The bench writes results to `~/.cache/lz4rip/<arch>/` (and subdirs for sweep/structured).
Chart generation reads from cache. Two separate steps, no piping.

Bench all impls (including paranoid) and generate all charts:
```sh
taskset -c 0 cargo run --release --example lz4rip_bench && \
taskset -c 0 cargo run --release --example lz4rip_bench --features paranoid && \
taskset -c 0 cargo run --release --example lz4rip_bench -- --structured && \
taskset -c 0 cargo run --release --example lz4rip_bench -- --structured-dict && \
taskset -c 0 cargo run --release --example lz4rip_bench -- --sweep && \
LZ4RIP_HW_EXTRAS="performance governor,turbo off" python3 benches/plot_bench.py --all doc/charts/x86_64
```

Rerun only lz4rip (other impls served from cache), then regenerate charts:
```sh
taskset -c 0 cargo run --release --example lz4rip_bench -- --impl lz4rip && \
LZ4RIP_HW_EXTRAS="performance governor,turbo off" python3 benches/plot_bench.py --all doc/charts/x86_64
```

Clear a specific impl's cache to force re-bench:
```sh
rm ~/.cache/lz4rip/x86_64/lz4rip.jsonl
```

## Miri

Checks unsafe code for undefined behavior (Stacked Borrows violations, use-after-free, etc.).

```sh
# unit tests only (~2 min)
MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test --lib

# unit + integration tests (~20 min)
# C FFI tests (cpp_compat.rs) are excluded via #![cfg(not(miri))].
# Large corpus tests (dickens, hdfs, proptest) are excluded via #[cfg_attr(miri, ignore)].
MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test
```

`-Zmiri-disable-isolation` is needed because proptest calls `getcwd`.

## Releasing

`release-plz` runs on every push to `main`
(`.github/workflows/release-plz.yml`). It opens or updates a release PR,
creates annotated tags after merge, publishes to crates.io, and creates
GitHub releases. Configuration lives in `release-plz.toml`.

### Steps

1. **Review the release-plz PR.** Verify semver bumps.

2. **Curate changelogs.** For each bumped crate, insert a new
   `## [x.y.z]` section below `## [Unreleased]`. Never modify existing
   versioned sections.

3. **Merge the release PR.** release-plz tags and publishes to
   crates.io automatically.

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
