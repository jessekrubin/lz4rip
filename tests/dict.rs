use lz4rip::block::{compress, Compressor, Decompressor};
use more_asserts::assert_lt;

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
