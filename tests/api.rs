use lz4rip::block::{Compressor, DecompressError, Decompressor};

#[test]
fn compressor_decompressor_debug() {
    let comp = Compressor::new();
    let dbg = format!("{comp:?}");
    assert!(dbg.contains("Compressor"), "{dbg}");

    let decomp = Decompressor::with_dict(b"test");
    let dbg = format!("{decomp:?}");
    assert!(dbg.contains("Decompressor"), "{dbg}");
}

#[test]
fn error_types_are_clone_eq() {
    let e1 = DecompressError::OffsetZero;
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}
