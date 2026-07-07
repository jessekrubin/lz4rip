mod common;

use common::*;
use lz4rip::compress as compress_block;
use more_asserts::assert_lt;

#[test]
fn test_minimum_compression_ratio_block() {
    let compressed = compress_block(compression34k());
    let ratio = compressed.len() as f64 / compression34k().len() as f64;
    assert_lt!(ratio, 0.59);

    let compressed = compress_block(compression65());
    let ratio = compressed.len() as f64 / compression65().len() as f64;
    assert_lt!(ratio, 0.59);

    let compressed = compress_block(compression66json());
    let ratio = compressed.len() as f64 / compression66json().len() as f64;
    assert_lt!(ratio, 0.240);
}

#[cfg(feature = "frame")]
#[test]
fn test_minimum_compression_ratio_frame() {
    use lz4rip::frame::FrameInfo;

    let get_ratio = |input| {
        let compressed = lz4rip_frame_compress_with(FrameInfo::new(), input).unwrap();
        compressed.len() as f64 / input.len() as f64
    };

    let ratio = get_ratio(compression34k());
    assert_lt!(ratio, 0.645);

    let ratio = get_ratio(compression65());
    assert_lt!(ratio, 0.645);

    let ratio = get_ratio(compression66json());
    assert_lt!(ratio, 0.245);
}

fn print_ratio(text: &str, val1: usize, val2: usize) {
    println!(
        "{:?} {:.3} {} -> {}",
        text,
        val1 as f32 / val2 as f32,
        val1,
        val2
    );
}

#[test]
fn test_comp_flex() {
    print_ratio(
        "Ratio 1k flex",
        compression1k().len(),
        compress_block(compression1k()).len(),
    );
    print_ratio(
        "Ratio 34k flex",
        compression34k().len(),
        compress_block(compression34k()).len(),
    );
}
