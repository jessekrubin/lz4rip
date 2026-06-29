//! Owning block compressors (alloc).
#![forbid(unsafe_code)]

use core::fmt;

use alloc::vec::Vec;

use crate::compress::{
    compress_dict_tables, get_maximum_output_size, init_dict, CompressorRef, HashTableU32U16,
};
use lz4rip_core::CompressError;
use lz4rip_core::{MINMATCH, WINDOW_SIZE};

/// A reusable block compressor that owns no dictionary.
///
/// This is the ergonomic owning API for use with `alloc`. For a no-alloc
/// variant, see [`CompressorRef`](crate::CompressorRef). For a dictionary-seeded
/// compressor, see [`DictCompressor`].
///
/// For one-shot compression, use [`compress`](crate::compress) or
/// [`compress_into`](crate::compress_into) instead.
///
/// # Example
/// ```
/// use lz4rip_encode::{Compressor, get_maximum_output_size};
///
/// let mut comp = Compressor::new();
/// let input = b"hello world, hello world, hello!";
/// let mut output = vec![0u8; get_maximum_output_size(input.len())];
/// let compressed_len = comp.compress_into(input, &mut output).unwrap();
/// ```
#[derive(Debug, Default)]
pub struct Compressor {
    inner: CompressorRef,
}

impl Compressor {
    /// Create a new compressor without a dictionary.
    #[must_use]
    pub fn new() -> Self {
        Compressor {
            inner: CompressorRef::new(),
        }
    }

    /// Compress `input` into `output`, returning the number of compressed bytes.
    ///
    /// `output` must be at least [`get_maximum_output_size`](crate::get_maximum_output_size)`(input.len())` bytes.
    pub fn compress_into(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CompressError> {
        self.inner.compress_into(input, output)
    }

    /// Compress `input` into a new `Vec<u8>`.
    pub fn compress(&mut self, input: &[u8]) -> Vec<u8> {
        self.inner.compress(input)
    }
}

/// A reusable block compressor that owns its dictionary.
///
/// This is the ergonomic owning dict API for use with `alloc`. For a no-alloc
/// variant that borrows the dictionary, see
/// [`DictCompressorRef`](crate::DictCompressorRef).
///
/// Unlike the previous self-referential design, this owns its hash tables and
/// dictionary as sibling fields and needs no `unsafe`.
///
/// # Example
/// ```
/// use lz4rip_encode::{DictCompressor, get_maximum_output_size};
///
/// let dict = b"the quick brown fox";
/// let mut comp = DictCompressor::new(dict);
/// let input = b"the quick brown fox jumps";
/// let mut output = vec![0u8; get_maximum_output_size(input.len())];
/// let compressed_len = comp.compress_into(input, &mut output).unwrap();
/// ```
pub struct DictCompressor {
    /// Trimmed dictionary bytes (empty when the dictionary was shorter than
    /// [`MINMATCH`]).
    dict: Vec<u8>,
    table: HashTableU32U16,
    pristine: HashTableU32U16,
}

impl fmt::Debug for DictCompressor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DictCompressor")
            .field("dict_len", &self.dict.len())
            .finish()
    }
}

impl DictCompressor {
    /// Create a new compressor seeded with an external dictionary.
    ///
    /// The dictionary is cloned into owned storage. If `dict` is longer than the
    /// LZ4 window it is trimmed to the last [`WINDOW_SIZE`] bytes. A dictionary
    /// shorter than 4 bytes is ignored (no dict matches); use [`Compressor`] for
    /// that case.
    #[must_use]
    pub fn new(dict: &[u8]) -> Self {
        let trimmed = if dict.len() < MINMATCH {
            b"".as_slice()
        } else if dict.len() > WINDOW_SIZE {
            &dict[dict.len() - WINDOW_SIZE..]
        } else {
            dict
        };
        let mut pristine = HashTableU32U16::new();
        let mut dict_ref = trimmed;
        init_dict(&mut pristine, &mut dict_ref);
        DictCompressor {
            dict: trimmed.to_vec(),
            table: HashTableU32U16::new(),
            pristine,
        }
    }

    /// Compress `input` into `output`, returning the number of compressed bytes.
    ///
    /// `output` must be at least [`get_maximum_output_size`](crate::get_maximum_output_size)`(input.len())` bytes.
    pub fn compress_into(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CompressError> {
        // Split the borrow so the dict and tables can be passed separately.
        let DictCompressor {
            dict,
            table,
            pristine,
        } = self;
        compress_dict_tables(table, pristine, dict, input, output)
    }

    /// Compress `input` into a new `Vec<u8>`.
    pub fn compress(&mut self, input: &[u8]) -> Vec<u8> {
        let max = get_maximum_output_size(input.len());
        let mut out = alloc::vec![0u8; max];
        let n = self.compress_into(input, &mut out).unwrap();
        out.truncate(n);
        out
    }
}
