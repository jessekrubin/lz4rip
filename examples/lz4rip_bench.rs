extern crate libc;
extern crate lz4_flex_upstream;

use std::io::Write;
use std::os::raw::c_int;
use std::path::PathBuf;
use std::process::Command;

#[repr(C)]
struct LZ4Stream {
    _opaque: [u8; 0],
}

extern "C" {
    fn LZ4_createStream() -> *mut LZ4Stream;
    fn LZ4_freeStream(stream: *mut LZ4Stream) -> c_int;
    fn LZ4_resetStream_fast(stream: *mut LZ4Stream);
    fn LZ4_loadDict(stream: *mut LZ4Stream, dict: *const u8, dict_size: c_int) -> c_int;
    fn LZ4_compress_fast_continue(
        stream: *mut LZ4Stream,
        src: *const u8,
        dst: *mut u8,
        src_size: c_int,
        dst_capacity: c_int,
        acceleration: c_int,
    ) -> c_int;
    fn LZ4_decompress_safe_usingDict(
        src: *const u8,
        dst: *mut u8,
        compressed_size: c_int,
        dst_capacity: c_int,
        dict: *const u8,
        dict_size: c_int,
    ) -> c_int;
}

fn cpu_nanos() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

#[derive(Clone)]
struct BenchResult {
    codec: String,
    input_name: String,
    input_size: usize,
    compressed_size: usize,
    compress_ns: f64,
    decompress_ns: f64,
}

impl BenchResult {
    fn to_json(&self) -> String {
        format!(
            r#"{{"codec": "{}", "input": "{}", "input_size": {}, "compressed_size": {}, "compress_ns": {:.1}, "decompress_ns": {:.1}}}"#,
            self.codec,
            self.input_name,
            self.input_size,
            self.compressed_size,
            self.compress_ns,
            self.decompress_ns
        )
    }

    fn from_json(line: &str) -> Option<Self> {
        let line = line.trim().trim_matches(',');
        if line == "[" || line == "]" || line.is_empty() {
            return None;
        }
        let get = |key: &str| -> Option<String> {
            let prefix = format!("\"{key}\": ");
            let start = line.find(&prefix)? + prefix.len();
            let rest = &line[start..];
            if let Some(stripped) = rest.strip_prefix('"') {
                let end = stripped.find('"')?;
                Some(stripped[..end].to_string())
            } else {
                let end = rest.find([',', '}']).unwrap_or(rest.len());
                Some(rest[..end].to_string())
            }
        };
        Some(BenchResult {
            codec: get("codec")?,
            input_name: get("input")?,
            input_size: get("input_size")?.parse().ok()?,
            compressed_size: get("compressed_size")?.parse().ok()?,
            compress_ns: get("compress_ns")?.parse().ok()?,
            decompress_ns: get("decompress_ns")?.parse().ok()?,
        })
    }
}

fn bench_loop<F: FnMut()>(warmup: usize, target_ns: u64, rounds: usize, mut f: F) -> f64 {
    for _ in 0..warmup {
        f();
    }
    let mut best = f64::MAX;
    for _ in 0..rounds {
        let mut iters = 0u64;
        let start = cpu_nanos();
        loop {
            std::hint::black_box(&mut f)();
            iters += 1;
            if cpu_nanos() - start >= target_ns {
                break;
            }
        }
        let elapsed = cpu_nanos() - start;
        let ns_per_op = elapsed as f64 / iters as f64;
        if ns_per_op < best {
            best = ns_per_op;
        }
    }
    best
}

fn bench_lz4rip(data: &[u8], name: &str, target_ns: u64) -> BenchResult {
    let max_out = lz4rip::block::get_maximum_output_size(data.len());
    let mut comp_buf = vec![0u8; max_out];
    let comp_len = lz4rip::block::compress_into(data, &mut comp_buf).unwrap();
    let compressed = comp_buf[..comp_len].to_vec();
    let mut decomp_buf = vec![0u8; data.len()];

    let compress_ns = bench_loop(3, target_ns, 10, || {
        let _ = lz4rip::block::compress_into(
            std::hint::black_box(data),
            std::hint::black_box(&mut comp_buf),
        );
    });

    let decompress_ns = bench_loop(3, target_ns, 10, || {
        let _ = lz4rip::block::decompress_into(
            std::hint::black_box(&compressed),
            std::hint::black_box(&mut decomp_buf),
        );
    });

    BenchResult {
        codec: "lz4rip".to_string(),
        input_name: name.to_string(),
        input_size: data.len(),
        compressed_size: comp_len,
        compress_ns,
        decompress_ns,
    }
}

fn bench_lz4_flex_upstream(data: &[u8], name: &str, target_ns: u64) -> BenchResult {
    let max_out = lz4_flex_upstream::block::get_maximum_output_size(data.len());
    let mut comp_buf = vec![0u8; max_out];
    let comp_len = lz4_flex_upstream::block::compress_into(data, &mut comp_buf).unwrap();
    let compressed = comp_buf[..comp_len].to_vec();
    let mut decomp_buf = vec![0u8; data.len()];

    let compress_ns = bench_loop(3, target_ns, 10, || {
        let _ = lz4_flex_upstream::block::compress_into(
            std::hint::black_box(data),
            std::hint::black_box(&mut comp_buf),
        );
    });

    let decompress_ns = bench_loop(3, target_ns, 10, || {
        let _ = lz4_flex_upstream::block::decompress_into(
            std::hint::black_box(&compressed),
            std::hint::black_box(&mut decomp_buf),
        );
    });

    BenchResult {
        codec: "lz4_flex".to_string(),
        input_name: name.to_string(),
        input_size: data.len(),
        compressed_size: comp_len,
        compress_ns,
        decompress_ns,
    }
}

fn bench_c_lz4(data: &[u8], name: &str, target_ns: u64) -> BenchResult {
    let max_out = lzzzz::lz4::max_compressed_size(data.len());
    let mut comp_buf = vec![0u8; max_out];
    let comp_len =
        lzzzz::lz4::compress(data, &mut comp_buf, lzzzz::lz4::ACC_LEVEL_DEFAULT).unwrap();
    let compressed = comp_buf[..comp_len].to_vec();
    let mut decomp_buf = vec![0u8; data.len()];

    let compress_ns = bench_loop(3, target_ns, 10, || {
        let _ = lzzzz::lz4::compress(
            std::hint::black_box(data),
            std::hint::black_box(&mut comp_buf),
            lzzzz::lz4::ACC_LEVEL_DEFAULT,
        );
    });

    let decompress_ns = bench_loop(3, target_ns, 10, || {
        let _ = lzzzz::lz4::decompress(
            std::hint::black_box(&compressed),
            std::hint::black_box(&mut decomp_buf),
        );
    });

    BenchResult {
        codec: "C lz4".to_string(),
        input_name: name.to_string(),
        input_size: data.len(),
        compressed_size: comp_len,
        compress_ns,
        decompress_ns,
    }
}

fn bench_lz4rip_dict(data: &[u8], dict: &[u8], name: &str, target_ns: u64) -> BenchResult {
    let mut compressor = lz4rip::block::Compressor::with_dict(dict);
    let max_out = lz4rip::block::get_maximum_output_size(data.len());
    let mut comp_buf = vec![0u8; max_out];
    let comp_len = compressor.compress_into(data, &mut comp_buf).unwrap();
    let compressed = comp_buf[..comp_len].to_vec();

    let decompressor = lz4rip::block::Decompressor::with_dict(dict);
    let mut decomp_buf = vec![0u8; data.len()];
    let check = decompressor
        .decompress_into(&compressed, &mut decomp_buf)
        .unwrap();
    assert_eq!(check, data.len());
    assert_eq!(&decomp_buf[..], data);

    let compress_ns = bench_loop(3, target_ns, 10, || {
        let _ = std::hint::black_box(&mut compressor).compress_into(
            std::hint::black_box(data),
            std::hint::black_box(&mut comp_buf),
        );
    });

    let decompress_ns = bench_loop(3, target_ns, 10, || {
        let _ = decompressor.decompress_into(
            std::hint::black_box(&compressed),
            std::hint::black_box(&mut decomp_buf),
        );
    });

    BenchResult {
        codec: "lz4rip (dict 2K)".to_string(),
        input_name: name.to_string(),
        input_size: data.len(),
        compressed_size: comp_len,
        compress_ns,
        decompress_ns,
    }
}

fn bench_c_lz4_dict(data: &[u8], dict: &[u8], name: &str, target_ns: u64) -> BenchResult {
    let max_out = lzzzz::lz4::max_compressed_size(data.len());
    let mut comp_buf = vec![0u8; max_out];
    let mut decomp_buf = vec![0u8; data.len()];

    let stream = unsafe { LZ4_createStream() };
    assert!(!stream.is_null());

    // initial compress to get compressed_size
    unsafe {
        LZ4_resetStream_fast(stream);
        LZ4_loadDict(stream, dict.as_ptr(), dict.len() as c_int);
    }
    let comp_len = unsafe {
        LZ4_compress_fast_continue(
            stream,
            data.as_ptr(),
            comp_buf.as_mut_ptr(),
            data.len() as c_int,
            max_out as c_int,
            1,
        )
    };
    assert!(comp_len > 0, "C lz4 dict compress failed");
    let comp_len = comp_len as usize;
    let compressed = comp_buf[..comp_len].to_vec();

    // verify roundtrip
    let dec_len = unsafe {
        LZ4_decompress_safe_usingDict(
            compressed.as_ptr(),
            decomp_buf.as_mut_ptr(),
            compressed.len() as c_int,
            decomp_buf.len() as c_int,
            dict.as_ptr(),
            dict.len() as c_int,
        )
    };
    assert_eq!(dec_len as usize, data.len());
    assert_eq!(&decomp_buf[..], data);

    let compress_ns = bench_loop(3, target_ns, 10, || unsafe {
        LZ4_resetStream_fast(stream);
        LZ4_loadDict(stream, dict.as_ptr(), dict.len() as c_int);
        LZ4_compress_fast_continue(
            stream,
            data.as_ptr(),
            comp_buf.as_mut_ptr(),
            data.len() as c_int,
            max_out as c_int,
            1,
        );
    });

    let decompress_ns = bench_loop(3, target_ns, 10, || unsafe {
        LZ4_decompress_safe_usingDict(
            compressed.as_ptr(),
            decomp_buf.as_mut_ptr(),
            compressed.len() as c_int,
            decomp_buf.len() as c_int,
            dict.as_ptr(),
            dict.len() as c_int,
        );
    });

    unsafe { LZ4_freeStream(stream) };

    BenchResult {
        codec: "C lz4 (dict 2K)".to_string(),
        input_name: name.to_string(),
        input_size: data.len(),
        compressed_size: comp_len,
        compress_ns,
        decompress_ns,
    }
}

fn cache_dir() -> PathBuf {
    let dir = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
        .join(".cache")
        .join("lz4rip");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn codec_cache_path(codec: &str) -> PathBuf {
    let filename = codec.replace(' ', "_").replace(['(', ')'], "");
    cache_dir().join(format!("{filename}.jsonl"))
}

fn load_cache(codecs: &[&str]) -> Vec<BenchResult> {
    let mut results = Vec::new();
    for codec in codecs {
        let path = codec_cache_path(codec);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        results.extend(content.lines().filter_map(BenchResult::from_json));
    }
    results
}

fn save_cache(results: &[BenchResult], codecs: &[&str]) {
    for codec in codecs {
        let entries: Vec<_> = results.iter().filter(|r| r.codec == *codec).collect();
        if entries.is_empty() {
            continue;
        }
        let path = codec_cache_path(codec);
        let mut f = std::fs::File::create(&path).unwrap();
        for r in &entries {
            writeln!(f, "{}", r.to_json()).unwrap();
        }
        eprintln!("cached {} results to {}", entries.len(), path.display());
    }
}

const CODECS: &[&str] = &["C lz4", "lz4rip", "lz4_flex"];
const DICT_CODECS: &[&str] = &["C lz4 (dict 2K)", "lz4rip (dict 2K)"];

const SILESIA_DOWNLOADS: &[(&str, &str)] = &[
    (
        "corpus/dickens.txt",
        "https://sun.aei.polsl.pl/~sdeor/corpus/dickens.bz2",
    ),
    (
        "corpus/silesia/mr",
        "https://sun.aei.polsl.pl/~sdeor/corpus/mr.bz2",
    ),
    (
        "corpus/silesia/mozilla",
        "https://sun.aei.polsl.pl/~sdeor/corpus/mozilla.bz2",
    ),
    (
        "corpus/silesia/nci",
        "https://sun.aei.polsl.pl/~sdeor/corpus/nci.bz2",
    ),
    (
        "corpus/silesia/osdb",
        "https://sun.aei.polsl.pl/~sdeor/corpus/osdb.bz2",
    ),
    (
        "corpus/silesia/samba",
        "https://sun.aei.polsl.pl/~sdeor/corpus/samba.bz2",
    ),
    (
        "corpus/silesia/sao",
        "https://sun.aei.polsl.pl/~sdeor/corpus/sao.bz2",
    ),
    (
        "corpus/silesia/webster",
        "https://sun.aei.polsl.pl/~sdeor/corpus/webster.bz2",
    ),
    (
        "corpus/silesia/x-ray",
        "https://sun.aei.polsl.pl/~sdeor/corpus/x-ray.bz2",
    ),
];

fn ensure_corpus() {
    for &(path, url) in SILESIA_DOWNLOADS {
        if std::fs::metadata(path).is_ok() {
            continue;
        }
        eprintln!("downloading {url} ...");
        let dir = PathBuf::from(path).parent().unwrap().to_owned();
        std::fs::create_dir_all(&dir).ok();
        let status = Command::new("sh")
            .arg("-c")
            .arg(format!("curl -fSL '{url}' | bzip2 -d > '{path}'"))
            .status();
        match status {
            Ok(s) if s.success() => {
                let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                eprintln!("  saved {path} ({size} bytes)");
            }
            _ => {
                eprintln!("  failed to download {path}, skipping");
                std::fs::remove_file(path).ok();
            }
        }
    }
}

const ALL_FILES: &[&str] = &[
    "corpus/compression_1k.txt",
    "corpus/compression_34k.txt",
    "corpus/compression_65k.txt",
    "corpus/compression_66k_JSON.txt",
    "corpus/dickens.txt",
    "corpus/hdfs.json",
    "corpus/reymont.pdf",
    "corpus/xml_collection.xml",
    "corpus/silesia/mr",
    "corpus/silesia/mozilla",
    "corpus/silesia/nci",
    "corpus/silesia/osdb",
    "corpus/silesia/samba",
    "corpus/silesia/sao",
    "corpus/silesia/webster",
    "corpus/silesia/x-ray",
];

fn main() {
    ensure_corpus();

    let args: Vec<String> = std::env::args().collect();
    let mut only: Vec<String> = Vec::new();
    let mut dict_path: Option<String> = None;
    let mut file_filter: Vec<String> = Vec::new();
    let mut extra_files: Vec<String> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--impl" => {
                i += 1;
                if i < args.len() {
                    only.push(args[i].clone());
                }
            }
            "--dict" => {
                i += 1;
                if i < args.len() {
                    dict_path = Some(args[i].clone());
                }
            }
            "--files" => {
                i += 1;
                if i < args.len() {
                    file_filter.extend(args[i].split(',').map(|s| s.to_string()));
                }
            }
            "--extra" => {
                i += 1;
                if i < args.len() {
                    extra_files.push(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    let dict_data = dict_path
        .map(|p| std::fs::read(&p).unwrap_or_else(|e| panic!("cannot read dict {p}: {e}")));

    let codecs: &[&str] = if dict_data.is_some() {
        DICT_CODECS
    } else {
        CODECS
    };
    let target_ns = 20_000_000u64;
    let cached = load_cache(codecs);
    let mut results: Vec<BenchResult> = Vec::new();

    let all_paths: Vec<&str> = ALL_FILES
        .iter()
        .copied()
        .chain(extra_files.iter().map(|s| s.as_str()))
        .collect();

    for path in &all_paths {
        let name = path.rsplit('/').next().unwrap();
        if !file_filter.is_empty() && !file_filter.iter().any(|f| f == name) {
            continue;
        }

        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("skipping {path}: not found");
                continue;
            }
        };

        for &codec in codecs {
            let should_run = only.is_empty() || only.iter().any(|o| codec.contains(o.as_str()));

            if !should_run {
                if let Some(c) = cached
                    .iter()
                    .find(|c| c.codec == codec && c.input_name == name)
                {
                    eprintln!("  {codec} x {name}: cached");
                    results.push(c.clone());
                    continue;
                }
            }

            eprintln!("  {codec} x {name}: benchmarking...");
            let r = match codec {
                "C lz4" => bench_c_lz4(&data, name, target_ns),
                "lz4rip" => bench_lz4rip(&data, name, target_ns),
                "lz4_flex" => bench_lz4_flex_upstream(&data, name, target_ns),
                "C lz4 (dict 2K)" => {
                    bench_c_lz4_dict(&data, dict_data.as_ref().unwrap(), name, target_ns)
                }
                "lz4rip (dict 2K)" => {
                    bench_lz4rip_dict(&data, dict_data.as_ref().unwrap(), name, target_ns)
                }
                _ => unreachable!(),
            };
            results.push(r);
        }
    }

    save_cache(&results, codecs);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "[").unwrap();
    for (i, r) in results.iter().enumerate() {
        let comma = if i + 1 < results.len() { "," } else { "" };
        writeln!(out, "  {}{}", r.to_json(), comma).unwrap();
    }
    writeln!(out, "]").unwrap();
}
