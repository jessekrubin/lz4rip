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

Generate all charts (pipeline, summary, dict2k, structured, sweep):
```sh
# pipeline + summary + dict2k
taskset -c 0 cargo run --release --example lz4rip_bench 2>/dev/null | \
  LZ4RIP_HW_EXTRAS="performance governor,turbo off" python3 benches/plot_bench.py /dev/stdin doc/charts/x86_64

# structured (no-dict and dict variants)
taskset -c 0 cargo run --release --example lz4rip_bench -- --structured 2>/dev/null | \
  LZ4RIP_HW_EXTRAS="performance governor,turbo off" python3 benches/plot_bench.py --structured /dev/stdin doc/charts/x86_64/structured
taskset -c 0 cargo run --release --example lz4rip_bench -- --structured-dict 2>/dev/null | \
  LZ4RIP_HW_EXTRAS="performance governor,turbo off" python3 benches/plot_bench.py --structured-dict /dev/stdin doc/charts/x86_64/structured

# sweep (slow, ~5 min)
# generate_sweep_chart aborts if LZ4RIP_HW_EXTRAS is unset and sysfs governor != performance
LZ4RIP_HW_EXTRAS="performance governor,turbo off" python3 -c \
  "import sys; sys.path.insert(0,'benches'); from pathlib import Path; import plot_bench; plot_bench.generate_sweep_chart(Path('doc/charts/x86_64'))"
```

Rerun only lz4rip (other implementations served from `~/.cache/lz4rip/`), then
regenerate charts from merged cache:
```sh
taskset -c 0 cargo run --release --example lz4rip_bench -- --impl lz4rip 2>/dev/null > /tmp/lz4rip_results.json
python3 - <<'EOF'
import json
from pathlib import Path
cache = Path.home() / '.cache/lz4rip'
results = []
for f in ['C_lz4.jsonl', 'lz4rip.jsonl', 'lz4_flex_unsafe.jsonl', 'lz4_flex.jsonl']:
    results += [json.loads(l) for l in (cache / f).read_text().splitlines() if l.strip()]
Path('/tmp/merged_results.json').write_text(json.dumps(results))
EOF
LZ4RIP_HW_EXTRAS="performance governor,turbo off" python3 benches/plot_bench.py /tmp/merged_results.json doc/charts/x86_64
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
