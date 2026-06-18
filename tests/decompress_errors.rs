use lz4rip::block::{decompress, Decompressor};

fn decompress_with_size_prefix(data: &[u8]) -> Result<Vec<u8>, lz4rip::block::DecompressError> {
    if data.len() < 4 {
        return decompress(data, 0);
    }
    let size = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
    decompress(&data[4..], size)
}

fn test_decomp(data: &[u8]) {
    let size = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    if size > 20_000_000 {
        return;
    }
    let _ = decompress(&data[4..], size);
    let _ = Decompressor::with_dict(data).decompress(&data[4..], size);
}

#[test]
fn error_case_1() {
    let _err = decompress_with_size_prefix(&[122, 1, 0, 1, 0, 10, 1, 0]);
}

#[test]
fn error_case_2() {
    let _err = decompress_with_size_prefix(&[
        44, 251, 49, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
}

#[test]
fn error_case_3() {
    let _err = decompress_with_size_prefix(&[
        7, 0, 0, 0, 0, 0, 0, 11, 0, 0, 7, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 1, 0, 0,
    ]);
}

#[test]
fn error_case_4() {
    let _err = decompress_with_size_prefix(&[0, 61, 0, 0, 0, 7, 0]);
}

#[test]
fn error_case_5() {
    let _err = decompress_with_size_prefix(&[8, 0, 0, 0, 4, 0, 0, 0]);
}

#[test]
fn bug_fuzz_7() {
    let data = &[
        39, 0, 0, 0, 0, 0, 0, 237, 0, 0, 0, 0, 0, 0, 16, 0, 0, 4, 0, 0, 0, 39, 32, 0, 2, 0, 162, 5,
        36, 0, 0, 0, 0, 7, 0,
    ];
    test_decomp(data);
}

#[test]
fn bug_fuzz_8() {
    let data = &[
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 0, 0, 10,
    ];
    test_decomp(data);
}
