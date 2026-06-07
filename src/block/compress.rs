//! LZ4 block compression.

use core::fmt;

use crate::block::hashtable::HashTable;
use crate::block::END_OFFSET;
use crate::block::LZ4_MIN_LENGTH;
use crate::block::MAX_DISTANCE;
use crate::block::MFLIMIT;
use crate::block::MINMATCH;
use crate::sink::Sink;
#[cfg(test)]
use crate::sink::SliceSink;
use crate::verified_sink::VerifiedSliceSink;
#[allow(unused_imports)]
use alloc::vec;

#[allow(unused_imports)]
use alloc::vec::Vec;

pub(crate) use super::hashtable::HashTable4K;
pub(crate) use super::hashtable::HashTable4KU16;
use super::{CompressError, WINDOW_SIZE};

/// Skip acceleration: step grows by 1 every `1 << N` consecutive non-matches.
/// C lz4 uses 6; see DESIGN.md for tradeoff analysis.
const INCREASE_STEPSIZE_BITSHIFT: usize = 3;

/// Read a native-endian 4-byte integer from `input[n..]`.
#[inline]
#[cfg(target_pointer_width = "32")]
pub(super) fn get_batch(input: &[u8], n: usize) -> u32 {
    u32::from_ne_bytes(input[n..n + 4].try_into().unwrap())
}

/// Read an usize sized "batch" from some position.
///
/// This will read a native-endian usize from some position.
#[inline]
pub(super) fn get_batch_arch(input: &[u8], n: usize) -> usize {
    const USIZE_SIZE: usize = core::mem::size_of::<usize>();
    let arr: &[u8; USIZE_SIZE] = input[n..n + USIZE_SIZE].try_into().unwrap();
    usize::from_ne_bytes(*arr)
}

#[inline]
fn token_from_literal(lit_len: usize) -> u8 {
    if lit_len < 0xF {
        (lit_len as u8) << 4
    } else {
        0xF0
    }
}

#[inline]
fn token_from_literal_and_match_length(lit_len: usize, duplicate_length: usize) -> u8 {
    let mut token = if lit_len < 0xF {
        (lit_len as u8) << 4
    } else {
        0xF0
    };

    token |= if duplicate_length < 0xF {
        duplicate_length as u8
    } else {
        0xF
    };

    token
}

/// Write an integer to the output.
///
/// Each additional byte then represent a value from 0 to 255, which is added to the previous value
/// to produce a total length. When the byte value is 255, another byte must read and added, and so
/// on. There can be any number of bytes of value "255" following token
#[inline]
pub(super) fn write_integer(output: &mut impl Sink, mut n: usize) {
    while n >= 0xFF {
        n -= 0xFF;
        push_byte(output, 0xFF);
    }
    push_byte(output, n as u8);
}

/// Handle the last bytes from the input as literals
#[cold]
fn handle_last_literals(output: &mut impl Sink, input: &[u8], start: usize) {
    let lit_len = input.len() - start;

    let token = token_from_literal(lit_len);
    push_byte(output, token);
    if lit_len >= 0xF {
        write_integer(output, lit_len - 0xF);
    }
    output.extend_from_slice(&input[start..]);
}

/// Moves the cursors back as long as the bytes match, to find additional bytes in a duplicate
#[inline]
fn backtrack_match(
    input: &[u8],
    cur: &mut usize,
    literal_start: usize,
    source: &[u8],
    candidate: &mut usize,
) {
    while *candidate > 0 && *cur > literal_start && input[*cur - 1] == source[*candidate - 1] {
        *cur -= 1;
        *candidate -= 1;
    }
}

/// Compress all bytes of `input[input_pos..]` into `output`.
///
/// Bytes in `input[..input_pos]` are treated as a preamble and can be used for lookback.
/// This part is known as the compressor "prefix".
/// Bytes in `ext_dict` logically precede the bytes in `input` and can also be used for lookback.
///
/// `input_stream_offset` is the logical position of the first byte of `input`. This allows same
/// `dict` to be used for many calls to `compress_internal` as we can "readdress" the first byte of
/// `input` to be something other than 0.
///
/// `dict` is the dictionary of previously encoded sequences.
///
/// This is used to find duplicates in the stream so they are not written multiple times.
///
/// Every four bytes are hashed, and in the resulting slot their position in the input buffer
/// is placed in the dict. This way we can easily look up a candidate to back references.
///
/// Returns the number of bytes written (compressed) into `output`.
///
/// # Const parameters
/// `USE_DICT`: Disables usage of ext_dict (it'll panic if a non-empty slice is used).
/// In other words, this generates more optimized code when an external dictionary isn't used.
///
/// A similar const argument could be used to disable the Prefix mode (eg. USE_PREFIX),
/// which would impose `input_pos == 0 && input_stream_offset == 0`. Experiments didn't
/// show significant improvement though.
// Intentionally avoid inlining.
// Empirical tests revealed it to be rarely better but often significantly detrimental.
#[inline(never)]
pub(crate) fn compress_internal<
    T: HashTable,
    const USE_DICT: bool,
    const HAS_OFFSET: bool,
    S: Sink,
>(
    input: &[u8],
    input_pos: usize,
    output: &mut S,
    dict: &mut T,
    ext_dict: &[u8],
    input_stream_offset: usize,
) -> Result<usize, CompressError> {
    assert!(input_pos <= input.len());
    if USE_DICT {
        assert!(ext_dict.len() <= super::WINDOW_SIZE);
        assert!(ext_dict.len() <= input_stream_offset);
        assert!(input_stream_offset
            .checked_add(input.len())
            .and_then(|i| i.checked_add(ext_dict.len()))
            .is_some_and(|i| i <= isize::MAX as usize));
    } else {
        assert!(ext_dict.is_empty());
    }
    if !HAS_OFFSET {
        debug_assert_eq!(input_stream_offset, 0);
    }
    // Shadow with literal 0 so LLVM can eliminate all offset arithmetic at compile time.
    let input_stream_offset = if HAS_OFFSET { input_stream_offset } else { 0 };
    if output.capacity() - output.pos() < get_maximum_output_size(input.len() - input_pos) {
        return Err(CompressError::OutputTooSmall);
    }

    let output_start_pos = output.pos();
    if input.len() - input_pos < LZ4_MIN_LENGTH {
        handle_last_literals(output, input, input_pos);
        return Ok(output.pos() - output_start_pos);
    }

    let ext_dict_stream_offset = input_stream_offset - ext_dict.len();
    let end_pos_check = input.len() - MFLIMIT;
    let mut literal_start = input_pos;
    let mut cur = input_pos;

    if cur == 0 && input_stream_offset == 0 {
        let hash = T::get_hash_at_unchecked(input, 0);
        dict.put_at(hash, 0);
        cur = 1;
    }

    // Pre-hash: compute the hash of the next position before checking the current one.
    let mut forward_hash = T::get_hash_at_unchecked(input, cur);

    loop {
        let mut candidate;
        let mut candidate_source;
        let mut offset;
        let mut non_match_count = 1 << INCREASE_STEPSIZE_BITSHIFT;

        loop {
            let step = non_match_count >> INCREASE_STEPSIZE_BITSHIFT;
            non_match_count += 1;
            let next_cur = cur + step;

            if next_cur > end_pos_check + 1 {
                handle_last_literals(output, input, literal_start);
                return Ok(output.pos() - output_start_pos);
            }

            let hash = forward_hash;
            candidate = dict.get_at(hash);
            forward_hash = T::get_hash_at_unchecked(input, next_cur);
            dict.put_at(hash, cur + input_stream_offset);

            debug_assert!(candidate <= input_stream_offset + cur);

            if input_stream_offset + cur - candidate > MAX_DISTANCE {
                cur = next_cur;
                continue;
            }

            if candidate >= input_stream_offset {
                offset = (input_stream_offset + cur - candidate) as u16;
                candidate -= input_stream_offset;
                candidate_source = input;
            } else if USE_DICT {
                debug_assert!(
                    candidate >= ext_dict_stream_offset,
                    "Lost history in ext dict mode"
                );
                offset = (input_stream_offset + cur - candidate) as u16;
                candidate -= ext_dict_stream_offset;
                candidate_source = ext_dict;
            } else {
                debug_assert!(input_pos == 0, "Lost history in prefix mode");
                cur = next_cur;
                continue;
            }
            let cand_bytes: u32 =
                super::hashtable::get_batch_unchecked(candidate_source, candidate);
            let curr_bytes: u32 = super::hashtable::get_batch_unchecked(input, cur);

            if cand_bytes == curr_bytes {
                break;
            }
            cur = next_cur;
        }

        loop {
            backtrack_match(
                input,
                &mut cur,
                literal_start,
                candidate_source,
                &mut candidate,
            );

            let lit_len = cur - literal_start;

            cur += MINMATCH;
            candidate += MINMATCH;
            let duplicate_length = super::hashtable::count_same_bytes_unchecked(
                input,
                &mut cur,
                candidate_source,
                candidate,
                END_OFFSET,
            );

            let hash = T::get_hash_at_unchecked(input, cur - 2);
            dict.put_at(hash, cur - 2 + input_stream_offset);

            let token = token_from_literal_and_match_length(lit_len, duplicate_length);
            push_byte(output, token);
            if lit_len >= 0xF {
                write_integer(output, lit_len - 0xF);
            }
            if lit_len > 0 {
                copy_literals_wild(output, input, literal_start, lit_len);
            }
            push_u16(output, offset);
            if duplicate_length >= 0xF {
                write_integer(output, duplicate_length - 0xF);
            }
            literal_start = cur;

            // Re-match: after encoding, test if the current position also matches.
            // Chains matches without re-entering the search loop (skips step reset,
            // re-hashing). Only in non-dict mode to keep dict code simple.
            // The hash must NOT be stored on failure, or the search loop's first
            // iteration would self-match at offset 0.
            if !USE_DICT && cur <= end_pos_check {
                let hash = T::get_hash_at_unchecked(input, cur);
                let rematch = dict.get_at(hash);

                if input_stream_offset + cur - rematch <= MAX_DISTANCE
                    && rematch >= input_stream_offset
                {
                    let rc = rematch - input_stream_offset;
                    if super::hashtable::get_batch_unchecked(input, cur)
                        == super::hashtable::get_batch_unchecked(input, rc)
                    {
                        dict.put_at(hash, cur + input_stream_offset);
                        candidate = rc;
                        candidate_source = input;
                        offset = (input_stream_offset + cur - rematch) as u16;
                        continue;
                    }
                }
                // Reuse hash for the search loop's first iteration.
                forward_hash = hash;
            } else if cur <= end_pos_check {
                forward_hash = T::get_hash_at_unchecked(input, cur);
            }
            break;
        }
    }
}

#[inline]
fn push_byte(output: &mut impl Sink, el: u8) {
    output.push(el);
}

#[inline]
fn push_u16(output: &mut impl Sink, el: u16) {
    output.extend_from_slice(&el.to_le_bytes());
}

#[inline(always)]
fn copy_literals_wild(output: &mut impl Sink, input: &[u8], input_start: usize, len: usize) {
    output.extend_from_slice_wild(&input[input_start..input_start + len], len)
}

/// Compress all bytes of `input` into `output`.
/// The method chooses an appropriate hashtable to lookup duplicates.
/// output should be preallocated with a size of
/// `get_maximum_output_size`.
///
/// Returns the number of bytes written (compressed) into `output`.
#[inline]
pub(crate) fn compress_into_sink_with_dict<const USE_DICT: bool>(
    input: &[u8],
    output: &mut impl Sink,
    mut dict_data: &[u8],
) -> Result<usize, CompressError> {
    if USE_DICT && dict_data.len() < MINMATCH {
        return compress_into_sink_with_dict::<false>(input, output, b"");
    }
    if dict_data.len() + input.len() < u16::MAX as usize {
        let mut dict = HashTable4KU16::new();
        init_dict(&mut dict, &mut dict_data);
        compress_internal::<_, USE_DICT, USE_DICT, _>(
            input,
            0,
            output,
            &mut dict,
            dict_data,
            dict_data.len(),
        )
    } else {
        let mut dict = HashTable4K::new();
        init_dict(&mut dict, &mut dict_data);
        compress_internal::<_, USE_DICT, USE_DICT, _>(
            input,
            0,
            output,
            &mut dict,
            dict_data,
            dict_data.len(),
        )
    }
}

#[inline]
fn init_dict<T: HashTable>(dict: &mut T, dict_data: &mut &[u8]) {
    if dict_data.len() > WINDOW_SIZE {
        *dict_data = &dict_data[dict_data.len() - WINDOW_SIZE..];
    }
    let mut i = 0usize;
    while i + core::mem::size_of::<usize>() <= dict_data.len() {
        let hash = T::get_hash_at(dict_data, i);
        dict.put_at(hash, i);
        i += 3;
    }
}

/// Returns the maximum output size of the compressed data.
/// Can be used to preallocate capacity on the output vector
#[inline]
pub const fn get_maximum_output_size(input_len: usize) -> usize {
    16 + 4 + (input_len as u64 * 110 / 100) as usize
}

/// Compress all bytes of `input` into `output`.
/// The method chooses an appropriate hashtable to lookup duplicates.
/// output should be preallocated with a size of
/// `get_maximum_output_size`.
///
/// Returns the number of bytes written (compressed) into `output`.
#[inline]
pub fn compress_into(input: &[u8], output: &mut [u8]) -> Result<usize, CompressError> {
    compress_into_sink_with_dict::<false>(input, &mut VerifiedSliceSink::new(output, 0), b"")
}

#[inline]
fn compress_into_vec(input: &[u8], prepend_size: bool) -> Vec<u8> {
    let prepend_bytes = if prepend_size { 4 } else { 0 };
    let max_compressed_size = get_maximum_output_size(input.len()) + prepend_bytes;
    let mut compressed: Vec<u8> = vec![0u8; max_compressed_size];
    let out = if prepend_size {
        assert!(
            input.len() <= u32::MAX as usize,
            "compress_prepend_size: input length {} exceeds u32::MAX; \
             use compress/compress_into for block format or FrameEncoder for streaming",
            input.len()
        );
        compressed[..4].copy_from_slice(&(input.len() as u32).to_le_bytes());
        &mut compressed[4..]
    } else {
        &mut compressed
    };
    let compressed_len =
        compress_into_sink_with_dict::<false>(input, &mut VerifiedSliceSink::new(out, 0), b"")
            .unwrap();
    compressed.truncate(prepend_bytes + compressed_len);
    compressed.shrink_to_fit();
    compressed
}

/// Compress all bytes of `input` into `output`. The uncompressed size will be prepended as a little
/// endian u32. Can be used in conjunction with `decompress_size_prepended`
#[inline]
pub fn compress_prepend_size(input: &[u8]) -> Vec<u8> {
    compress_into_vec(input, true)
}

/// Compress all bytes of `input`.
#[inline]
pub fn compress(input: &[u8]) -> Vec<u8> {
    compress_into_vec(input, false)
}

/// A reusable block compressor. Pre-allocates the hash table once and reuses
/// it across calls. When constructed with [`Compressor::with_dict`], the
/// dictionary is hashed once and restored via a 16 KB memcpy before each call.
///
/// For one-shot compression, use [`compress`], [`compress_into`], or
/// [`compress_prepend_size`] instead.
///
/// # Example
/// ```
/// use lz4rip::block::{Compressor, get_maximum_output_size};
///
/// let mut comp = Compressor::new();
/// let input = b"hello world, hello world, hello!";
/// let mut output = vec![0u8; get_maximum_output_size(input.len())];
/// let compressed_len = comp.compress_into(input, &mut output).unwrap();
/// ```
pub struct Compressor {
    table: HashTable4K,
    pristine: Option<HashTable4K>,
    dict: Vec<u8>,
}

impl fmt::Debug for Compressor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Compressor")
            .field("dict_len", &self.dict.len())
            .field("has_pristine", &self.pristine.is_some())
            .finish()
    }
}

impl Compressor {
    /// Create a new compressor without a dictionary.
    pub fn new() -> Self {
        Compressor {
            table: HashTable4K::new(),
            pristine: None,
            dict: Vec::new(),
        }
    }

    /// Create a new compressor seeded with an external dictionary.
    ///
    /// The dictionary is hashed once during construction. Each subsequent
    /// call restores the pristine table state via a 16 KB memcpy.
    ///
    /// If `dict` is shorter than 4 bytes, it is ignored.
    pub fn with_dict(dict: &[u8]) -> Self {
        if dict.len() < MINMATCH {
            return Self::new();
        }
        let trimmed = if dict.len() > WINDOW_SIZE {
            &dict[dict.len() - WINDOW_SIZE..]
        } else {
            dict
        };
        let mut pristine = HashTable4K::new();
        let mut dict_ref = trimmed;
        init_dict(&mut pristine, &mut dict_ref);
        let mut table = HashTable4K::new();
        table.copy_from(&pristine);
        Compressor {
            table,
            pristine: Some(pristine),
            dict: trimmed.to_vec(),
        }
    }

    #[inline]
    fn prepare_table(&mut self) {
        if let Some(ref pristine) = self.pristine {
            self.table.copy_from(pristine);
        } else {
            self.table.clear();
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
        self.prepare_table();
        if !self.dict.is_empty() {
            compress_internal::<_, true, true, _>(
                input,
                0,
                &mut VerifiedSliceSink::new(output, 0),
                &mut self.table,
                &self.dict,
                self.dict.len(),
            )
        } else {
            compress_internal::<_, false, false, _>(
                input,
                0,
                &mut VerifiedSliceSink::new(output, 0),
                &mut self.table,
                b"",
                0,
            )
        }
    }

    /// Compress `input` into a new `Vec<u8>`.
    pub fn compress(&mut self, input: &[u8]) -> Vec<u8> {
        self.compress_vec(input, false)
    }

    /// Compress `input` with the uncompressed size prepended as a little-endian u32.
    pub fn compress_prepend_size(&mut self, input: &[u8]) -> Vec<u8> {
        self.compress_vec(input, true)
    }

    fn compress_vec(&mut self, input: &[u8], prepend_size: bool) -> Vec<u8> {
        let prepend_bytes = if prepend_size { 4 } else { 0 };
        let max_compressed = get_maximum_output_size(input.len()) + prepend_bytes;
        let mut compressed = vec![0u8; max_compressed];
        let out = if prepend_size {
            assert!(
                input.len() <= u32::MAX as usize,
                "compress_prepend_size: input length {} exceeds u32::MAX; \
                 use compress/compress_into for block format or FrameEncoder for streaming",
                input.len()
            );
            compressed[..4].copy_from_slice(&(input.len() as u32).to_le_bytes());
            &mut compressed[4..]
        } else {
            &mut compressed
        };
        self.prepare_table();
        let compressed_len = if !self.dict.is_empty() {
            compress_internal::<_, true, true, _>(
                input,
                0,
                &mut VerifiedSliceSink::new(out, 0),
                &mut self.table,
                &self.dict,
                self.dict.len(),
            )
        } else {
            compress_internal::<_, false, false, _>(
                input,
                0,
                &mut VerifiedSliceSink::new(out, 0),
                &mut self.table,
                b"",
                0,
            )
        }
        .unwrap();
        compressed.truncate(prepend_bytes + compressed_len);
        compressed.shrink_to_fit();
        compressed
    }
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn count_same_bytes(input: &[u8], cur: &mut usize, source: &[u8], candidate: usize) -> usize {
        const USIZE_SIZE: usize = core::mem::size_of::<usize>();
        let cur_slice = &input[*cur..input.len() - END_OFFSET];
        let cand_slice = &source[candidate..];

        let mut num = 0;
        for (block1, block2) in cur_slice
            .chunks_exact(USIZE_SIZE)
            .zip(cand_slice.chunks_exact(USIZE_SIZE))
        {
            let input_block = usize::from_ne_bytes(block1.try_into().unwrap());
            let match_block = usize::from_ne_bytes(block2.try_into().unwrap());

            if input_block == match_block {
                num += USIZE_SIZE;
            } else {
                let diff = input_block ^ match_block;
                num += (diff.to_le().trailing_zeros() / 8) as usize;
                *cur += num;
                return num;
            }
        }

        #[cold]
        fn count_same_bytes_tail(a: &[u8], b: &[u8], offset: usize) -> usize {
            a.iter()
                .zip(b)
                .skip(offset)
                .take_while(|(a, b)| a == b)
                .count()
        }
        num += count_same_bytes_tail(cur_slice, cand_slice, num);

        *cur += num;
        num
    }

    #[test]
    fn test_count_same_bytes() {
        let first: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let second: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        ];
        assert_eq!(count_same_bytes(first, &mut 0, second, 0), 16);

        let first: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let second: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            1, 1, 1,
        ];
        assert_eq!(count_same_bytes(first, &mut 0, second, 0), 20);

        let first: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 3, 4, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0,
        ];
        let second: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 3, 4, 1, 1, 1, 1, 1, 1, 1,
            1, 1, 1, 1, 1,
        ];
        assert_eq!(count_same_bytes(first, &mut 0, second, 0), 22);

        let first: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 3, 4, 5, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0,
        ];
        let second: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 3, 4, 5, 1, 1, 1, 1, 1, 1,
            1, 1, 1, 1, 1, 1,
        ];
        assert_eq!(count_same_bytes(first, &mut 0, second, 0), 23);

        let first: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 3, 4, 5, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0,
        ];
        let second: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 3, 4, 6, 1, 1, 1, 1, 1, 1,
            1, 1, 1, 1, 1, 1,
        ];
        assert_eq!(count_same_bytes(first, &mut 0, second, 0), 22);

        let first: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 3, 9, 5, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0,
        ];
        let second: &[u8] = &[
            1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 3, 4, 6, 1, 1, 1, 1, 1, 1,
            1, 1, 1, 1, 1, 1,
        ];
        assert_eq!(count_same_bytes(first, &mut 0, second, 0), 21);

        for diff_idx in 8..100 {
            let first: Vec<u8> = (0u8..255).cycle().take(100 + 12).collect();
            let mut second = first.clone();
            second[diff_idx] = 255;
            for start in 0..=diff_idx {
                let same_bytes = count_same_bytes(&first, &mut start.clone(), &second, start);
                assert_eq!(same_bytes, diff_idx - start);
            }
        }
    }

    #[test]
    fn test_bug() {
        let input: &[u8] = &[
            10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18,
        ];
        let _out = compress(input);
    }

    #[test]
    fn test_dict() {
        let input: &[u8] = &[
            10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18,
        ];
        let dict = input;
        let mut comp = Compressor::with_dict(dict);
        let compressed = comp.compress(input);
        assert_lt!(compressed.len(), compress(input).len());

        let decomp = crate::block::decompress::Decompressor::with_dict(dict);
        let uncompressed = decomp.decompress(&compressed, input.len()).unwrap();
        assert_eq!(input, &uncompressed[..]);
    }

    #[test]
    fn test_dict_no_panic() {
        let input: &[u8] = &[
            10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18,
        ];
        let dict = &[10, 12, 14];
        let mut comp = Compressor::with_dict(dict);
        let _compressed = comp.compress(input);
    }

    #[test]
    fn test_dict_match_crossing() {
        let input: &[u8] = &[
            10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18,
        ];
        let dict = input;
        let mut comp = Compressor::with_dict(dict);
        let compressed = comp.compress(input);
        assert_lt!(compressed.len(), compress(input).len());

        let mut uncompressed = vec![0u8; input.len() * 2];
        let dict_cutoff = dict.len() / 2;
        let output_start = dict.len() - dict_cutoff;
        uncompressed[..output_start].copy_from_slice(&dict[dict_cutoff..]);
        let uncomp_len = {
            let mut sink = SliceSink::new(&mut uncompressed[..], output_start);
            crate::block::decompress::decompress_internal::<true, _>(
                &compressed,
                &mut sink,
                &dict[..dict_cutoff],
            )
            .unwrap()
        };
        assert_eq!(input.len(), uncomp_len);
        assert_eq!(
            input,
            &uncompressed[output_start..output_start + uncomp_len]
        );
    }

    #[test]
    fn test_conformant_last_block() {
        let aaas: &[u8] = b"aaaaaaaaaaaaaaa";

        let out = compress(&aaas[..12]);
        assert_gt!(out.len(), 12);
        let out = compress(&aaas[..13]);
        assert_le!(out.len(), 13);
        let out = compress(&aaas[..14]);
        assert_le!(out.len(), 14);
        let out = compress(&aaas[..15]);
        assert_le!(out.len(), 15);

        let mut comp = Compressor::with_dict(aaas);
        let out = comp.compress(&aaas[..11]);
        assert_gt!(out.len(), 11);
        let out = comp.compress(&aaas[..12]);
        assert_gt!(out.len(), 12);
        let out = comp.compress(&aaas[..13]);
        assert_le!(out.len(), 13);
        let out = comp.compress(&aaas[..14]);
        assert_le!(out.len(), 14);
        let out = comp.compress(&aaas[..15]);
        assert_le!(out.len(), 15);
    }

    #[test]
    fn test_dict_size() {
        let dict = vec![b'a'; 1024 * 1024];
        let input = &b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaa"[..];
        let mut comp = Compressor::with_dict(&dict);
        let compressed = comp.compress_prepend_size(input);
        let decomp = crate::block::decompress::Decompressor::with_dict(&dict);
        let decompressed = decomp.decompress_size_prepended(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }

    #[test]
    fn test_compressor_roundtrip() {
        let input: &[u8] = &[
            10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18,
        ];
        let dict = input;

        let mut comp = Compressor::with_dict(dict);
        let mut compressed = vec![0u8; get_maximum_output_size(input.len())];
        let n = comp.compress_into(input, &mut compressed).unwrap();
        compressed.truncate(n);

        assert_lt!(compressed.len(), compress(input).len());

        let decomp = crate::block::decompress::Decompressor::with_dict(dict);
        let uncompressed = decomp.decompress(&compressed, input.len()).unwrap();
        assert_eq!(input, &uncompressed[..]);
    }

    #[test]
    fn test_compressor_reuse() {
        let input: &[u8] = &[
            10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18, 10, 12, 14, 16, 18,
        ];
        let dict = input;
        let mut comp = Compressor::with_dict(dict);
        let out_a = comp.compress(input);
        let out_b = comp.compress(input);
        assert_eq!(out_a, out_b);
    }

    #[test]
    fn compress_into_with_short_dict_does_not_panic() {
        let input = [0u8; 13];
        let decomp = crate::block::decompress::Decompressor::with_dict(&[]);

        for dict_len in 0..MINMATCH {
            let dict = vec![0u8; dict_len];
            let mut comp = Compressor::with_dict(&dict);
            let mut output = vec![0u8; get_maximum_output_size(input.len())];
            let compressed_len = comp.compress_into(&input, &mut output).unwrap();

            let mut uncompressed = vec![0u8; input.len()];
            let uncompressed_len = decomp
                .decompress_into(&output[..compressed_len], &mut uncompressed)
                .unwrap();
            uncompressed.truncate(uncompressed_len);
            assert_eq!(uncompressed, input);
        }
    }
}
