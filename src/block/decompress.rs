//! LZ4 block decompression.

use core::fmt;

use crate::block::DecompressError;
use crate::block::MINMATCH;
use crate::sink::Sink;
use crate::sink::SliceSink;

#[allow(unused_imports)]
use alloc::vec;
#[allow(unused_imports)]
use alloc::vec::Vec;

/// Read a variable-length integer: sum consecutive 0xFF bytes, terminated by a non-0xFF byte.
#[inline]
pub(super) fn read_integer(input: &[u8], input_pos: &mut usize) -> Result<usize, DecompressError> {
    let mut n: usize = 0;
    loop {
        let extra: u8 = *input
            .get(*input_pos)
            .ok_or(DecompressError::ExpectedAnotherByte)?;
        *input_pos += 1;
        n = n
            .checked_add(extra as usize)
            .ok_or(DecompressError::LiteralOutOfBounds)?;
        if extra != 0xFF {
            break;
        }
    }
    Ok(n)
}

const FIT_TOKEN_MASK_LITERAL: u8 = 0b00001111;
const FIT_TOKEN_MASK_MATCH: u8 = 0b11110000;

#[test]
fn check_token() {
    assert!(!does_token_fit(15));
    assert!(does_token_fit(14));
    assert!(does_token_fit(114));
    assert!(!does_token_fit(0b11110000));
    assert!(does_token_fit(0b10110000));
}

/// Returns true if both nibbles of the token are below 15 (no extended lengths needed).
#[inline]
fn does_token_fit(token: u8) -> bool {
    !((token & FIT_TOKEN_MASK_LITERAL) == FIT_TOKEN_MASK_LITERAL
        || (token & FIT_TOKEN_MASK_MATCH) == FIT_TOKEN_MASK_MATCH)
}

/// Decompress all bytes of `input` into `output`.
///
/// Returns the number of bytes written (decompressed) into `output`.
#[inline]
pub(crate) fn decompress_internal<const USE_DICT: bool, S: Sink>(
    input: &[u8],
    output: &mut S,
    ext_dict: &[u8],
) -> Result<usize, DecompressError> {
    let mut input_pos = 0;
    let initial_output_pos = output.pos();

    // Margins for unchecked reads in the fast path. Inside these bounds,
    // wild copies are guaranteed not to read/write past the buffer.
    // aarch64 uses 32-byte copies; x86_64 uses 16-byte.
    let (lit_margin, match_margin) = if cfg!(target_arch = "aarch64") {
        (32, 32)
    } else {
        (16, 18)
    };
    let safe_input_pos = input
        .len()
        .saturating_sub(lit_margin + 2 /* u16 match offset */);
    let mut safe_output_pos = output.capacity().saturating_sub(lit_margin + match_margin);

    if USE_DICT {
        safe_output_pos = safe_output_pos.saturating_sub(17);
    };

    loop {
        let in_safe_region = input_pos < safe_input_pos;
        let token = if in_safe_region {
            super::hashtable::read_byte_unchecked(input, input_pos)
        } else {
            *input
                .get(input_pos)
                .ok_or(DecompressError::ExpectedAnotherByte)?
        };
        input_pos += 1;

        // Fast path: both lengths fit in token, safe distance from end.
        // aarch64: register comparisons first while token load is in flight.
        #[cfg(target_arch = "aarch64")]
        let enter_fast = in_safe_region && output.pos() < safe_output_pos && does_token_fit(token);
        #[cfg(not(target_arch = "aarch64"))]
        let enter_fast = does_token_fit(token) && in_safe_region && output.pos() < safe_output_pos;
        if enter_fast {
            let literal_length = (token >> 4) as usize;
            let match_length = MINMATCH + (token & 0xF) as usize;

            let offset =
                super::hashtable::read_u16_unchecked(input, input_pos + literal_length) as usize;
            if offset == 0 {
                return Err(DecompressError::OffsetZero);
            }

            let (out, pos) = output.output_mut_with_pos();
            #[cfg(target_arch = "aarch64")]
            super::hashtable::wild_copy_32(input, input_pos, out, pos, literal_length);
            #[cfg(not(target_arch = "aarch64"))]
            super::hashtable::wild_copy_16(input, input_pos, out, pos, literal_length);
            input_pos += literal_length + 2;

            if USE_DICT && offset > *pos {
                let _ = (out, pos);
                let copied = copy_from_dict(output, ext_dict, offset, match_length)?;
                if copied == match_length {
                    continue;
                }
                let match_length = match_length - copied;
                let (start, did_overflow) = output.pos().overflowing_sub(offset);
                if did_overflow {
                    return Err(DecompressError::OffsetOutOfBounds);
                }
                output.extend_from_within_overlapping(start, match_length);
                continue;
            }

            let (start, did_overflow) = pos.overflowing_sub(offset);
            if did_overflow {
                return Err(DecompressError::OffsetOutOfBounds);
            }
            if offset >= 8 {
                #[cfg(target_arch = "aarch64")]
                super::hashtable::wild_match_copy_32(out, start, pos, match_length);
                #[cfg(not(target_arch = "aarch64"))]
                super::hashtable::wild_match_copy_18(out, start, pos, match_length);
            } else if offset == 1 {
                let val = out[start];
                out[*pos..*pos + match_length].fill(val);
                *pos += match_length;
            } else if match_length <= offset {
                super::hashtable::copy_within_nonoverlap(out, start, pos, match_length);
            } else {
                super::hashtable::copy_within_overlapping(out, start, pos, match_length, offset);
            }
            continue;
        }

        let mut literal_length = (token >> 4) as usize;
        if literal_length != 0 {
            if literal_length == 15 {
                literal_length = literal_length
                    .checked_add(read_integer(input, &mut input_pos)?)
                    .ok_or(DecompressError::LiteralOutOfBounds)?;
            }

            if literal_length > input.len() - input_pos {
                return Err(DecompressError::LiteralOutOfBounds);
            }
            if literal_length > output.capacity() - output.pos() {
                return Err(DecompressError::OutputTooSmall {
                    expected: output.pos() + literal_length,
                    actual: output.capacity(),
                });
            }
            let (out, pos) = output.output_mut_with_pos();
            super::hashtable::copy_from_src(input, input_pos, out, pos, literal_length);
            input_pos += literal_length;
        }

        if input_pos >= input.len() {
            break;
        }
        let offset = {
            let dst = input
                .get(input_pos..input_pos + 2)
                .ok_or(DecompressError::ExpectedAnotherByte)?;
            input_pos += 2;
            let o = u16::from_le_bytes(dst.try_into().unwrap());
            if o == 0 {
                return Err(DecompressError::OffsetZero);
            }
            o as usize
        };

        let mut match_length = MINMATCH + (token & 0xF) as usize;
        if match_length == MINMATCH + 15 {
            match_length = match_length
                .checked_add(read_integer(input, &mut input_pos)?)
                .ok_or(DecompressError::LiteralOutOfBounds)?;
        }

        if output.pos() + match_length > output.capacity() {
            return Err(DecompressError::OutputTooSmall {
                expected: output.pos() + match_length,
                actual: output.capacity(),
            });
        }
        if USE_DICT && offset > output.pos() {
            let copied = copy_from_dict(output, ext_dict, offset, match_length)?;
            if copied == match_length {
                continue;
            }
            match_length -= copied;
        }

        let (out, pos) = output.output_mut_with_pos();
        let (start, did_overflow) = pos.overflowing_sub(offset);
        if did_overflow {
            return Err(DecompressError::OffsetOutOfBounds);
        }
        if match_length > offset {
            if offset == 1 {
                let val = out[start];
                out[*pos..*pos + match_length].fill(val);
                *pos += match_length;
            } else {
                super::hashtable::copy_within_overlapping(out, start, pos, match_length, offset);
            }
        } else {
            super::hashtable::copy_within_nonoverlap(out, start, pos, match_length);
        }
    }
    Ok(output.pos() - initial_output_pos)
}

#[inline]
fn copy_from_dict(
    output: &mut impl Sink,
    ext_dict: &[u8],
    offset: usize,
    match_length: usize,
) -> Result<usize, DecompressError> {
    // If we're here we know offset > output.pos
    debug_assert!(offset > output.pos());
    let (dict_offset, did_overflow) = ext_dict.len().overflowing_sub(offset - output.pos());
    if did_overflow {
        return Err(DecompressError::OffsetOutOfBounds);
    }
    // Can't copy past ext_dict len, the match may cross dict and output
    let dict_match_length = match_length.min(ext_dict.len() - dict_offset);
    let ext_match = &ext_dict[dict_offset..dict_offset + dict_match_length];
    output.extend_from_slice(ext_match);
    Ok(dict_match_length)
}

/// Decompress all bytes of `input` into `output`.
/// `output` should be preallocated with a size of of the uncompressed data.
#[inline]
pub fn decompress_into(input: &[u8], output: &mut [u8]) -> Result<usize, DecompressError> {
    decompress_internal::<false, _>(input, &mut SliceSink::new(output, 0), b"")
}

/// Decompress all bytes of `input` into a new vec. The first 4 bytes are the uncompressed size in
/// little endian. Can be used in conjunction with `compress_prepend_size`
#[inline]
pub fn decompress_size_prepended(input: &[u8]) -> Result<Vec<u8>, DecompressError> {
    let (uncompressed_size, input) = super::uncompressed_size(input)?;
    decompress(input, uncompressed_size)
}

/// Decompress all bytes of `input` into a new vec.
///
/// `uncompressed_size` must be >= the actual decompressed output size.
/// Returns [`DecompressError::OutputTooSmall`] if the buffer is too small.
/// The returned `Vec` is truncated to the actual decompressed length.
#[inline]
pub fn decompress(input: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, DecompressError> {
    let mut decompressed: Vec<u8> = vec![0; uncompressed_size];
    let decomp_len =
        decompress_internal::<false, _>(input, &mut SliceSink::new(&mut decompressed, 0), b"")?;
    decompressed.truncate(decomp_len);
    Ok(decompressed)
}

/// A block decompressor seeded with an external dictionary.
///
/// When no dictionary is needed, use the free functions [`decompress`],
/// [`decompress_into`], or [`decompress_size_prepended`] instead.
pub struct Decompressor {
    dict: Vec<u8>,
}

impl fmt::Debug for Decompressor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Decompressor")
            .field("dict_len", &self.dict.len())
            .finish()
    }
}

impl Decompressor {
    /// Create a decompressor seeded with an external dictionary.
    pub fn with_dict(dict: &[u8]) -> Self {
        Decompressor {
            dict: dict.to_vec(),
        }
    }

    /// Decompress `input` into a new `Vec<u8>`.
    ///
    /// `uncompressed_size` must be >= the actual decompressed size.
    pub fn decompress(
        &self,
        input: &[u8],
        uncompressed_size: usize,
    ) -> Result<Vec<u8>, DecompressError> {
        let mut decompressed = vec![0u8; uncompressed_size];
        let len = decompress_internal::<true, _>(
            input,
            &mut SliceSink::new(&mut decompressed, 0),
            &self.dict,
        )?;
        decompressed.truncate(len);
        Ok(decompressed)
    }

    /// Decompress `input` into `output`, returning the number of bytes written.
    pub fn decompress_into(
        &self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, DecompressError> {
        decompress_internal::<true, _>(input, &mut SliceSink::new(output, 0), &self.dict)
    }

    /// Decompress `input` where the first 4 bytes encode the uncompressed size
    /// as a little-endian u32.
    pub fn decompress_size_prepended(&self, input: &[u8]) -> Result<Vec<u8>, DecompressError> {
        let (uncompressed_size, input) = super::uncompressed_size(input)?;
        self.decompress(input, uncompressed_size)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn all_literal() {
        assert_eq!(decompress(&[0x30, b'a', b'4', b'9'], 3).unwrap(), b"a49");
    }

    #[test]
    fn incomplete_input() {
        assert!(matches!(
            decompress(&[], 255),
            Err(DecompressError::ExpectedAnotherByte)
        ));
        assert!(matches!(
            // incomplete literal len
            decompress(&[0xF0], 255),
            Err(DecompressError::ExpectedAnotherByte)
        ));
        assert!(matches!(
            // incomplete match offset
            decompress(&[0x0F, 0], 255),
            Err(DecompressError::ExpectedAnotherByte)
        ));
        assert!(matches!(
            // incomplete match len
            decompress(&[0x0F, 1, 0], 255),
            Err(DecompressError::ExpectedAnotherByte)
        ));
    }

    #[test]
    fn offset_oob() {
        // incomplete literal
        assert!(matches!(
            decompress(&[0x40, b'a', 1, 0], 4),
            Err(DecompressError::LiteralOutOfBounds)
        ));
        // literal too large for output
        assert!(matches!(
            decompress(&[0x20, b'a', b'a', 1, 0], 1),
            Err(DecompressError::OutputTooSmall {
                expected: 2,
                actual: 1
            })
        ));
        // match too large for output
        assert!(matches!(
            decompress(&[0x10, b'a', 1, 0], 4),
            Err(DecompressError::OutputTooSmall {
                expected: 5,
                actual: 4
            })
        ));

        // out-of-bounds hot-loop
        assert!(matches!(
            decompress(
                &[0x0E, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                256
            ),
            Err(DecompressError::OffsetOutOfBounds)
        ));
        // out-of-bounds for dict
        assert!(matches!(
            Decompressor::with_dict(&[0_u8; 250])
                .decompress(&[0x0E, 255, 0, 0x70, 0, 0, 0, 0, 0, 0, 0], 256,),
            Err(DecompressError::OffsetOutOfBounds)
        ));
        // out-of-bounds non-hot-loop overlapping
        assert!(matches!(
            decompress(&[0x0F, 1, 0, 1, 0x70, 0, 0, 0, 0, 0, 0, 0], 256),
            Err(DecompressError::OffsetOutOfBounds)
        ));
        // out-of-bounds non-hot-loop non-overlapping
        assert!(matches!(
            decompress(&[0x40, 0, 0, 0, 0, 255, 0, 0x70, 0, 0, 0, 0, 0, 0, 0], 256),
            Err(DecompressError::OffsetOutOfBounds)
        ));
    }

    #[test]
    fn offset_0() {
        assert!(matches!(
            decompress(&[0x0E, 0, 0, 0x70, 0, 0, 0, 0, 0, 0, 0], 256),
            Err(DecompressError::OffsetZero)
        ));
    }

    #[test]
    fn read_integer_overflow() {
        // Token 0xF0 = 15 literals, 0 match. Literal length extended by read_integer.
        // Feed enough 0xFF continuation bytes to overflow usize on 32-bit (or just
        // exceed any reasonable length on 64-bit). The checked_add must catch it.
        let mut input = vec![0xF0u8]; // token: 15 literals
                                      // 256 continuation bytes: value = 15 + 256*255 = 65295 on 64-bit (no overflow),
                                      // but literal_length > input.len() - input_pos catches it as LiteralOutOfBounds.
        input.extend(core::iter::repeat_n(0xFF, 256));
        input.push(0); // terminator for read_integer
        let result = decompress(&input, 1024);
        assert!(result.is_err(), "must reject absurd literal length");
    }
}
