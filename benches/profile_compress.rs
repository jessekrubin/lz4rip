use std::hint::black_box;

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "json66k".into());
    let data: &[u8] = match which.as_str() {
        "json66k" => include_bytes!("../corpus/compression_66k_JSON.txt"),
        "text34k" => include_bytes!("../corpus/compression_34k.txt"),
        "hdfs" => include_bytes!("../corpus/hdfs.json"),
        "xml" => include_bytes!("../corpus/xml_collection.xml"),
        "dickens" => include_bytes!("../corpus/dickens.txt"),
        other => panic!("unknown input: {other}"),
    };
    let iters: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);
    let max_output = lz4rip::block::get_maximum_output_size(data.len());
    let mut output = vec![0u8; max_output];

    for _ in 0..iters {
        let n = lz4rip::compress_into(data, &mut output).unwrap();
        black_box(n);
    }
}
