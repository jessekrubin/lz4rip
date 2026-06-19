use core::fmt;

use alloc::vec::Vec;

use crate::compress::CompressorRef;
use lz4rip_core::CompressError;

/// A reusable block compressor that owns its dictionary.
///
/// This is the ergonomic API for use with `alloc`. For a no-alloc variant that
/// borrows the dictionary, see [`CompressorRef`].
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
pub struct Compressor {
    // SAFETY invariants (self-referential struct):
    //   `inner` may hold a `&[u8]` fabricated via `from_raw_parts` pointing
    //   into `dict`'s heap buffer. Sound because:
    //   1. `inner` is declared before `dict` → dropped first (Rust field order).
    //   2. `dict` is private and never reallocated after construction.
    //   3. `CompressorRef` has no Drop impl that accesses the slice.
    //   4. No Clone/Copy impl exists. Cloning would copy the Vec (new alloc)
    //      but `inner` would still point at the original buffer → UB on drop.
    //   5. No method exposes `inner` by value or mutates `dict`.
    inner: CompressorRef<'static>,
    #[allow(dead_code)]
    dict: Vec<u8>,
}

impl fmt::Debug for Compressor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Compressor")
            .field("dict_len", &self.dict.len())
            .finish()
    }
}

impl Compressor {
    /// Create a new compressor without a dictionary.
    pub fn new() -> Self {
        Compressor {
            inner: CompressorRef::new(),
            dict: Vec::new(),
        }
    }

    /// Create a new compressor seeded with an external dictionary.
    ///
    /// The dictionary is cloned into owned storage.
    /// If `dict` is shorter than 4 bytes, it is ignored.
    pub fn with_dict(dict: &[u8]) -> Self {
        let dict = dict.to_vec();
        // SAFETY: We create a &'static [u8] pointing into `dict`'s heap buffer.
        // Sound because `dict` is stored in self, never reallocated, and `inner`
        // (which holds the reference) is dropped before `dict` per field order.
        let dict_ref: &'static [u8] =
            unsafe { core::slice::from_raw_parts(dict.as_ptr(), dict.len()) };
        Compressor {
            inner: CompressorRef::with_dict(dict_ref),
            dict,
        }
    }

    /// Compress `input` into `output`, returning the number of compressed bytes.
    ///
    /// `output` must be at least [`get_maximum_output_size`]`(input.len())` bytes.
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

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
}
