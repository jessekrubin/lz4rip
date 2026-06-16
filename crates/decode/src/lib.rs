//! LZ4 block decompression.

#![deny(warnings)]
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[forbid(unsafe_code)]
mod decompress;
pub(crate) mod primitives;

pub use decompress::{decompress, decompress_into, Decompressor};
pub use lz4rip_core::DecompressError;

// Internal items needed by the lz4rip facade crate for the frame module.
#[doc(hidden)]
pub use decompress::{decompress_internal, read_integer};
