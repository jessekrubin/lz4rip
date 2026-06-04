#![no_main]
use libfuzzer_sys::fuzz_target;

use lz4rip::block::{compress, decompress};

/// Compress valid data, then corrupt the compressed output and try to decompress.
/// This targets the decompressor with structurally-plausible-but-wrong LZ4 blocks:
/// corrupted offsets, corrupted lengths, truncated blocks.
#[derive(Debug, arbitrary::Arbitrary)]
struct Input {
    data: Vec<u8>,
    #[arbitrary(with = |u: &mut arbitrary::Unstructured| u.int_in_range(1..=4u8))]
    repeat: u8,
    mutations: Vec<Mutation>,
}

#[derive(Debug, arbitrary::Arbitrary)]
enum Mutation {
    FlipBit { pos_frac: u8, bit: u8 },
    SetByte { pos_frac: u8, val: u8 },
    Truncate { keep_frac: u8 },
    InsertFF { pos_frac: u8 },
}

fuzz_target!(|input: Input| {
    let mut payload = Vec::with_capacity(input.data.len() * input.repeat as usize);
    for _ in 0..input.repeat {
        payload.extend_from_slice(&input.data);
    }
    if payload.is_empty() {
        return;
    }

    let compressed = compress(&payload);
    if compressed.is_empty() {
        return;
    }

    let mut corrupted = compressed.clone();
    for mutation in &input.mutations {
        if corrupted.is_empty() {
            break;
        }
        match *mutation {
            Mutation::FlipBit { pos_frac, bit } => {
                let pos = (pos_frac as usize * corrupted.len()) / 256;
                let pos = pos.min(corrupted.len() - 1);
                corrupted[pos] ^= 1 << (bit & 7);
            }
            Mutation::SetByte { pos_frac, val } => {
                let pos = (pos_frac as usize * corrupted.len()) / 256;
                let pos = pos.min(corrupted.len() - 1);
                corrupted[pos] = val;
            }
            Mutation::Truncate { keep_frac } => {
                let keep = ((keep_frac as usize + 1) * corrupted.len()) / 256;
                let keep = keep.max(1).min(corrupted.len());
                corrupted.truncate(keep);
            }
            Mutation::InsertFF { pos_frac } => {
                let pos = (pos_frac as usize * corrupted.len()) / 256;
                let pos = pos.min(corrupted.len());
                corrupted.insert(pos, 0xFF);
            }
        }
    }

    // Must not panic regardless of corruption. Error is fine.
    // Try with the original size and with generous over-allocation.
    let _ = decompress(&corrupted, payload.len());
    let _ = decompress(&corrupted, payload.len() * 2 + 1024);
    let _ = decompress(&corrupted, 0);

    // Exact-buffer decompress_into
    let mut out = vec![0u8; payload.len()];
    let _ = lz4rip::block::decompress_into(&corrupted, &mut out);
});
