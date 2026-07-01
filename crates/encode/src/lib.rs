//! LZ4 block compression.

#![deny(warnings)]
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "nightly", feature(optimize_attribute))]
#![cfg_attr(feature = "paranoid", forbid(unsafe_code))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[forbid(unsafe_code)]
mod compress;
#[cfg(feature = "alloc")]
mod compressor;
#[cfg(feature = "alloc")]
#[forbid(unsafe_code)]
mod dict;
pub(crate) mod hashtable;
mod verified_sink;

#[cfg(feature = "alloc")]
pub use compress::compress;
pub use compress::{
    CompressorRef, CompressorRefN, DEFAULT_DICT_ENTRIES, DEFAULT_NODICT_ENTRIES, DictCompressorRef,
    DictCompressorRefN, MIN_ENTRIES, compress_into, compress_into_with_dict,
    get_maximum_output_size,
};
#[cfg(feature = "alloc")]
pub use compressor::{Compressor, CompressorN, DictCompressor, DictCompressorN};
#[cfg(feature = "alloc")]
pub use dict::DictTrainer;
pub use lz4rip_core::CompressError;

// Cross-crate plumbing for the lz4rip facade (frame module + tests).
// Public for workspace access but not part of the stable API.
#[doc(hidden)]
pub use compress::{compress_into_sink_with_dict, compress_into_sink_with_table, write_integer};
#[doc(hidden)]
pub use hashtable::HashTableU32;
