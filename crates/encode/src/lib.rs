//! LZ4 block compression.

#![deny(warnings)]
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "nightly", feature(optimize_attribute))]

extern crate alloc;

#[forbid(unsafe_code)]
mod compress;
#[forbid(unsafe_code)]
mod dict;
pub(crate) mod hashtable;
mod verified_sink;

pub use compress::{compress, compress_into, get_maximum_output_size, Compressor};
pub use dict::DictTrainer;
pub use lz4rip_core::CompressError;

// Internal items needed by the lz4rip facade crate for the frame module.
#[doc(hidden)]
pub use compress::{compress_internal, compress_into_sink_with_dict, write_integer};
#[doc(hidden)]
pub use hashtable::{HashTable, HashTableU32};
#[doc(hidden)]
pub use verified_sink::VerifiedSliceSink;
