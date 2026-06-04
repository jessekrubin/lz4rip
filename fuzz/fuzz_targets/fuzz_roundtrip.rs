#![no_main]
use libfuzzer_sys::fuzz_target;

use lz4rip::block::{compress_into, compress_prepend_size, decompress, decompress_into,
                     decompress_size_prepended, get_maximum_output_size};

#[derive(Debug, arbitrary::Arbitrary)]
struct Input {
    data: Vec<u8>,
    #[arbitrary(with = |u: &mut arbitrary::Unstructured| u.int_in_range(1..=8u8))]
    repeat: u8,
}

fuzz_target!(|input: Input| {
    // Repeat data to cross the 64KB hash table boundary (HashTable4KU16 → HashTable4K).
    let mut payload = Vec::with_capacity(input.data.len() * input.repeat as usize);
    for _ in 0..input.repeat {
        payload.extend_from_slice(&input.data);
    }

    // prepend-size API
    let compressed = compress_prepend_size(&payload);
    let decompressed = decompress_size_prepended(&compressed).unwrap();
    assert_eq!(payload, decompressed);

    // raw API
    let compressed = lz4rip::compress(&payload);
    let decompressed = decompress(&compressed, payload.len()).unwrap();
    assert_eq!(payload, decompressed);

    // into-buffer API: exact-size output buffer
    let max_out = get_maximum_output_size(payload.len());
    let mut comp_buf = vec![0u8; max_out];
    let comp_len = compress_into(&payload, &mut comp_buf).unwrap();
    let mut decomp_buf = vec![0u8; payload.len()];
    let decomp_len = decompress_into(&comp_buf[..comp_len], &mut decomp_buf).unwrap();
    assert_eq!(&payload[..], &decomp_buf[..decomp_len]);
});
