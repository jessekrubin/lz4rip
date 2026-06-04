//! LZ4 block format. Works in `no_std` environments.
//!
//! See the [spec](https://github.com/lz4/lz4/blob/dev/doc/lz4_Block_format.md).
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

#[forbid(unsafe_code)]
pub(crate) mod compress;
#[forbid(unsafe_code)]
pub(crate) mod decompress;
pub(crate) mod hashtable;

pub use compress::{
    compress, compress_into, compress_prepend_size, get_maximum_output_size, Compressor,
};
pub use decompress::{decompress, decompress_into, decompress_size_prepended, Decompressor};

use core::{error::Error, fmt};

pub(crate) const WINDOW_SIZE: usize = 64 * 1024;

/// Last match must start at least 12 bytes before end of block.
const MFLIMIT: usize = 12;

/// Last 5 bytes are always literals.
const LAST_LITERALS: usize = 5;

/// LAST_LITERALS + 1: extra byte for register-width reads near end.
const END_OFFSET: usize = LAST_LITERALS + 1;

/// Minimum compressible block length: MFLIMIT + 1 for the token.
const LZ4_MIN_LENGTH: usize = MFLIMIT + 1;

const MAXD_LOG: usize = 16;
const MAX_DISTANCE: usize = (1 << MAXD_LOG) - 1;

#[allow(dead_code)]
const MATCH_LENGTH_MASK: u32 = (1_u32 << 4) - 1;

const MINMATCH: usize = 4;

#[allow(dead_code)]
const FASTLOOP_SAFE_DISTANCE: usize = 64;

/// Inputs below this size use `HashTable4KU16`.
#[allow(dead_code)]
const LZ4_64KLIMIT: usize = (64 * 1024) + (MFLIMIT - 1);

/// An error representing invalid compressed data.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecompressError {
    /// The provided output is too small
    OutputTooSmall {
        /// Minimum expected output size
        expected: usize,
        /// Actual size of output
        actual: usize,
    },
    /// Literal is out of bounds of the input
    LiteralOutOfBounds,
    /// Expected another byte, but none found.
    ExpectedAnotherByte,
    /// Match offset is 0
    OffsetZero,
    /// Deduplication offset out of bounds (not in buffer).
    OffsetOutOfBounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Errors that can happen during compression.
pub enum CompressError {
    /// The provided output is too small.
    OutputTooSmall,
}

impl fmt::Display for DecompressError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DecompressError::OutputTooSmall { expected, actual } => {
                write!(
                    f,
                    "provided output is too small for the decompressed data, actual {actual}, expected \
                     {expected}"
                )
            }
            DecompressError::LiteralOutOfBounds => {
                f.write_str("literal is out of bounds of the input")
            }
            DecompressError::ExpectedAnotherByte => {
                f.write_str("expected another byte, found none")
            }
            DecompressError::OffsetZero => f.write_str("0 is not a valid match offset"),
            DecompressError::OffsetOutOfBounds => {
                f.write_str("the offset to copy is not contained in the decompressed buffer")
            }
        }
    }
}

impl fmt::Display for CompressError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompressError::OutputTooSmall => f.write_str(
                "output is too small for the compressed data, use get_maximum_output_size to \
                 reserve enough space",
            ),
        }
    }
}

impl Error for DecompressError {}

impl Error for CompressError {}

/// This can be used in conjunction with `decompress_size_prepended`.
/// It will read the first 4 bytes as little-endian encoded length, and return
/// the rest of the bytes after the length encoding.
#[inline]
pub fn uncompressed_size(input: &[u8]) -> Result<(usize, &[u8]), DecompressError> {
    let size = input.get(..4).ok_or(DecompressError::ExpectedAnotherByte)?;
    let size: &[u8; 4] = size.try_into().unwrap();
    let uncompressed_size = u32::from_le_bytes(*size) as usize;
    let rest = &input[4..];
    Ok((uncompressed_size, rest))
}

#[test]
fn integer_roundtrip() {
    for value in [0, 1, 254, 255, 256, 1000, 65535, 100_000] {
        let mut buf = vec![0u8; value / 255 + 2];
        let mut sink = crate::sink::SliceSink::new(&mut buf, 0);
        self::compress::write_integer(&mut sink, value);

        let decoded = self::decompress::read_integer(&buf, &mut 0).unwrap();
        assert_eq!(value, decoded);
    }
}
