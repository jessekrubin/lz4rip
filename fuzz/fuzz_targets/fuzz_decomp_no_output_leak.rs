#![no_main]
use libfuzzer_sys::fuzz_target;

use lz4rip::block::{decompress_into, Decompressor};

#[derive(Debug, arbitrary::Arbitrary)]
struct FuzzData {
    input: Vec<u8>,
    #[arbitrary(with = |u: &mut arbitrary::Unstructured| u.int_in_range(0..=65536usize))]
    dict_size: usize,
    dict_byte: u8,
}

fuzz_target!(|fuzz_data: FuzzData| {
    let input = &fuzz_data.input;
    // Dict with non-trivial content so dict-match copies are exercised.
    let dict: Vec<u8> = (0..fuzz_data.dict_size)
        .map(|i| fuzz_data.dict_byte.wrapping_add(i as u8))
        .collect();

    let buf_size = 512.max(input.len() * 4);

    fn decompress(input: &[u8], output: &mut [u8], dict: &[u8]) -> Result<usize, lz4rip::block::DecompressError> {
        if dict.is_empty() {
            decompress_into(input, output)
        } else {
            Decompressor::new(dict).decompress_into(input, output)
        }
    }

    // First decompress: zero-filled buffer.
    let mut output1 = vec![0u8; buf_size];
    let len1 = match decompress(input, &mut output1, &dict) {
        Ok(len) => len,
        Err(_) => return,
    };
    let result1 = output1[..len1].to_owned();

    // Second decompress: 0xFF-filled buffer. Result must be identical.
    let mut output2 = vec![0xFFu8; buf_size];
    let len2 = decompress(input, &mut output2, &dict).unwrap();
    assert_eq!(result1, &output2[..len2]);

    // Third: 0xAA-filled. Belt and suspenders.
    let mut output3 = vec![0xAAu8; buf_size];
    let len3 = decompress(input, &mut output3, &dict).unwrap();
    assert_eq!(result1, &output3[..len3]);
});
