use lz4rip::block::{compress, Compressor, Decompressor, DictTrainer};
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

#[test]
fn dict_trainer_zero_samples() {
    let trainer = DictTrainer::new(2048);
    assert!(trainer.train().is_empty());
}

#[test]
fn dict_trainer_one_sample() {
    let mut trainer = DictTrainer::new(2048);
    trainer.add_sample(b"hello world");
    assert!(trainer.train().is_empty());
}

#[test]
fn dict_trainer_short_samples_skipped() {
    let mut trainer = DictTrainer::new(2048);
    trainer.add_sample(b"ab");
    trainer.add_sample(b"cd");
    assert_eq!(trainer.sample_count(), 0);
}

#[test]
fn dict_trainer_oversized_sample_skipped() {
    let mut trainer = DictTrainer::new(16);
    let big = vec![b'x'; 32];
    trainer.add_sample(&big);
    assert_eq!(trainer.sample_count(), 0);
}

#[test]
fn dict_trainer_produces_usable_dict() {
    let mut trainer = DictTrainer::new(2048);
    for i in 0..100u8 {
        let sample = format!("message id={i} payload=the quick brown fox");
        trainer.add_sample(sample.as_bytes());
    }
    let dict = trainer.train();
    assert!(!dict.is_empty());
    assert!(dict.len() <= 2048);

    let input = b"message id=42 payload=the quick brown fox jumps";
    let mut comp = Compressor::with_dict(&dict);
    let compressed = comp.compress(input);
    let decomp = Decompressor::with_dict(&dict);
    let decompressed = decomp.decompress(&compressed, input.len()).unwrap();
    assert_eq!(&decompressed[..], input);
}
