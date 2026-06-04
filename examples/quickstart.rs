use std::io::{self, Write};

use lz4rip::block::{compress_prepend_size, decompress_size_prepended, Compressor, Decompressor};

fn main() {
    // Block format
    let input: &[u8] = b"Hello people, what's up?";
    let compressed = compress_prepend_size(input);
    let uncompressed = decompress_size_prepended(&compressed).unwrap();
    assert_eq!(input, uncompressed);
    println!("block: {} -> {} bytes", input.len(), compressed.len());

    // Block format with dictionary
    // Dict trained on representative samples of the message format.
    let dict = br#"{"sensor_id":"","temperature":0.0,"humidity":0.0,"timestamp":"2025-01-01T00:00:00Z","location":{"building":"","floor":0,"room":""},"status":"online"}"#;

    let mut comp = Compressor::with_dict(dict);
    let msg = br#"{"sensor_id":"env-7f3a","temperature":22.4,"humidity":51.3,"timestamp":"2025-06-04T14:30:07Z","location":{"building":"HQ","floor":3,"room":"3-117"},"status":"online"}"#;
    let compressed = comp.compress_prepend_size(msg);
    let no_dict = compress_prepend_size(msg);

    let decomp = Decompressor::with_dict(dict);
    let original = decomp.decompress_size_prepended(&compressed).unwrap();
    assert_eq!(&original, msg);
    println!(
        "block+dict: {} -> {} bytes (vs {} without dict)",
        msg.len(),
        compressed.len(),
        no_dict.len()
    );

    // Frame format (streaming)
    let mut encoder = lz4rip::frame::FrameEncoder::new(Vec::new());
    encoder.write_all(b"Hello people, what's up?").unwrap();
    let compressed = encoder.finish().unwrap();

    let mut decoder = lz4rip::frame::FrameDecoder::new(&compressed[..]);
    let mut output = Vec::new();
    io::copy(&mut decoder, &mut output).unwrap();
    assert_eq!(&output, b"Hello people, what's up?");
    println!("frame: {} -> {} bytes", output.len(), compressed.len());
}
