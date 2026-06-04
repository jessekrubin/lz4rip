#!/usr/bin/env python3
"""Generate benchmark pipeline SVG from JSON (array or JSONL).

Usage:
    taskset -c 0 cargo run --release --example lz4rip_bench 2>/dev/null \
        | python3 benches/plot_bench.py /dev/stdin doc/charts
"""

import json
import os
import random
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


CODEC_ORDER = ["C lz4", "lz4rip", "lz4_flex"]

COLORS = {
    "C lz4":             ("#60a5fa", "#4680c4"),   # blue
    "lz4rip":            ("#f87171", "#c45050"),   # red
    "lz4_flex":          ("#4ade80", "#3aaf60"),   # green
}

LABELS = {
    "C lz4":             "lz4 (C)",
    "lz4rip":            "lz4rip (safe Rust)",
    "lz4_flex":          "lz4_flex (safe Rust)",
}

DICT_CODEC_ORDER = ["C lz4 (dict 2K)", "lz4rip (dict 2K)"]

DICT_COLORS = {
    "C lz4 (dict 2K)":   ("#60a5fa", "#4680c4"),   # blue
    "lz4rip (dict 2K)":  ("#f87171", "#c45050"),   # red
}

DICT_LABELS = {
    "C lz4 (dict 2K)":   "lz4 (C)",
    "lz4rip (dict 2K)":  "lz4rip (safe Rust)",
}


def human_size(n):
    if n >= 1_000_000:
        return f"{n / 1_000_000:.1f} MB"
    if n >= 1_000:
        return f"{n / 1_000:.0f} KB"
    return f"{n} B"


def load_results(path):
    text = Path(path).read_text()
    text = text.strip()
    if text.startswith("["):
        return json.loads(text)
    return [json.loads(line) for line in text.splitlines() if line.strip()]


def get_inputs(results):
    seen = set()
    inputs = []
    for r in results:
        if r["input"] not in seen:
            inputs.append(r["input"])
            seen.add(r["input"])
    return inputs


def nice_step(max_val, target_lines):
    raw = max_val / target_lines
    mag = 10 ** int(f"{raw:.0e}".split("e")[1])
    for s in [1, 2, 5, 10]:
        step = s * mag
        if max_val / step <= target_lines + 1:
            return step
    return mag * 10


def detect_hardware():
    try:
        cpu = os.environ.get("LZ4RIP_CPU")
        if not cpu:
            for line in open("/proc/cpuinfo"):
                if line.startswith("model name"):
                    cpu = line.split(":", 1)[1].strip()
                    cpu = cpu.replace("(R)", "").replace("(TM)", "").replace("CPU ", "")
                    break
        if cpu:
            label = cpu
            extras = []
            try:
                gov = open("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").read().strip()
                if gov == "performance":
                    extras.append("performance governor")
            except OSError:
                pass
            for path, off_val in [
                ("/sys/devices/system/cpu/intel_pstate/no_turbo", "1"),
                ("/sys/devices/system/cpu/cpufreq/boost", "0"),
            ]:
                try:
                    if open(path).read().strip() == off_val:
                        extras.append("turbo off")
                    break
                except OSError:
                    continue
            if not extras:
                hw_extras = os.environ.get("LZ4RIP_HW_EXTRAS")
                if hw_extras:
                    extras.extend(hw_extras.split(","))
            if extras:
                label += ", " + ", ".join(extras)
            return label
    except OSError:
        pass
    return None


def pipeline_chart(results, out_dir):
    inputs = get_inputs(results)
    codecs = [c for c in CODEC_ORDER if any(r["codec"] == c for r in results)]
    n_codecs = len(codecs)

    # split inputs into two panels
    mid = (len(inputs) + 1) // 2
    panels = [inputs[:mid], inputs[mid:]]

    hw_label = detect_hardware()

    svg_w = 850
    x_left, x_right = 55, 830
    plot_w = x_right - x_left
    panel_h = 240
    panel_gap = 70
    top_margin = 50 if hw_label else 40

    panel_tops = [
        top_margin,
        top_margin + panel_h + panel_gap,
    ]
    svg_h = panel_tops[-1] + panel_h + 110

    # compute stacked values: compress + transfer + decompress (seconds per GB)
    # transfer rate: 1000 Mbit/s = 125 MB/s (same assumption as lz4.org)
    transfer_rate = 1e9  # bytes per second (1 GB/s)
    stacks = {}
    y_max = 0
    for inp in inputs:
        for codec in codecs:
            r = next((r for r in results if r["input"] == inp and r["codec"] == codec), None)
            if not r:
                continue
            per_gb = 1e9 / r["input_size"]
            comp = r["compress_ns"] / 1e9 * per_gb
            transfer = (r["compressed_size"] / r["input_size"]) * (1e9 / transfer_rate)
            decomp = r["decompress_ns"] / 1e9 * per_gb
            stacks[(inp, codec)] = (comp, transfer, decomp)
            y_max = max(y_max, comp + transfer + decomp)

    y_max *= 1.1

    mid_x = (x_left + x_right) / 2
    L = []
    L.append(
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {svg_w} {svg_h}"'
        f' font-family="system-ui, -apple-system, sans-serif">'
    )
    L.append(f'  <rect width="{svg_w}" height="{svg_h}" fill="#0d1117"/>')

    # title
    L.append(
        f'  <text x="{mid_x}" y="22" text-anchor="middle" fill="#e6edf3"'
        f' font-size="14" font-weight="700">'
        f'LZ4 Block: Compress + Transfer @1 GB/s + Decompress (lower is better)'
        f'</text>'
    )
    if hw_label:
        L.append(
            f'  <text x="{mid_x}" y="38" text-anchor="middle" fill="#7d8590"'
            f' font-size="10">{hw_label}</text>'
        )

    for pi, panel_inputs in enumerate(panels):
        n_inputs = len(panel_inputs)
        p_top = panel_tops[pi]
        p_bot = p_top + panel_h

        group_w = plot_w / n_inputs
        bar_w = group_w * 0.75 / n_codecs
        gap = group_w * 0.25

        def y(v, _bot=p_bot, _top=p_top):
            return _bot - (v / y_max) * (_bot - _top)

        # y gridlines + labels
        step = nice_step(y_max, 5)
        v = step
        while v <= y_max:
            yy = y(v)
            L.append(
                f'  <line x1="{x_left}" y1="{yy:.1f}" x2="{x_right}" y2="{yy:.1f}"'
                f' stroke="#21262d" stroke-width="1"/>'
            )
            L.append(
                f'  <text x="{x_left - 8}" y="{yy:.1f}" text-anchor="end"'
                f' dominant-baseline="middle" fill="#7d8590" font-size="10">{v:.0f}</text>'
            )
            v += step

        # baseline
        L.append(
            f'  <line x1="{x_left}" y1="{p_bot}" x2="{x_right}" y2="{p_bot}"'
            f' stroke="#30363d" stroke-width="1.5"/>'
        )

        # y-axis label (only on first panel)
        if pi == 0:
            total_mid_y = (panel_tops[0] + panel_tops[1] + panel_h) / 2
            L.append(
                f'  <text x="22" y="{total_mid_y}" text-anchor="middle" fill="#e6edf3"'
                f' font-size="11" font-weight="600"'
                f' transform="rotate(-90,22,{total_mid_y})">seconds / GB</text>'
            )

        # bars
        for gi, inp in enumerate(panel_inputs):
            group_x = x_left + gi * group_w + gap / 2

            for ci, codec in enumerate(codecs):
                if (inp, codec) not in stacks:
                    continue
                comp, transfer, decomp = stacks[(inp, codec)]
                main_c, xfer_c = COLORS[codec]

                bx = group_x + ci * bar_w
                h_comp = (comp / y_max) * (p_bot - p_top)
                L.append(
                    f'  <rect x="{bx:.1f}" y="{y(comp):.1f}"'
                    f' width="{bar_w:.1f}" height="{h_comp:.1f}"'
                    f' fill="{main_c}" rx="1"/>'
                )
                h_transfer = (transfer / y_max) * (p_bot - p_top)
                L.append(
                    f'  <rect x="{bx:.1f}" y="{y(comp + transfer):.1f}"'
                    f' width="{bar_w:.1f}" height="{h_transfer:.1f}"'
                    f' fill="{xfer_c}" rx="1"/>'
                )
                h_decomp = (decomp / y_max) * (p_bot - p_top)
                L.append(
                    f'  <rect x="{bx:.1f}" y="{y(comp + transfer + decomp):.1f}"'
                    f' width="{bar_w:.1f}" height="{h_decomp:.1f}"'
                    f' fill="{main_c}" rx="1"/>'
                )

            # group label
            r0 = next(r for r in results if r["input"] == inp)
            label = inp.replace(".txt", "").replace("compression_", "")
            size_label = human_size(r0["input_size"])
            cx = group_x + (n_codecs * bar_w) / 2
            L.append(
                f'  <text x="{cx:.1f}" y="{p_bot + 16}" text-anchor="middle"'
                f' fill="#e6edf3" font-size="10" font-weight="600">{label}</text>'
            )
            L.append(
                f'  <text x="{cx:.1f}" y="{p_bot + 28}" text-anchor="middle"'
                f' fill="#7d8590" font-size="9">{size_label}</text>'
            )

    # legend: codec colors
    leg_y = panel_tops[-1] + panel_h + 50
    legend_items = [(k, LABELS[k]) for k in codecs if k in COLORS]
    col_w = 180
    row_h = 20
    for i, (key, label) in enumerate(legend_items):
        col = i % 2
        row = i // 2
        lx = mid_x - col_w + col * col_w
        ly = leg_y + row * row_h
        main_c, xfer_c = COLORS[key]
        L.append(
            f'  <rect x="{lx:.0f}" y="{ly - 5}" width="12" height="12"'
            f' fill="{main_c}" rx="2"/>'
        )
        L.append(
            f'  <text x="{lx + 18:.0f}" y="{ly + 5}" fill="#e6edf3"'
            f' font-size="11" font-weight="500">{label}</text>'
        )

    # legend: stack meaning (bright = compress/decompress, dim = transfer)
    stack_y = leg_y + row_h * 2 + 4
    stack_items = [
        ("compress + decompress", "#e6edf3"),
        ("transfer (ratio)", "#7d8590"),
    ]
    stack_total = 320
    stack_start = mid_x - stack_total / 2

    for i, (label, fill) in enumerate(stack_items):
        sx = stack_start + i * 200
        shades = ["#60a5fa", "#4680c4"]
        L.append(
            f'  <rect x="{sx:.0f}" y="{stack_y - 5}" width="12" height="12"'
            f' fill="{shades[i]}" rx="2"/>'
        )
        L.append(
            f'  <text x="{sx + 18:.0f}" y="{stack_y + 5}" fill="#7d8590"'
            f' font-size="10">{label}</text>'
        )

    L.append("</svg>")
    return "\n".join(L) + "\n"


COMPRESSIBLE = {
    "dickens.txt", "hdfs.json", "nci", "xml_collection.xml", "webster",
    "samba", "reymont.pdf", "mozilla", "compression_34k.txt",
    "compression_65k.txt", "compression_66k_JSON.txt", "osdb",
}
INCOMPRESSIBLE = {"sao", "x-ray", "mr", "compression_1k.txt"}


def summary_chart(results, out_dir):
    codecs = [c for c in CODEC_ORDER if any(r["codec"] == c for r in results)]
    n_codecs = len(codecs)
    hw_label = detect_hardware()

    transfer_rate = 1e9  # bytes per second (1 GB/s)

    groups = [
        ("Compressible", COMPRESSIBLE),
        ("Incompressible", INCOMPRESSIBLE),
    ]

    # aggregate pipeline time per codec per group (sum bytes, sum ns, derive s/GB)
    group_data = {}
    for group_name, file_set in groups:
        for codec in codecs:
            total_input = 0
            total_compressed = 0
            total_compress_ns = 0
            total_decompress_ns = 0
            for r in results:
                if r["codec"] != codec or r["input"] not in file_set:
                    continue
                total_input += r["input_size"]
                total_compressed += r["compressed_size"]
                total_compress_ns += r["compress_ns"]
                total_decompress_ns += r["decompress_ns"]
            if total_input > 0:
                per_gb = 1e9 / total_input
                comp = total_compress_ns / 1e9 * per_gb
                transfer = (total_compressed / total_input) * (1e9 / transfer_rate)
                decomp = total_decompress_ns / 1e9 * per_gb
                group_data[(group_name, codec)] = (comp, transfer, decomp)

    n_groups = len(groups)
    svg_w = 600
    svg_h = 420
    x_left, x_right = 70, 570
    plot_w = x_right - x_left
    y_top = 55 if hw_label else 45
    y_bot = 310
    plot_h = y_bot - y_top

    y_max = 0
    for v in group_data.values():
        y_max = max(y_max, sum(v))
    y_max *= 1.15

    def y(v):
        return y_bot - (v / y_max) * plot_h

    mid_x = (x_left + x_right) / 2
    L = []
    L.append(
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {svg_w} {svg_h}"'
        f' font-family="system-ui, -apple-system, sans-serif">'
    )
    L.append(f'  <rect width="{svg_w}" height="{svg_h}" fill="#0d1117"/>')

    L.append(
        f'  <text x="{mid_x}" y="22" text-anchor="middle" fill="#e6edf3"'
        f' font-size="14" font-weight="700">'
        f'LZ4 Pipeline @1 GB/s: Aggregate across corpus (lower is better)'
        f'</text>'
    )
    if hw_label:
        L.append(
            f'  <text x="{mid_x}" y="38" text-anchor="middle" fill="#7d8590"'
            f' font-size="10">{hw_label}</text>'
        )

    # y gridlines
    step = nice_step(y_max, 5)
    v = step
    while v <= y_max:
        yy = y(v)
        L.append(
            f'  <line x1="{x_left}" y1="{yy:.1f}" x2="{x_right}" y2="{yy:.1f}"'
            f' stroke="#21262d" stroke-width="1"/>'
        )
        L.append(
            f'  <text x="{x_left - 8}" y="{yy:.1f}" text-anchor="end"'
            f' dominant-baseline="middle" fill="#7d8590" font-size="10">{v:.0f}</text>'
        )
        v += step

    # baseline
    L.append(
        f'  <line x1="{x_left}" y1="{y_bot}" x2="{x_right}" y2="{y_bot}"'
        f' stroke="#30363d" stroke-width="1.5"/>'
    )

    # y-axis label
    mid_y = (y_top + y_bot) / 2
    L.append(
        f'  <text x="22" y="{mid_y}" text-anchor="middle" fill="#e6edf3"'
        f' font-size="11" font-weight="600"'
        f' transform="rotate(-90,22,{mid_y})">seconds / GB</text>'
    )

    # bars: 2 groups, n_codecs bars each, with gap between groups
    group_w = plot_w / n_groups
    bar_w = group_w * 0.7 / n_codecs
    inner_gap = group_w * 0.1
    group_gap = group_w * 0.2

    for gi, (group_name, _) in enumerate(groups):
        group_x = x_left + gi * group_w + group_gap / 2

        for ci, codec in enumerate(codecs):
            if (group_name, codec) not in group_data:
                continue
            comp, transfer, decomp = group_data[(group_name, codec)]
            main_c, xfer_c = COLORS[codec]

            bx = group_x + ci * (bar_w + inner_gap / n_codecs)
            h_comp = (comp / y_max) * plot_h
            L.append(
                f'  <rect x="{bx:.1f}" y="{y(comp):.1f}"'
                f' width="{bar_w:.1f}" height="{h_comp:.1f}"'
                f' fill="{main_c}" rx="1"/>'
            )
            h_transfer = (transfer / y_max) * plot_h
            L.append(
                f'  <rect x="{bx:.1f}" y="{y(comp + transfer):.1f}"'
                f' width="{bar_w:.1f}" height="{h_transfer:.1f}"'
                f' fill="{xfer_c}" rx="1"/>'
            )
            h_decomp = (decomp / y_max) * plot_h
            L.append(
                f'  <rect x="{bx:.1f}" y="{y(comp + transfer + decomp):.1f}"'
                f' width="{bar_w:.1f}" height="{h_decomp:.1f}"'
                f' fill="{main_c}" rx="1"/>'
            )

        # group label
        cx = group_x + (n_codecs * (bar_w + inner_gap / n_codecs)) / 2
        L.append(
            f'  <text x="{cx:.1f}" y="{y_bot + 18}" text-anchor="middle"'
            f' fill="#e6edf3" font-size="11" font-weight="600">{group_name}</text>'
        )

    # legend: codec colors (two rows of two)
    leg_y = y_bot + 40
    legend_items = [(k, LABELS[k]) for k in codecs if k in COLORS]
    col_w = 180
    row_h = 20
    for i, (key, label) in enumerate(legend_items):
        col = i % 2
        row = i // 2
        lx = mid_x - col_w + col * col_w
        ly = leg_y + row * row_h
        main_c, xfer_c = COLORS[key]
        L.append(
            f'  <rect x="{lx:.0f}" y="{ly - 5}" width="12" height="12"'
            f' fill="{main_c}" rx="2"/>'
        )
        L.append(
            f'  <text x="{lx + 18:.0f}" y="{ly + 5}" fill="#e6edf3"'
            f' font-size="11" font-weight="500">{label}</text>'
        )

    # stack legend
    stack_y = leg_y + row_h * 2 + 4
    stack_items = [
        ("compress + decompress", "#60a5fa"),
        ("transfer @1 GB/s", "#4680c4"),
    ]
    stack_total = 320
    stack_start = mid_x - stack_total / 2
    for i, (label, swatch) in enumerate(stack_items):
        sx = stack_start + i * 200
        L.append(
            f'  <rect x="{sx:.0f}" y="{stack_y - 5}" width="12" height="12"'
            f' fill="{swatch}" rx="2"/>'
        )
        L.append(
            f'  <text x="{sx + 18:.0f}" y="{stack_y + 5}" fill="#7d8590"'
            f' font-size="10">{label}</text>'
        )

    L.append("</svg>")
    return "\n".join(L) + "\n"


def json_payload(target_bytes, counter_start=0):
    """Generate deterministic NDJSON log data, same pattern as OMQ.rs benchmarks."""
    LEVELS = ["DEBUG", "INFO", "WARN", "ERROR"]
    SERVICES = ["api-gateway", "auth-svc", "order-svc", "payment-svc", "notify-svc"]
    METHODS = ["GET", "POST", "PUT", "DELETE", "PATCH"]
    PATHS = ["/v1/widgets", "/v1/users", "/v1/orders", "/v2/events", "/v1/health"]
    REGIONS = ["us-east-1", "us-west-2", "eu-west-1", "ap-south-1", "eu-central-1"]
    STATUSES = [200, 201, 204, 400, 404, 500, 502, 503]
    MSGS = [
        "request handled successfully",
        "resource created",
        "cache miss, fetched from origin",
        "rate limit approaching threshold",
        "upstream timeout, retrying",
    ]
    out = []
    counter = counter_start
    total = 0
    while total < target_bytes:
        h = (counter * 0x9E3779B1) & 0xFFFFFFFF
        hid = f"{h:08x}"
        level = LEVELS[h % len(LEVELS)]
        service = SERVICES[(h >> 4) % len(SERVICES)]
        method = METHODS[(h >> 8) % len(METHODS)]
        path = PATHS[(h >> 12) % len(PATHS)]
        region = REGIONS[(h >> 16) % len(REGIONS)]
        status = STATUSES[(h >> 20) % len(STATUSES)]
        latency = (h % 500) + 1
        msg = MSGS[(h >> 24) % len(MSGS)]
        line = (
            f'{{"ts":"2026-04-27T12:34:56.{hid}Z","level":"{level}",'
            f'"service":"{service}","trace_id":"{hid}","span_id":"{hid}",'
            f'"user_id":"u-{hid}","method":"{method}",'
            f'"path":"{path}/{hid}","status":{status},'
            f'"latency_ms":{latency},"region":"{region}",'
            f'"host":"{service}-{hid}.svc.cluster.local","msg":"{msg}"}}\n'
        )
        out.append(line)
        total += len(line)
        counter += 1
    result = "".join(out)
    return result[:target_bytes]


def train_dict(sample_dir, dict_path, max_dict_size=2048):
    samples = sorted(str(p) for p in Path(sample_dir).glob("*.json"))
    if not samples:
        print("  no samples for dict training", file=sys.stderr)
        return False
    cmd = ["zstd", "--train", f"--maxdict={max_dict_size}",
           "-o", str(dict_path)] + samples
    result = subprocess.run(cmd, capture_output=True)
    if result.returncode != 0:
        print(f"  zstd --train failed: {result.stderr.decode()}", file=sys.stderr)
        return False
    size = Path(dict_path).stat().st_size
    print(f"  trained dict ({size} bytes) -> {dict_path}", file=sys.stderr)
    return True


def run_dict_bench(dict_path, extra_file):
    cmd = ["cargo", "run", "--release", "--example", "lz4rip_bench", "--",
           "--dict", str(dict_path), "--extra", str(extra_file),
           "--files", Path(extra_file).name]
    has_taskset = shutil.which("taskset") is not None
    if has_taskset:
        cmd = ["taskset", "-c", "0"] + cmd
    print(f"  running dict bench: {Path(extra_file).name}", file=sys.stderr)
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"  dict bench failed: {result.stderr}", file=sys.stderr)
        return []
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)
    return json.loads(result.stdout)


def dict_chart(results, out_dir):
    codecs = [c for c in DICT_CODEC_ORDER if any(r["codec"] == c for r in results)]
    n_codecs = len(codecs)
    hw_label = detect_hardware()
    transfer_rate = 1e9

    r0 = results[0]
    input_label = f"1 KB JSON message, dict trained on {DICT_TRAIN_COUNT} messages"

    bar_data = {}
    for codec in codecs:
        r = next((r for r in results if r["codec"] == codec), None)
        if not r:
            continue
        per_gb = 1e9 / r["input_size"]
        comp = r["compress_ns"] / 1e9 * per_gb
        transfer = (r["compressed_size"] / r["input_size"]) * (1e9 / transfer_rate)
        decomp = r["decompress_ns"] / 1e9 * per_gb
        bar_data[codec] = (comp, transfer, decomp, r)

    svg_w = 400
    svg_h = 340
    x_left, x_right = 70, 370
    plot_w = x_right - x_left
    y_top = 55 if hw_label else 45
    y_bot = 230
    plot_h = y_bot - y_top

    y_max = max(sum(v[:3]) for v in bar_data.values()) * 1.15

    def y(v):
        return y_bot - (v / y_max) * plot_h

    mid_x = (x_left + x_right) / 2
    L = []
    L.append(
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {svg_w} {svg_h}"'
        f' font-family="system-ui, -apple-system, sans-serif">'
    )
    L.append(f'  <rect width="{svg_w}" height="{svg_h}" fill="#0d1117"/>')

    L.append(
        f'  <text x="{mid_x}" y="22" text-anchor="middle" fill="#e6edf3"'
        f' font-size="13" font-weight="700">'
        f'LZ4 + Dict (2 KB): 1 KB JSON (lower is better)'
        f'</text>'
    )
    if hw_label:
        L.append(
            f'  <text x="{mid_x}" y="38" text-anchor="middle" fill="#7d8590"'
            f' font-size="10">{hw_label}</text>'
        )

    step = nice_step(y_max, 5)
    v = step
    while v <= y_max:
        yy = y(v)
        L.append(
            f'  <line x1="{x_left}" y1="{yy:.1f}" x2="{x_right}" y2="{yy:.1f}"'
            f' stroke="#21262d" stroke-width="1"/>'
        )
        L.append(
            f'  <text x="{x_left - 8}" y="{yy:.1f}" text-anchor="end"'
            f' dominant-baseline="middle" fill="#7d8590" font-size="10">{v:.1f}</text>'
        )
        v += step

    L.append(
        f'  <line x1="{x_left}" y1="{y_bot}" x2="{x_right}" y2="{y_bot}"'
        f' stroke="#30363d" stroke-width="1.5"/>'
    )

    mid_y = (y_top + y_bot) / 2
    L.append(
        f'  <text x="22" y="{mid_y}" text-anchor="middle" fill="#e6edf3"'
        f' font-size="11" font-weight="600"'
        f' transform="rotate(-90,22,{mid_y})">seconds / GB</text>'
    )

    bar_w = plot_w * 0.3
    gap = plot_w * 0.1
    total_bars_w = n_codecs * bar_w + (n_codecs - 1) * gap
    start_x = x_left + (plot_w - total_bars_w) / 2

    for ci, codec in enumerate(codecs):
        if codec not in bar_data:
            continue
        comp, transfer, decomp, r = bar_data[codec]
        main_c, xfer_c = DICT_COLORS[codec]

        bx = start_x + ci * (bar_w + gap)

        h_comp = (comp / y_max) * plot_h
        L.append(
            f'  <rect x="{bx:.1f}" y="{y(comp):.1f}"'
            f' width="{bar_w:.1f}" height="{h_comp:.1f}"'
            f' fill="{main_c}" rx="2"/>'
        )
        h_transfer = (transfer / y_max) * plot_h
        L.append(
            f'  <rect x="{bx:.1f}" y="{y(comp + transfer):.1f}"'
            f' width="{bar_w:.1f}" height="{h_transfer:.1f}"'
            f' fill="{xfer_c}" rx="2"/>'
        )
        h_decomp = (decomp / y_max) * plot_h
        L.append(
            f'  <rect x="{bx:.1f}" y="{y(comp + transfer + decomp):.1f}"'
            f' width="{bar_w:.1f}" height="{h_decomp:.1f}"'
            f' fill="{main_c}" rx="2"/>'
        )

        total_ns = r["compress_ns"] + r["decompress_ns"]
        cx = bx + bar_w / 2
        L.append(
            f'  <text x="{cx:.1f}" y="{y(comp + transfer + decomp) - 6:.1f}"'
            f' text-anchor="middle" fill="#e6edf3" font-size="10"'
            f' font-weight="600">{total_ns:.0f} ns</text>'
        )

    # codec labels below bars
    for ci, codec in enumerate(codecs):
        bx = start_x + ci * (bar_w + gap)
        cx = bx + bar_w / 2
        L.append(
            f'  <text x="{cx:.1f}" y="{y_bot + 16}" text-anchor="middle"'
            f' fill="#e6edf3" font-size="10" font-weight="600">'
            f'{DICT_LABELS[codec]}</text>'
        )


    # legend
    leg_y = y_bot + 52
    stack_items = [
        ("compress + decompress", "#60a5fa"),
        ("transfer @1 GB/s", "#4680c4"),
    ]
    stack_total = 300
    stack_start = mid_x - stack_total / 2
    for i, (label, swatch) in enumerate(stack_items):
        sx = stack_start + i * 180
        L.append(
            f'  <rect x="{sx:.0f}" y="{leg_y - 5}" width="12" height="12"'
            f' fill="{swatch}" rx="2"/>'
        )
        L.append(
            f'  <text x="{sx + 18:.0f}" y="{leg_y + 5}" fill="#7d8590"'
            f' font-size="10">{label}</text>'
        )

    # subtitle with training info
    L.append(
        f'  <text x="{mid_x}" y="{svg_h - 10}" text-anchor="middle" fill="#484f58"'
        f' font-size="9">{input_label}</text>'
    )

    L.append("</svg>")
    return "\n".join(L) + "\n"


DICT_TRAIN_COUNT = 2000


def generate_dict_charts(out_dir):
    with tempfile.TemporaryDirectory() as tmp:
        # generate training messages (128-2048 bytes each)
        sample_dir = Path(tmp) / "samples"
        sample_dir.mkdir()
        rng = random.Random(42)
        sizes = [rng.randint(128, 2048) for _ in range(DICT_TRAIN_COUNT)]
        for i, size in enumerate(sizes):
            data = json_payload(size, counter_start=i * 100)
            (sample_dir / f"msg_{i:04d}.json").write_text(data)

        # train dict
        dict_path = Path(tmp) / "json.dict"
        if not train_dict(sample_dir, dict_path):
            print("  skipping dict chart: training failed", file=sys.stderr)
            return

        # generate 1 KB bench message
        bench_msg = json_payload(1024, counter_start=999999)
        bench_file = Path(tmp) / "json_1k.json"
        bench_file.write_text(bench_msg)
        print(f"  bench message: {len(bench_msg)} bytes", file=sys.stderr)

        # run benchmark
        results = run_dict_bench(dict_path, bench_file)

    if not results:
        print("  skipping dict chart: no results", file=sys.stderr)
        return

    svg = dict_chart(results, out_dir)
    out_path = out_dir / "dict2k.svg"
    out_path.write_text(svg)
    print(f"  wrote {out_path}")


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <results.json|results.jsonl> [output_dir]", file=sys.stderr)
        sys.exit(1)

    results = load_results(sys.argv[1])
    out_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("doc/charts")
    out_dir.mkdir(exist_ok=True)

    svg = pipeline_chart(results, out_dir)
    out_path = out_dir / "pipeline.svg"
    out_path.write_text(svg)
    print(f"  wrote {out_path}")

    svg = summary_chart(results, out_dir)
    out_path = out_dir / "summary.svg"
    out_path.write_text(svg)
    print(f"  wrote {out_path}")

    generate_dict_charts(out_dir)


if __name__ == "__main__":
    main()
