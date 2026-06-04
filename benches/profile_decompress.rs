use std::hint::black_box;

const JSON_DATA: &[u8] = include_bytes!("../corpus/compression_66k_JSON.txt");
const TEXT_34K: &[u8] = include_bytes!("../corpus/compression_34k.txt");
const ITERATIONS: usize = 50_000;

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "json".into());
    let data: &[u8] = match which.as_str() {
        "json" => JSON_DATA,
        "text" => TEXT_34K,
        _ => panic!("usage: profile_decompress [json|text]"),
    };
    let mut compressed = Vec::new();
    lzzzz::lz4::compress_to_vec(data, &mut compressed, lzzzz::lz4::ACC_LEVEL_DEFAULT).unwrap();
    let mut output = vec![0u8; data.len()];

    for _ in 0..ITERATIONS {
        let n = lz4rip::decompress_into(&compressed, &mut output).unwrap();
        black_box(n);
    }
}
