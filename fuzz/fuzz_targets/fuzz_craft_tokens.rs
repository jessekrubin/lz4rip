#![no_main]
use libfuzzer_sys::fuzz_target;

use lz4rip::block::{decompress_into, Decompressor};

/// Build a structurally-valid LZ4 block from fuzzer-controlled parameters.
/// Targets: wild_copy_16 overshoot, wild_match_copy_18, offset=1 RLE,
/// small-offset overlapping copies, variable-length integer overflow,
/// fast-path/slow-path boundary, dict-to-output match crossing.
#[derive(Debug, arbitrary::Arbitrary)]
struct TokenSequence {
    tokens: Vec<Token>,
    #[arbitrary(with = |u: &mut arbitrary::Unstructured| u.int_in_range(0..=65536usize))]
    output_size: usize,
    #[arbitrary(with = |u: &mut arbitrary::Unstructured| u.int_in_range(0..=4096usize))]
    dict_size: usize,
}

#[derive(Debug, arbitrary::Arbitrary)]
struct Token {
    literal_len: u8,
    match_len: u8,
    offset: u16,
    literal_fill: u8,
    /// Extra 0xFF bytes before the terminating length byte. Targets read_integer.
    #[arbitrary(with = |u: &mut arbitrary::Unstructured| u.int_in_range(0..=4u8))]
    extra_lit_ext: u8,
    #[arbitrary(with = |u: &mut arbitrary::Unstructured| u.int_in_range(0..=4u8))]
    extra_match_ext: u8,
    is_last: bool,
}

fn build_block(seq: &TokenSequence) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    let num_tokens = seq.tokens.len().min(32);
    for (i, tok) in seq.tokens.iter().take(num_tokens).enumerate() {
        let is_last = tok.is_last || i + 1 == num_tokens;
        let lit_nibble = (tok.literal_len >> 4).min(15);
        let match_nibble = (tok.match_len & 0x0F).min(15);

        let token_byte = (lit_nibble << 4) | match_nibble;
        out.push(token_byte);

        // Extended literal length
        let mut lit_len = lit_nibble as usize;
        if lit_nibble == 15 {
            for _ in 0..tok.extra_lit_ext {
                out.push(0xFF);
                lit_len += 255;
            }
            out.push(tok.literal_len & 0x7F);
            lit_len += (tok.literal_len & 0x7F) as usize;
        }

        // Literal bytes
        for _ in 0..lit_len {
            out.push(tok.literal_fill);
        }

        if is_last && match_nibble == 0 && lit_nibble < 15 {
            break;
        }

        // Offset (little-endian u16). Include pathological values: 0, 1, small.
        out.extend_from_slice(&tok.offset.to_le_bytes());

        // Extended match length
        if match_nibble == 15 {
            for _ in 0..tok.extra_match_ext {
                out.push(0xFF);
            }
            out.push(tok.match_len & 0x7F);
        }

        if is_last {
            break;
        }
    }
    out
}

fuzz_target!(|seq: TokenSequence| {
    let block = build_block(&seq);
    let output_size = seq.output_size.min(1 << 20);

    // Without dict: must not panic.
    let mut out = vec![0u8; output_size];
    let _ = decompress_into(&block, &mut out);

    // With dict: exercises dict-to-output match crossing.
    if seq.dict_size > 0 {
        let dict: Vec<u8> = (0..seq.dict_size).map(|i| (i & 0xFF) as u8).collect();
        let mut out = vec![0u8; output_size];
        let _ = Decompressor::with_dict(&dict).decompress_into(&block, &mut out);
    }
});
