#![allow(dead_code)]

#[cfg(feature = "frame")]
use lz4rip::frame::BlockMode;
use lz4rip::{block::decompress, compress as compress_block};

pub const COMPRESSION1K: &[u8] = include_bytes!("../../corpus/compression_1k.txt");
pub const COMPRESSION34K: &[u8] = include_bytes!("../../corpus/compression_34k.txt");
pub const COMPRESSION65: &[u8] = include_bytes!("../../corpus/compression_65k.txt");
pub const COMPRESSION66JSON: &[u8] = include_bytes!("../../corpus/compression_66k_JSON.txt");

pub static DICKENS: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
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

pub fn test_roundtrip(bytes: impl AsRef<[u8]>) {
    let bytes = bytes.as_ref();
    let compressed = compress_block(bytes);
    let decompressed = decompress(&compressed, bytes.len()).unwrap();
    assert_eq!(decompressed, bytes);

    #[cfg(feature = "frame")]
    for bm in &[BlockMode::Independent, BlockMode::Linked] {
        let mut frame_info = lz4rip::frame::FrameInfo::new();
        frame_info.block_mode = *bm;
        let compressed = lz4rip_frame_compress_with(frame_info, bytes).unwrap();
        let decompressed = lz4rip_frame_decompress(&compressed).unwrap();
        assert_eq!(decompressed, bytes);
    }
}
