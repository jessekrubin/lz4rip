use std::hint::black_box;

const ITERATIONS: usize = 50_000;

fn read_input(which: &str) -> Vec<u8> {
    let path = match which {
        "hdfs" | "json" => "corpus/hdfs.json",
        "xml" => "corpus/silesia/xml",
        "dickens" => "corpus/silesia/dickens",
        _ => panic!("usage: profile_decompress_c [hdfs|xml|dickens]"),
    };
    std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "hdfs".into());
    let data = read_input(&which);
    let mut compressed = Vec::new();
    lzzzz::lz4::compress_to_vec(&data, &mut compressed, lzzzz::lz4::ACC_LEVEL_DEFAULT).unwrap();
    let mut output = vec![0u8; data.len()];

    for _ in 0..ITERATIONS {
        let n = lzzzz::lz4::decompress(&compressed, &mut output).unwrap();
        black_box(n);
    }
}
