#[allow(unused)]
use std::io::{Read, Write};

use binggan::plugins::*;
use binggan::*;

extern crate lz4_flex_upstream;

const HDFS: &[u8] = include_bytes!("../corpus/hdfs.json");

#[global_allocator]
pub static GLOBAL: PeakMemAlloc<jemallocator::Jemalloc> = PeakMemAlloc::new(jemallocator::Jemalloc);

const MAIN_CORPUS: &[&str] = &[
    "silesia/dickens",
    "silesia/mozilla",
    "silesia/mr",
    "silesia/nci",
    "silesia/ooffice",
    "silesia/osdb",
    "silesia/reymont",
    "silesia/samba",
    "silesia/sao",
    "silesia/webster",
    "silesia/x-ray",
    "silesia/xml",
    "hdfs.json",
];

#[cfg(feature = "frame")]
type FrameDictInput = (Vec<u8>, Vec<u8>);

#[cfg(feature = "frame")]
const FRAME_DICT_ID: u32 = 1;
#[cfg(feature = "frame")]
const FRAME_DICT_BLOCK_BYTES: usize = 64 * 1024;
#[cfg(feature = "frame")]
const FRAME_DICT_REPEAT_START: usize = FRAME_DICT_BLOCK_BYTES / 2;

fn main() {
    #[cfg(feature = "frame")]
    {
        let data_sets = get_frame_datasets();
        frame_decompress(&data_sets);
        frame_compress(InputGroup::new_with_inputs(data_sets));
        frame_dict_compress(InputGroup::new_with_inputs(get_frame_dict_datasets()));
    }

    let named_data = load_main_corpus();
    block_compress(InputGroup::new_with_inputs(named_data));
    block_decompress(load_main_corpus());
}

#[cfg(feature = "frame")]
fn frame_decompress(data_sets: &[(String, Vec<u8>)]) {
    let mut runner = BenchRunner::with_name("frame_decompress");
    runner.add_plugin(PeakMemAllocPlugin::new(&GLOBAL));
    for (name, data_set) in data_sets {
        let compressed_independent = lz4_cpp_frame_compress(data_set, true).unwrap();
        let compressed_linked = lz4_cpp_frame_compress(data_set, false).unwrap();
        let comp_snap = compress_snap_frame(data_set);
        let mut group = runner.new_group();
        group.set_name(name);
        group.set_input_size(data_set.len());

        group.register_with_input("lz4rip independent", &compressed_independent, move |i| {
            let out = black_box(lz4rip_frame_decompress(i).unwrap());
            out.len()
        });
        group.register_with_input("lz4 c90 independent", &compressed_independent, move |i| {
            let out = black_box(lz4_cpp_frame_decompress(i).unwrap());
            out.len()
        });
        group.register_with_input("lz4rip linked", &compressed_linked, move |i| {
            let out = black_box(lz4rip_frame_decompress(i).unwrap());
            out.len()
        });
        group.register_with_input("lz4 c90 linked", &compressed_linked, move |i| {
            let out = black_box(lz4_cpp_frame_decompress(i).unwrap());
            out.len()
        });
        group.register_with_input("snap", &comp_snap, move |i| {
            let out = black_box(decompress_snap_frame(i));
            out.len()
        });

        group.run();
    }
}

#[cfg(feature = "frame")]
fn frame_compress(mut runner: InputGroup<Vec<u8>, usize>) {
    runner.set_name("frame_compress");
    runner.add_plugin(PeakMemAllocPlugin::new(&GLOBAL));

    runner.throughput(|data| data.len());
    runner.register("lz4rip independent", move |i| {
        let mut frame_info = lz4rip::frame::FrameInfo::new();
        frame_info.block_size = lz4rip::frame::BlockSize::Max256KB;
        frame_info.block_mode = lz4rip::frame::BlockMode::Independent;
        let out = black_box(lz4rip_frame_compress_with(frame_info, i).unwrap());
        out.len()
    });
    runner.register("lz4 c90 indep", move |i| {
        let out = black_box(lz4_cpp_frame_compress(i, true).unwrap());
        out.len()
    });
    runner.register("lz4rip linked", move |i| {
        let mut frame_info = lz4rip::frame::FrameInfo::new();
        frame_info.block_size = lz4rip::frame::BlockSize::Max256KB;
        frame_info.block_mode = lz4rip::frame::BlockMode::Linked;
        let out = black_box(lz4rip_frame_compress_with(frame_info, i).unwrap());
        out.len()
    });
    runner.register("lz4 c90 linked", move |i| {
        let out = black_box(lz4_cpp_frame_compress(i, false).unwrap());
        out.len()
    });
    runner.register("snap", move |i| {
        let out = compress_snap_frame(i);
        out.len()
    });

    runner.run();
}

#[cfg(feature = "frame")]
fn frame_dict_compress(mut runner: InputGroup<FrameDictInput, usize>) {
    runner.set_name("frame_dict_compress");
    runner.add_plugin(PeakMemAllocPlugin::new(&GLOBAL));
    runner.throughput(|(data, _)| data.len());

    runner.register("lz4rip independent", move |(data, dict)| {
        let mut frame_info = lz4rip::frame::FrameInfo::new();
        frame_info.block_size = lz4rip::frame::BlockSize::Max64KB;
        frame_info.block_mode = lz4rip::frame::BlockMode::Independent;
        let mut enc = lz4rip::frame::FrameEncoder::with_dictionary(
            Vec::new(),
            dict,
            FRAME_DICT_ID,
            Some(frame_info),
        )
        .unwrap();
        enc.write_all(data).unwrap();
        let out = black_box(enc.finish().unwrap());
        out.len()
    });
    runner.register("lz4rip linked", move |(data, dict)| {
        let mut frame_info = lz4rip::frame::FrameInfo::new();
        frame_info.block_size = lz4rip::frame::BlockSize::Max64KB;
        frame_info.block_mode = lz4rip::frame::BlockMode::Linked;
        let mut enc = lz4rip::frame::FrameEncoder::with_dictionary(
            Vec::new(),
            dict,
            FRAME_DICT_ID,
            Some(frame_info),
        )
        .unwrap();
        enc.write_all(data).unwrap();
        let out = black_box(enc.finish().unwrap());
        out.len()
    });

    runner.run();
}

fn block_compress(mut runner: InputGroup<Vec<u8>, usize>) {
    runner.set_name("block_compress");
    // Set the peak mem allocator. This will enable peak memory reporting.
    runner.add_plugin(PeakMemAllocPlugin::new(&GLOBAL));

    runner.throughput(|data| data.len());
    runner.register("lz4rip", move |i| {
        let out = black_box(lz4rip::compress(i));
        out.len()
    });
    runner.register("lz4_flex (unsafe)", move |i| {
        let out = black_box(lz4_flex_block_compress(i));
        out.len()
    });
    runner.register("lz4 c90", move |i| {
        let out = black_box(lz4_cpp_block_compress(i).unwrap());
        out.len()
    });
    runner.register("snap", move |i| {
        let out = black_box(compress_snap(i));
        out.len()
    });

    runner.run();
}

fn block_decompress(data_sets: Vec<(String, Vec<u8>)>) {
    let mut runner = BenchRunner::with_name("block_decompress");
    // Set the peak mem allocator. This will enable peak memory reporting.
    runner.add_plugin(PeakMemAllocPlugin::new(&GLOBAL));
    runner.add_plugin(CacheTrasher::default());
    for (name, data_uncomp) in data_sets {
        let comp_lz4 = lz4_cpp_block_compress(&data_uncomp).unwrap();
        let bundle = (comp_lz4, data_uncomp.len());

        let mut group = runner.new_group();
        group.set_name(name.clone());
        group.set_input_size(data_uncomp.len());

        group.register_with_input("lz4rip", &bundle, move |i| {
            let size = black_box(lz4rip::decompress(&i.0, i.1).unwrap());
            size.len()
        });
        group.register_with_input("lz4_flex (unsafe)", &bundle, move |i| {
            let size = black_box(lz4_flex_block_decompress(&i.0, i.1));
            size.len()
        });
        group.register_with_input("lz4 c90", &bundle, move |i| {
            let size = black_box(lz4_cpp_block_decompress(&i.0, i.1).unwrap());
            size.len()
        });

        group.run();
    }
}

fn get_frame_datasets() -> Vec<(String, Vec<u8>)> {
    let paths = [
        "silesia/dickens",
        "hdfs.json",
        "silesia/reymont",
        "silesia/xml",
    ];
    paths
        .iter()
        .map(|path| {
            let path_buf = std::path::Path::new("corpus").join(path);
            let mut file = std::fs::File::open(path_buf).unwrap();
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).unwrap();
            (path.to_string(), buf)
        })
        .collect()
}

#[cfg(feature = "frame")]
fn get_frame_dict_datasets() -> Vec<(String, FrameDictInput)> {
    let dict = b"prefix=orders region=west status=complete payload=".repeat(32);
    let first_block = &HDFS[..FRAME_DICT_BLOCK_BYTES];
    let mut data = first_block.to_vec();
    data.extend_from_slice(&first_block[FRAME_DICT_REPEAT_START..]);
    data.extend_from_slice(&first_block[FRAME_DICT_REPEAT_START..]);

    vec![("linked-dict-repeat".to_string(), (data, dict))]
}

fn load_main_corpus() -> Vec<(String, Vec<u8>)> {
    MAIN_CORPUS
        .iter()
        .filter_map(|path| {
            let path_buf = std::path::Path::new("corpus").join(path);
            let data = match std::fs::read(&path_buf) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("skipping {}: {e}", path_buf.display());
                    return None;
                }
            };
            let name = path.rsplit('/').next().unwrap().to_string();
            Some((name, data))
        })
        .collect()
}

fn compress_snap(input: &[u8]) -> Vec<u8> {
    snap::raw::Encoder::new().compress_vec(input).unwrap()
}

#[cfg(feature = "frame")]
fn compress_snap_frame(input: &[u8]) -> Vec<u8> {
    let mut fe = snap::write::FrameEncoder::new(Vec::new());
    fe.write_all(input).unwrap();
    fe.into_inner().unwrap()
}

#[cfg(feature = "frame")]
fn decompress_snap_frame(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut fe = snap::read::FrameDecoder::new(input);
    fe.read_to_end(&mut out).unwrap();
    out
}

fn lz4_flex_block_compress(input: &[u8]) -> Vec<u8> {
    let max_out = lz4_flex_upstream::block::get_maximum_output_size(input.len());
    let mut out = vec![0u8; max_out];
    let len = lz4_flex_upstream::block::compress_into(input, &mut out).unwrap();
    out.truncate(len);
    out
}

fn lz4_flex_block_decompress(input: &[u8], decomp_len: usize) -> Vec<u8> {
    let mut out = vec![0u8; decomp_len];
    lz4_flex_upstream::block::decompress_into(input, &mut out).unwrap();
    out
}

fn lz4_cpp_block_decompress(input: &[u8], decomp_len: usize) -> Result<Vec<u8>, lzzzz::Error> {
    let mut out = vec![0u8; decomp_len];
    lzzzz::lz4::decompress(input, &mut out)?;
    Ok(out)
}

fn lz4_cpp_block_compress(input: &[u8]) -> Result<Vec<u8>, lzzzz::Error> {
    let mut out = Vec::new();
    lzzzz::lz4::compress_to_vec(input, &mut out, lzzzz::lz4::ACC_LEVEL_DEFAULT).unwrap();
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
        .block_size(lzzzz::lz4f::BlockSize::Max64KB)
        .build();
    let mut comp = lzzzz::lz4f::WriteCompressor::new(Vec::new(), pref).unwrap();
    comp.write_all(input).unwrap();
    let out = comp.into_inner();

    Ok(out)
}

#[cfg(feature = "frame")]
fn lz4_cpp_frame_decompress(mut input: &[u8]) -> Result<Vec<u8>, lzzzz::lz4f::Error> {
    let mut r = lzzzz::lz4f::ReadDecompressor::new(&mut input)?;
    let mut buf = Vec::new();
    r.read_to_end(&mut buf).unwrap();

    Ok(buf)
}

#[cfg(feature = "frame")]
pub fn lz4rip_frame_compress_with(
    frame_info: lz4rip::frame::FrameInfo,
    input: &[u8],
) -> Result<Vec<u8>, lz4rip::frame::Error> {
    let buffer = Vec::new();
    let mut enc = lz4rip::frame::FrameEncoder::with_frame_info(frame_info, buffer);
    enc.write_all(input)?;
    enc.finish()
}

#[cfg(feature = "frame")]
pub fn lz4rip_frame_decompress(input: &[u8]) -> Result<Vec<u8>, lz4rip::frame::Error> {
    let mut de = lz4rip::frame::FrameDecoder::new(input);
    let mut out = Vec::new();
    de.read_to_end(&mut out)?;
    Ok(out)
}
