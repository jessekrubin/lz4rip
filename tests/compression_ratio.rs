mod common;

use common::*;
use lz4rip::compress as compress_block;
use more_asserts::assert_lt;

#[test]
fn test_minimum_compression_ratio_block() {
    let compressed = compress_block(COMPRESSION34K);
    let ratio = compressed.len() as f64 / COMPRESSION34K.len() as f64;
    assert_lt!(ratio, 0.59);

    let compressed = compress_block(COMPRESSION65);
    let ratio = compressed.len() as f64 / COMPRESSION65.len() as f64;
    assert_lt!(ratio, 0.59);

    let compressed = compress_block(COMPRESSION66JSON);
    let ratio = compressed.len() as f64 / COMPRESSION66JSON.len() as f64;
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

    let ratio = get_ratio(COMPRESSION34K);
    assert_lt!(ratio, 0.645);

    let ratio = get_ratio(COMPRESSION65);
    assert_lt!(ratio, 0.645);

    let ratio = get_ratio(COMPRESSION66JSON);
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
        COMPRESSION1K.len(),
        compress_block(COMPRESSION1K).len(),
    );
    print_ratio(
        "Ratio 34k flex",
        COMPRESSION34K.len(),
        compress_block(COMPRESSION34K).len(),
    );
}
