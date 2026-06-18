//! Tests.

#[macro_use]
extern crate more_asserts;

use std::iter;

#[cfg(feature = "frame")]
use lz4rip::frame::BlockMode;
use lz4rip::{
    block::{decompress, Decompressor},
    compress as compress_block,
};

const COMPRESSION1K: &[u8] = include_bytes!("../corpus/compression_1k.txt");
const COMPRESSION34K: &[u8] = include_bytes!("../corpus/compression_34k.txt");
const COMPRESSION65: &[u8] = include_bytes!("../corpus/compression_65k.txt");
const COMPRESSION66JSON: &[u8] = include_bytes!("../corpus/compression_66k_JSON.txt");
static DICKENS: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
    let path = std::path::Path::new("corpus/dickens.txt");
    if let Ok(data) = std::fs::read(path) {
        return data;
    }
    let url = "https://sun.aei.polsl.pl/~sdeor/corpus/dickens.bz2";
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("curl -fSL '{url}' | bzip2 -d"))
        .output()
        .expect("failed to download dickens");
    assert!(output.status.success(), "failed to download dickens.bz2");
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    std::fs::write(path, &output.stdout).ok();
    output.stdout
});

fn lz4_cpp_block_compress(input: &[u8]) -> Result<Vec<u8>, lzzzz::Error> {
    let mut out = Vec::new();
    lzzzz::lz4::compress_to_vec(input, &mut out, lzzzz::lz4::ACC_LEVEL_DEFAULT).unwrap();
    Ok(out)
}

fn lz4_cpp_block_decompress(input: &[u8], decomp_len: usize) -> Result<Vec<u8>, lzzzz::Error> {
    let mut out = vec![0u8; decomp_len];
    lzzzz::lz4::decompress(input, &mut out)?;
    Ok(out)
}

#[cfg(feature = "frame")]
fn lz4_cpp_frame_compress(input: &[u8], independent: bool) -> Result<Vec<u8>, lzzzz::Error> {
    let pref = lzzzz::lz4f::PreferencesBuilder::new()
        .block_mode(if independent {
            lzzzz::lz4f::BlockMode::Independent
        } else {
            lzzzz::lz4f::BlockMode::Linked
        })
        .build();
    let mut out = Vec::new();
    lzzzz::lz4f::compress_to_vec(input, &mut out, &pref).unwrap();
    Ok(out)
}

#[cfg(feature = "frame")]
fn lz4_cpp_frame_decompress(input: &[u8]) -> Result<Vec<u8>, lzzzz::lz4f::Error> {
    let mut out = Vec::new();
    lzzzz::lz4f::decompress_to_vec(input, &mut out)?;
    Ok(out)
}

#[cfg(feature = "frame")]
pub fn lz4rip_frame_compress_with(
    frame_info: lz4rip::frame::FrameInfo,
    input: &[u8],
) -> Result<Vec<u8>, std::io::Error> {
    let buffer = Vec::new();
    let mut enc = lz4rip::frame::FrameEncoder::with_frame_info(frame_info, buffer);
    std::io::Write::write_all(&mut enc, input)?;
    Ok(enc.finish()?)
}

#[cfg(feature = "frame")]
pub fn lz4rip_frame_decompress(input: &[u8]) -> Result<Vec<u8>, lz4rip::frame::Error> {
    let mut de = lz4rip::frame::FrameDecoder::new(input);
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut de, &mut out)?;
    Ok(out)
}

/// Test that the compressed string decompresses to the original string.
fn test_roundtrip(bytes: impl AsRef<[u8]>) {
    let bytes = bytes.as_ref();
    // compress with rust, decompress with rust
    let compressed_flex = compress_block(bytes);
    let decompressed = decompress(&compressed_flex, bytes.len()).unwrap();
    assert_eq!(decompressed, bytes);

    // Frame format
    // compress with rust, decompress with rust
    #[cfg(feature = "frame")]
    for bm in &[BlockMode::Independent, BlockMode::Linked] {
        let mut frame_info = lz4rip::frame::FrameInfo::new();
        frame_info.block_mode = *bm;
        let compressed_flex = lz4rip_frame_compress_with(frame_info, bytes).unwrap();
        let decompressed = lz4rip_frame_decompress(&compressed_flex).unwrap();
        assert_eq!(decompressed, bytes);
    }

    lz4_cpp_compatibility(bytes);
}

/// disabled in miri case
#[cfg(miri)]
fn lz4_cpp_compatibility(_bytes: &[u8]) {}

#[cfg(not(miri))]
fn lz4_cpp_compatibility(bytes: &[u8]) {
    // compress with lz4 cpp, decompress with rust
    if !bytes.is_empty() {
        // lz4_cpp_block_compress will return empty output for empty input but
        // that's in the bindings and not the linked library.
        let compressed = lz4_cpp_block_compress(bytes).unwrap();
        let decompressed = decompress(&compressed, bytes.len()).unwrap();
        assert_eq!(decompressed, bytes);
    }

    // compress with rust, decompress with lz4 cpp
    let compressed_flex = compress_block(bytes);
    let decompressed = lz4_cpp_block_decompress(&compressed_flex, bytes.len()).unwrap();
    assert_eq!(decompressed, bytes);

    // Frame format
    #[cfg(feature = "frame")]
    {
        // compress with lz4 cpp, decompress with rust
        let compressed = lz4_cpp_frame_compress(bytes, true).unwrap();
        let decompressed = lz4rip_frame_decompress(&compressed).unwrap();
        assert_eq!(decompressed, bytes);
        let compressed = lz4_cpp_frame_compress(bytes, false).unwrap();
        let decompressed = lz4rip_frame_decompress(&compressed).unwrap();
        assert_eq!(decompressed, bytes);

        // compress with rust, decompress with lz4 cpp
        //if !bytes.is_empty() {
        // compress_frame won't write a header if nothing is written to it
        // which is more in line with io::Write interface?
        for bm in &[BlockMode::Independent, BlockMode::Linked] {
            let mut frame_info = lz4rip::frame::FrameInfo::new();
            frame_info.block_mode = *bm;
            let compressed_flex = lz4rip_frame_compress_with(frame_info, bytes).unwrap();
            let decompressed = lz4_cpp_frame_decompress(&compressed_flex).unwrap();
            assert_eq!(decompressed, bytes);
        }
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn compare_compression() {
    print_compression_ration(COMPRESSION1K, "1k");
    print_compression_ration(COMPRESSION34K, "34k");
    print_compression_ration(COMPRESSION66JSON, "66k JSON");
    print_compression_ration(&DICKENS, "dickens");
}

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

fn print_compression_ration(input: &'static [u8], name: &str) {
    println!("\nComparing for {name}");
    let name = "";
    let compressed = compress_block(input);
    // println!("{:?}", compressed);
    println!(
        "lz4rip block Compression Ratio {:?} {:?}",
        name,
        compressed.len() as f64 / input.len() as f64
    );
    let decompressed = decompress(&compressed, input.len()).unwrap();
    assert_eq!(decompressed, input);

    let compressed = lz4_cpp_block_compress(input).unwrap();
    // println!("{:?}", compressed);
    println!(
        "Lz4 Cpp block Compression Ratio {:?} {:?}",
        name,
        compressed.len() as f64 / input.len() as f64
    );
    let decompressed = decompress(&compressed, input.len()).unwrap();

    assert_eq!(decompressed, input);

    let compressed = snap::raw::Encoder::new().compress_vec(input).unwrap();
    println!(
        "snap Compression Ratio {:?} {:?}",
        name,
        compressed.len() as f64 / input.len() as f64
    );

    #[cfg(feature = "frame")]
    {
        let mut frame_info = lz4rip::frame::FrameInfo::new();
        frame_info.block_mode = BlockMode::Independent;
        //frame_info.block_size = lz4rip::frame::BlockSize::Max4MB;
        let compressed = lz4rip_frame_compress_with(frame_info, input).unwrap();
        println!(
            "lz4rip frame indep Compression Ratio {:?} {:?}",
            name,
            compressed.len() as f64 / input.len() as f64
        );

        let mut frame_info = lz4rip::frame::FrameInfo::new();
        frame_info.block_mode = BlockMode::Linked;
        let compressed = lz4rip_frame_compress_with(frame_info, input).unwrap();
        println!(
            "lz4rip frame linked Compression Ratio {:?} {:?}",
            name,
            compressed.len() as f64 / input.len() as f64
        );

        let compressed = lz4_cpp_frame_compress(input, true).unwrap();
        println!(
            "lz4 cpp frame indep Compression Ratio {:?} {:?}",
            name,
            compressed.len() as f64 / input.len() as f64
        );

        let compressed = lz4_cpp_frame_compress(input, false).unwrap();
        println!(
            "lz4 cpp frame linked Compression Ratio {:?} {:?}",
            name,
            compressed.len() as f64 / input.len() as f64
        );
    }
}

// #[test]
// fn test_ratio() {
//     const COMPRESSION66K: &'static [u8] = include_bytes!("../corpus/compression_65k.txt");
//     let compressed = compress(COMPRESSION66K);
//     println!("Compression Ratio 66K {:?}", compressed.len() as f64/ COMPRESSION66K.len()  as
// f64);     let _decompressed = decompress(&compressed).unwrap();

//     let mut vec = Vec::with_capacity(10 + (COMPRESSION66K.len() as f64 * 1.1) as usize);
//     let input = COMPRESSION66K;

//     let bytes_written = compress_into_2(input, &mut vec, 256, 8).unwrap();
//     println!("dict size 256 {:?}", bytes_written as f64/ COMPRESSION66K.len()  as f64);
//     let bytes_written = compress_into_2(input, &mut vec, 512, 7).unwrap();
//     println!("dict size 512 {:?}", bytes_written as f64/ COMPRESSION66K.len()  as f64);
//     let bytes_written = compress_into_2(input, &mut vec, 1024, 6).unwrap();
//     println!("dict size 1024 {:?}", bytes_written as f64/ COMPRESSION66K.len()  as f64);
//     let bytes_written = compress_into_2(input, &mut vec, 2048, 5).unwrap();
//     println!("dict size 2048 {:?}", bytes_written as f64/ COMPRESSION66K.len()  as f64);
//     let bytes_written = compress_into_2(input, &mut vec, 4096, 4).unwrap();
//     println!("dict size 4096 {:?}", bytes_written as f64/ COMPRESSION66K.len()  as f64);
//     let bytes_written = compress_into_2(input, &mut vec, 8192, 3).unwrap();
//     println!("dict size 8192 {:?}", bytes_written as f64/ COMPRESSION66K.len()  as f64);
//     let bytes_written = compress_into_2(input, &mut vec, 16384, 2).unwrap();
//     println!("dict size 16384 {:?}", bytes_written as f64/ COMPRESSION66K.len()  as f64);
//     let bytes_written = compress_into_2(input, &mut vec, 32768, 1).unwrap();
//     println!("dict size 32768 {:?}", bytes_written as f64/ COMPRESSION66K.len()  as f64);

//     // let bytes_written = compress_into_2(input, &mut vec).unwrap();

// }

#[cfg(test)]
mod checked_decode {
    use super::*;

    fn decompress_with_size_prefix(data: &[u8]) -> Result<Vec<u8>, lz4rip::block::DecompressError> {
        if data.len() < 4 {
            return decompress(data, 0);
        }
        let size = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
        decompress(&data[4..], size)
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
}

#[test]
fn test_end_offset() {
    // the last 5 bytes need to be literals, so the last match block is not allowed to match to the
    // end
    test_roundtrip("AAAAAAAAAAAAAAAAAAAAAAAAaAAAAAAAAAAAAAAAAAAAAAAAA");
    test_roundtrip("AAAAAAAAAAAAAAAAAAAAAAAABBBBBBBBBaAAAAAAAAAAAAAAAAAAAAAAAA");
}
#[test]
fn small_compressible_1() {
    test_roundtrip("AAAAAAAAAAAAAAAAAAAAAAAABBBBBBBBBaAAAAAAAAAAAAAAAAAAAAAAAABBBBBBBBBa");
}
#[test]
fn small_compressible_2() {
    test_roundtrip("AAAAAAAAAAAZZZZZZZZAAAAAAAA");
}

#[test]
fn small_compressible_3() {
    test_roundtrip("AAAAAAAAAAAZZZZZZZZAAAAAAAA");
}

#[test]
fn shakespear1() {
    test_roundtrip("to live or not to live");
}
#[test]
fn shakespear2() {
    test_roundtrip("Love is a wonderful terrible thing");
}
#[test]
fn shakespear3() {
    test_roundtrip("There is nothing either good or bad, but thinking makes it so.");
}
#[test]
fn shakespear4() {
    test_roundtrip("I burn, I pine, I perish.");
}

#[test]
fn text_text() {
    test_roundtrip("Save water, it doesn't grow on trees.");
    test_roundtrip("The panda bear has an amazing black-and-white fur.");
    test_roundtrip("The average panda eats as much as 9 to 14 kg of bamboo shoots a day.");
    test_roundtrip("You are 60% water. Save 60% of yourself!");
    test_roundtrip("To cute to die! Save the red panda!");
}

#[test]
fn not_compressible() {
    test_roundtrip("as6yhol.;jrew5tyuikbfewedfyjltre22459ba");
    test_roundtrip("jhflkdjshaf9p8u89ybkvjsdbfkhvg4ut08yfrr");
}
#[test]
fn short_1() {
    test_roundtrip("ahhd");
    test_roundtrip("ahd");
    test_roundtrip("x-29");
    test_roundtrip("x");
    test_roundtrip("k");
    test_roundtrip(".");
    test_roundtrip("ajsdh");
    test_roundtrip("aaaaaa");
}

#[test]
fn short_2() {
    test_roundtrip("aaaaaabcbcbcbc");
}

#[test]
fn empty_string() {
    test_roundtrip("");
}

#[test]
fn nulls() {
    test_roundtrip("\0\0\0\0\0\0\0\0\0\0\0\0\0");
}

#[test]
fn bug_fuzz() {
    let data = &[
        8, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 46, 0, 0, 8, 0, 138,
    ];
    test_roundtrip(data);
}
#[test]
fn bug_fuzz_2() {
    let data = &[
        122, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 65, 0, 0, 128, 10, 1, 10, 1, 0, 122,
    ];
    test_roundtrip(data);
}
#[test]
fn bug_fuzz_3() {
    let data = &[
        36, 16, 0, 0, 79, 177, 176, 176, 171, 1, 0, 255, 207, 79, 79, 79, 79, 79, 1, 1, 49, 0, 16,
        0, 79, 79, 79, 79, 79, 1, 0, 255, 36, 79, 79, 79, 79, 79, 1, 0, 255, 207, 79, 79, 79, 79,
        79, 1, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 8, 207, 1, 207, 207, 79, 199,
        79, 79, 40, 79, 1, 1, 1, 1, 1, 1, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
        15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 79, 15, 15, 14, 15, 15, 15, 15, 15, 15,
        15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 61, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 0,
        48, 45, 0, 1, 0, 0, 1, 0,
    ];
    test_roundtrip(data);
}
#[test]
fn bug_fuzz_4() {
    let data = &[147];
    test_roundtrip(data);
}
#[test]
fn buf_fuzz_5() {
    let data = &[
        255, 255, 255, 255, 253, 235, 156, 140, 8, 0, 140, 45, 169, 0, 27, 128, 48, 0, 140, 0, 0,
        255, 255, 255, 253, 235, 156, 140, 8, 61, 255, 255, 255, 255, 65, 239, 254,
    ];

    test_roundtrip(data);
}

#[test]
fn bug_fuzz_6() {
    let data = &[
        181, 181, 181, 181, 181, 147, 147, 147, 0, 0, 255, 218, 44, 0, 177, 44, 0, 233, 177, 74,
        85, 47, 95, 146, 189, 177, 1, 0, 255, 2, 109, 180, 255, 255, 0, 0, 0, 181, 181, 181, 147,
        147, 147, 0, 0, 255, 218, 146, 146, 181, 0, 0, 181,
    ];

    test_roundtrip(data);
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

#[test]
fn test_so_many_zeros() {
    let data: Vec<u8> = iter::repeat_n(0, 30_000).collect();
    test_roundtrip(data);
}

#[test]
fn compression_works() {
    let s = r#"An iterator that knows its exact length.
        Many Iterators don't know how many times they will iterate, but some do. If an iterator knows how many times it can iterate, providing access to that information can be useful. For example, if you want to iterate backwards, a good start is to know where the end is.
        When implementing an ExactSizeIterator, you must also implement Iterator. When doing so, the implementation of size_hint must return the exact size of the iterator.
        The len method has a default implementation, so you usually shouldn't implement it. However, you may be able to provide a more performant implementation than the default, so overriding it in this case makes sense."#;

    test_roundtrip(s);
    assert!(compress_block(s.as_bytes()).len() < s.len());
}

// #[test]
// fn multi_compress() {
//     let s1 = r#"An iterator that knows its exact length.performant implementation than the
// default, so overriding it in this case makes sense."#;     let s2 = r#"An iterator that knows its
// exact length.performant implementation than the default, so overriding it in this case makes
// sense."#;     let mut out = vec![];
//     compress_into()
//     inverse(s);
//     assert!(compress(s.as_bytes()).len() < s.len());
// }

#[ignore]
#[test]
fn big_compression() {
    let mut s = Vec::with_capacity(80_000_000);

    for n in 0..80_000_000 {
        s.push((n as u8).wrapping_mul(0xA).wrapping_add(33) ^ 0xA2);
    }

    test_roundtrip(s);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_dickens() {
    test_roundtrip(&*DICKENS);
}
#[test]
fn test_json_66k() {
    test_roundtrip(COMPRESSION66JSON);
}
#[test]
fn test_text_65k() {
    test_roundtrip(COMPRESSION65);
}
#[test]
fn test_text_34k() {
    test_roundtrip(COMPRESSION34K);
}

#[test]
fn test_text_1k() {
    test_roundtrip(COMPRESSION1K);
}

#[test]
fn compressor_decompressor_debug() {
    use lz4rip::block::{Compressor, Decompressor};
    let comp = Compressor::new();
    let dbg = format!("{comp:?}");
    assert!(dbg.contains("Compressor"), "{dbg}");

    let decomp = Decompressor::with_dict(b"test");
    let dbg = format!("{decomp:?}");
    assert!(dbg.contains("Decompressor"), "{dbg}");
}

#[test]
fn error_types_are_clone_eq() {
    use lz4rip::block::DecompressError;
    let e1 = DecompressError::OffsetZero;
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}

use proptest::{prelude::*, test_runner::FileFailurePersistence};

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("regressions"))),
        ..Default::default()
    })]

    #[test]
    #[cfg_attr(miri, ignore)]
    fn proptest_roundtrip(v in vec_of_vec()) {
        let data: Vec<u8>  = v.iter().flat_map(|v|v.iter()).cloned().collect::<Vec<_>>();
        test_roundtrip(data);  // sum of the sum of all vectors.
    }

}

fn vec_of_vec() -> impl Strategy<Value = Vec<Vec<u8>>> {
    const N: u8 = 200;

    let length = 0..N;
    length.prop_flat_map(vec_from_length)
}

fn vec_from_length(length: u8) -> impl Strategy<Value = Vec<Vec<u8>>> {
    const K: usize = u8::MAX as usize;
    let mut result = vec![];
    for index in 1..length {
        let inner = proptest::collection::vec(0..index, 0..K);
        result.push(inner);
    }
    result
}

#[cfg(feature = "frame")]
mod frame {
    use lz4rip::frame::BlockSize;

    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn concatenated() {
        let mut enc = lz4rip::frame::FrameEncoder::new(Vec::new());
        enc.write_all(COMPRESSION1K).unwrap();
        enc.try_finish().unwrap();
        enc.write_all(COMPRESSION34K).unwrap();
        let compressed = enc.finish().unwrap();

        let mut dec = lz4rip::frame::FrameDecoder::new(&*compressed);
        let mut uncompressed = Vec::new();
        dec.read_to_end(&mut uncompressed).unwrap();
        assert_eq!(&*uncompressed, COMPRESSION1K);
        uncompressed.clear();
        dec.read_to_end(&mut uncompressed).unwrap();
        assert_eq!(&*uncompressed, COMPRESSION34K);
    }

    #[test]
    fn checksums() {
        for &input in &[COMPRESSION34K, COMPRESSION66JSON] {
            // Block checksum
            let mut frame_info = lz4rip::frame::FrameInfo::new();
            frame_info.block_checksums = true;
            let mut compressed = lz4rip_frame_compress_with(frame_info, input).unwrap();
            // roundtrip
            let uncompressed = lz4rip_frame_decompress(&compressed).unwrap();
            assert_eq!(uncompressed, input);
            // corrupt last block checksum, which is at 8th to 4th last bytes of the compressed
            // output
            let compressed_len = compressed.len();
            compressed[compressed_len - 5] ^= 0xFF;
            match lz4rip_frame_decompress(&compressed) {
                Err(lz4rip::frame::Error::BlockChecksumError) => (),
                r => panic!("{:?}", r),
            }

            // Content checksum
            let mut frame_info = lz4rip::frame::FrameInfo::new();
            frame_info.content_checksum = true;
            let mut compressed = lz4rip_frame_compress_with(frame_info, input).unwrap();
            // roundtrip
            let uncompressed = lz4rip_frame_decompress(&compressed).unwrap();
            assert_eq!(uncompressed, input);

            // corrupt content checksum, which is the last 4 bytes of the compressed output
            let compressed_len = compressed.len();
            compressed[compressed_len - 1] ^= 0xFF;
            match lz4rip_frame_decompress(&compressed) {
                Err(lz4rip::frame::Error::ContentChecksumError) => (),
                r => panic!("{:?}", r),
            }
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn block_size() {
        let mut last_compressed_len = usize::MAX;
        for block_size in &[
            BlockSize::Max64KB,
            BlockSize::Max256KB,
            BlockSize::Max1MB,
            BlockSize::Max4MB,
        ] {
            let mut frame_info = lz4rip::frame::FrameInfo::new();
            frame_info.block_size = *block_size;
            let compressed = lz4rip_frame_compress_with(frame_info, &DICKENS).unwrap();

            // roundtrip
            let uncompressed = lz4rip_frame_decompress(&compressed).unwrap();
            assert_eq!(uncompressed, *DICKENS);

            // For a large enough input (eg. a large input like dickens (10 MB)) we should get strictly
            // better compression by increasing the block size.
            assert!(compressed.len() < last_compressed_len);
            last_compressed_len = compressed.len();
        }
    }

    #[test]
    fn content_size() {
        let mut frame_info = lz4rip::frame::FrameInfo::new();
        frame_info.content_size = Some(COMPRESSION1K.len() as u64);
        let mut compressed = lz4rip_frame_compress_with(frame_info, COMPRESSION1K).unwrap();

        // roundtrip
        let uncompressed = lz4rip_frame_decompress(&compressed).unwrap();
        assert_eq!(uncompressed, COMPRESSION1K);

        // corrupt the len in the compressed bytes
        {
            // We'll generate a valid FrameInfo and copy it to the test data
            let mut frame_info = lz4rip::frame::FrameInfo::new();
            frame_info.content_size = Some(3);
            let dummy_compressed = lz4rip_frame_compress_with(frame_info, b"123").unwrap();
            // `15` (7 + 8) is the size of the header plus the content size in the compressed bytes
            compressed[..15].copy_from_slice(&dummy_compressed[..15]);
        }
        match lz4rip_frame_decompress(&compressed) {
            Err(lz4rip::frame::Error::ContentLengthError { expected, actual }) => {
                assert_eq!(expected, 3);
                assert_eq!(actual, 725);
            }
            r => panic!("{:?}", r),
        }
    }

    #[test]
    fn dict_round_trip() {
        let dict = b"JSON schema v1 field name= value= type= len= ".repeat(4);
        let dict_id: u32 = 0xDEADBEEF;
        let msg = b"JSON schema v1 field name=hello value=world type=str len=5";

        let mut enc = lz4rip::frame::FrameEncoder::with_dictionary(Vec::new(), &dict, dict_id);
        enc.write_all(msg).unwrap();
        let compressed = enc.finish().unwrap();

        // Frame magic and Dict_ID flag must be set in FLG.
        assert_eq!(&compressed[..4], &[0x04, 0x22, 0x4d, 0x18]);

        let mut dec = lz4rip::frame::FrameDecoder::with_dictionary(&*compressed, &dict, dict_id);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        assert_eq!(out, msg);
    }

    #[test]
    fn dict_id_mismatch_fails() {
        let dict = b"prefix AAA ".repeat(8);
        let msg = b"prefix AAA tail";
        let mut enc = lz4rip::frame::FrameEncoder::with_dictionary(Vec::new(), &dict, 0xAAAA_AAAA);
        enc.write_all(msg).unwrap();
        let compressed = enc.finish().unwrap();

        let mut dec =
            lz4rip::frame::FrameDecoder::with_dictionary(&*compressed, &dict, 0xBBBB_BBBB);
        let mut out = Vec::new();
        let err = dec.read_to_end(&mut out).unwrap_err();
        let inner = err
            .into_inner()
            .and_then(|e| e.downcast::<lz4rip::frame::Error>().ok());
        match inner.as_deref() {
            Some(lz4rip::frame::Error::DictIdMismatch { .. }) => {}
            other => panic!("expected DictIdMismatch, got {other:?}"),
        }
    }

    #[test]
    fn dict_required_when_frame_declares_one() {
        let dict = b"common ".repeat(8);
        let mut enc = lz4rip::frame::FrameEncoder::with_dictionary(Vec::new(), &dict, 1);
        enc.write_all(b"common payload").unwrap();
        let compressed = enc.finish().unwrap();

        let mut dec = lz4rip::frame::FrameDecoder::new(&*compressed);
        let mut out = Vec::new();
        let err = dec.read_to_end(&mut out).unwrap_err();
        let inner = err
            .into_inner()
            .and_then(|e| e.downcast::<lz4rip::frame::Error>().ok());
        assert!(matches!(
            inner.as_deref(),
            Some(lz4rip::frame::Error::DictionaryNotSupported)
        ));
    }

    #[test]
    fn truncated_standard_frame_is_error() {
        let frame_info = lz4rip::frame::FrameInfo::new();
        let compressed = lz4rip_frame_compress_with(frame_info, COMPRESSION34K).unwrap();
        // Chop off the last 4 bytes (EndMark) to simulate truncation.
        let truncated = &compressed[..compressed.len() - 4];
        let err = lz4rip_frame_decompress(truncated).unwrap_err();
        assert!(
            matches!(err, lz4rip::frame::Error::IoError(_)),
            "expected IoError(UnexpectedEof), got {err:?}"
        );
    }

    #[test]
    fn try_finish_idempotent() {
        let mut enc = lz4rip::frame::FrameEncoder::new(Vec::new());
        std::io::Write::write_all(&mut enc, b"hello").unwrap();
        enc.try_finish().unwrap();
        let first = enc.get_ref().clone();
        enc.try_finish().unwrap();
        assert_eq!(
            enc.get_ref(),
            &first,
            "double try_finish must not append bytes"
        );
    }
}

#[cfg(test)]
mod test_dict_corpus {
    use lz4rip::block::{compress, Compressor, Decompressor};

    const HDFS: &[u8] = include_bytes!("../corpus/hdfs.json");
    const JSON66K: &[u8] = include_bytes!("../corpus/compression_66k_JSON.txt");

    fn dict_roundtrip(corpus: &[u8], dict_bytes: usize) {
        let dict = &corpus[..dict_bytes];
        let input = &corpus[dict_bytes..];

        let mut comp = Compressor::with_dict(dict);
        let compressed = comp.compress(input);
        let decomp = Decompressor::with_dict(dict);
        let decompressed = decomp.decompress(&compressed, input.len()).unwrap();
        assert_eq!(input, &decompressed[..]);

        // Dict should help: compressed size strictly smaller than without dict.
        let no_dict = compress(input);
        assert_lt!(compressed.len(), no_dict.len());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn hdfs_dict_4k() {
        dict_roundtrip(HDFS, 4096);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn hdfs_dict_64k() {
        dict_roundtrip(HDFS, 65536);
    }

    #[test]
    fn json66k_dict_4k() {
        dict_roundtrip(JSON66K, 4096);
    }
}

#[cfg(test)]
mod test_compression {
    use super::*;

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

    mod lz4_linked {
        use super::*;
        fn get_compressed_size(input: &[u8]) -> usize {
            let output = lz4_cpp_block_compress(input).unwrap();
            output.len()
        }

        #[test]
        #[cfg_attr(miri, ignore)]
        fn test_comp_lz4_linked() {
            print_ratio(
                "Ratio 1k C",
                COMPRESSION1K.len(),
                get_compressed_size(COMPRESSION1K),
            );
            print_ratio(
                "Ratio 34k C",
                COMPRESSION34K.len(),
                get_compressed_size(COMPRESSION34K),
            );
        }
    }
}
