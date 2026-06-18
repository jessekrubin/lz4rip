mod common;

use common::*;
use std::iter;

#[test]
fn test_end_offset() {
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
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 46, 0, 0, 8, 0, 138,
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
    assert!(lz4rip::compress(s.as_bytes()).len() < s.len());
}

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

use proptest::{prelude::*, test_runner::FileFailurePersistence};

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

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("regressions"))),
        ..Default::default()
    })]

    #[test]
    #[cfg_attr(miri, ignore)]
    fn proptest_roundtrip(v in vec_of_vec()) {
        let data: Vec<u8> = v.iter().flat_map(|v| v.iter()).cloned().collect::<Vec<_>>();
        test_roundtrip(data);
    }
}
