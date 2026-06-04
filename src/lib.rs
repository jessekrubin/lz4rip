//! Fast Rust implementation of LZ4 compression.
//!
//! # Overview
//!
//! This crate provides two ways to use lz4. The first is through
//! [`frame::FrameDecoder`] and [`frame::FrameEncoder`],
//! which implement the `std::io::Read` and `std::io::Write` traits with the
//! lz4 frame format. The frame format supports streaming compression and
//! decompression.
//!
//! The second way is through the
//! [`decompress_size_prepended`](block/fn.decompress_size_prepended.html)
//! and
//! [`compress_prepend_size`](block/fn.compress_prepend_size.html)
//! functions. These functions provide access to the lz4 block format, and
//! don't support a streaming interface directly. You should only use these types
//! if you know you specifically need the lz4 block format.
//!
//! # Example: compress data on `stdin` with frame format
//! This program reads data from `stdin`, compresses it and emits it to `stdout`.
//! ```no_run
//! # #[cfg(feature = "frame")]
//! # {
//! use std::io;
//! let stdin = io::stdin();
//! let stdout = io::stdout();
//! let mut rdr = stdin.lock();
//! // Wrap the stdout writer in a LZ4 Frame writer.
//! let mut wtr = lz4rip::frame::FrameEncoder::new(stdout.lock());
//! io::copy(&mut rdr, &mut wtr).expect("I/O operation failed");
//! wtr.finish().unwrap();
//! # }
//! ```
//! # Example: decompress data on `stdin` with frame format
//! This program reads data from `stdin`, decompresses it and emits it to `stdout`.
//! ```no_run
//! # #[cfg(feature = "frame")]
//! # {
//! use std::io;
//! let stdin = io::stdin();
//! let stdout = io::stdout();
//! // Wrap the stdin reader in a LZ4 FrameDecoder.
//! let mut rdr = lz4rip::frame::FrameDecoder::new(stdin.lock());
//! let mut wtr = stdout.lock();
//! io::copy(&mut rdr, &mut wtr).expect("I/O operation failed");
//! # }
//! ```
//!
//! # Example: block format roundtrip
//! ```
//! use lz4rip::block::{compress_prepend_size, decompress_size_prepended};
//! let input: &[u8] = b"Hello people, what's up?";
//! let compressed = compress_prepend_size(input);
//! let uncompressed = decompress_size_prepended(&compressed).unwrap();
//! assert_eq!(input, uncompressed);
//! ```
//!
//! ## Feature Flags
//!
//! - `frame` support for LZ4 frame format. _implies `std`, enabled by default_
//! - `std` enables dependency on the standard library. _enabled by default_
//!
//! For no_std support only the [`block format`](block/index.html) is supported.
//!
//!
#![deny(warnings)]
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(feature = "nightly", feature(optimize_attribute))]
#![cfg_attr(feature = "nightly", feature(core_intrinsics))]
#![cfg_attr(feature = "nightly", allow(internal_features, unused_features))]

#[cfg_attr(test, macro_use)]
extern crate alloc;

#[cfg(test)]
#[macro_use]
extern crate more_asserts;

pub mod block;
#[cfg(feature = "frame")]
#[cfg_attr(docsrs, doc(cfg(feature = "frame")))]
pub mod frame;

#[allow(dead_code)]
mod fastcpy;

pub use block::{compress, compress_into, compress_prepend_size, get_maximum_output_size};
pub use block::{decompress, decompress_into, decompress_size_prepended};

#[forbid(unsafe_code)]
pub(crate) mod sink;
pub(crate) mod verified_sink;
