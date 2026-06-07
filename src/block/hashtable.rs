#[allow(unused_imports)]
use alloc::boxed::Box;

/// Count matching bytes between `input[cur..]` and `source[candidate..]`,
/// stopping before `input[input_len - end_offset]`. Uses raw pointer
/// comparison (usize, then u32/u16/u8 stepdown) without bounds checks.
///
/// Caller must ensure both ranges are valid and `end_offset` bytes of
/// input are reserved after the match region.
#[inline]
pub(crate) fn count_same_bytes_unchecked(
    input: &[u8],
    cur: &mut usize,
    source: &[u8],
    candidate: usize,
    end_offset: usize,
) -> usize {
    let max_input = input.len().saturating_sub(*cur + end_offset);
    debug_assert!(candidate <= source.len());
    let max_cand = source.len() - candidate;
    let input_end = *cur + max_input.min(max_cand);
    let start = *cur;

    // SAFETY: `input_end` is clamped to both `input.len() - end_offset` and
    // `source.len() - candidate`, so all pointer offsets up to `input_end` are
    // within the respective slices.
    unsafe {
        let mut src_ptr = source.as_ptr().add(candidate);
        let inp_base = input.as_ptr();

        const STEP: usize = core::mem::size_of::<usize>();
        while *cur + STEP <= input_end {
            let diff = (inp_base.add(*cur) as *const usize).read_unaligned()
                ^ (src_ptr as *const usize).read_unaligned();
            if diff == 0 {
                *cur += STEP;
                src_ptr = src_ptr.add(STEP);
            } else {
                *cur += (diff.to_le().trailing_zeros() / 8) as usize;
                return *cur - start;
            }
        }

        #[cfg(target_pointer_width = "64")]
        if input_end - *cur >= 4 {
            let diff = (inp_base.add(*cur) as *const u32).read_unaligned()
                ^ (src_ptr as *const u32).read_unaligned();
            if diff == 0 {
                *cur += 4;
                src_ptr = src_ptr.add(4);
            } else {
                *cur += (diff.to_le().trailing_zeros() / 8) as usize;
                return *cur - start;
            }
        }

        if input_end - *cur >= 2
            && (inp_base.add(*cur) as *const u16).read_unaligned()
                == (src_ptr as *const u16).read_unaligned()
        {
            *cur += 2;
            src_ptr = src_ptr.add(2);
        }

        if *cur < input_end && *inp_base.add(*cur) == *src_ptr {
            *cur += 1;
        }
    }

    *cur - start
}

/// Read 4 bytes from `input` at position `n` without bounds checking.
///
/// # Safety
/// Caller must ensure `n + 4 <= input.len()`. Hash table candidates satisfy
/// this: they were stored as positions <= end_pos_check (input.len() - 12),
/// so candidate + 4 <= input.len() - 8.
#[inline]
pub(crate) fn get_batch_unchecked(input: &[u8], n: usize) -> u32 {
    debug_assert!(n + 4 <= input.len());
    // SAFETY: caller ensures `n + 4 <= input.len()`.
    unsafe { (input.as_ptr().add(n) as *const u32).read_unaligned() }
}

/// Read 1 byte without bounds checking.
#[inline]
pub(crate) fn read_byte_unchecked(input: &[u8], n: usize) -> u8 {
    debug_assert!(n < input.len());
    // SAFETY: caller ensures `n < input.len()`.
    unsafe { *input.get_unchecked(n) }
}

/// Read 2 bytes as little-endian u16 without bounds checking.
#[inline]
pub(crate) fn read_u16_unchecked(input: &[u8], n: usize) -> u16 {
    debug_assert!(n + 2 <= input.len());
    // SAFETY: caller ensures `n + 2 <= input.len()`.
    unsafe {
        (input.as_ptr().add(n) as *const u16)
            .read_unaligned()
            .to_le()
    }
}

/// Copy 16 bytes from `src[src_pos..]` to `dst[dst_pos..]`, advancing `dst_pos` by `advance`.
#[inline]
pub(crate) fn wild_copy_16(
    src: &[u8],
    src_pos: usize,
    dst: &mut [u8],
    dst_pos: &mut usize,
    advance: usize,
) {
    debug_assert!(src_pos + 16 <= src.len());
    debug_assert!(*dst_pos + 16 <= dst.len());
    debug_assert!(advance <= 16);
    // SAFETY: caller ensures `src_pos + 16 <= src.len()` and
    // `*dst_pos + 16 <= dst.len()`. Regions do not alias (src is input,
    // dst is output).
    unsafe {
        core::ptr::copy_nonoverlapping(
            src.as_ptr().add(src_pos),
            dst.as_mut_ptr().add(*dst_pos),
            16,
        );
    }
    *dst_pos += advance;
}

/// Copy match within `buf`: three sequential 8-byte copies from `src_pos` to `*dst_pos`.
/// Handles offsets >= 8 correctly (sequential copies read freshly written data,
/// reproducing LZ4's overlapping-match semantics).
#[inline]
pub(crate) fn wild_match_copy_18(
    buf: &mut [u8],
    src_pos: usize,
    dst_pos: &mut usize,
    advance: usize,
) {
    debug_assert!(*dst_pos + 18 <= buf.len());
    debug_assert!(*dst_pos - src_pos >= 8);
    debug_assert!(advance <= 18);
    // SAFETY: caller ensures `*dst_pos + 18 <= buf.len()` and offset >= 8.
    // The three copies (8+8+2 = 18 bytes) start at `src_pos` and `*dst_pos`
    // with a gap of at least 8, so each 8-byte copy_nonoverlapping has
    // non-overlapping source and destination.
    unsafe {
        let ptr = buf.as_mut_ptr();
        core::ptr::copy_nonoverlapping(ptr.add(src_pos), ptr.add(*dst_pos), 8);
        core::ptr::copy_nonoverlapping(ptr.add(src_pos + 8), ptr.add(*dst_pos + 8), 8);
        core::ptr::copy_nonoverlapping(ptr.add(src_pos + 16), ptr.add(*dst_pos + 16), 2);
    }
    *dst_pos += advance;
}

// Knuth's multiplicative hash constant (golden ratio * 2^32).
const KNUTH: u32 = 2654435761;

// On 64-bit, hash functions read 8 bytes but shift left 24 to use only
// the low 5 bytes of input (5-byte hash for better distribution across
// 4K-8K entries). The prime differs by endianness because the read is
// native-endian: on LE the low 5 bytes are input bytes 0-4; on BE they
// are bytes 3-7.
#[cfg(target_pointer_width = "64")]
const PRIME5: usize = if cfg!(target_endian = "little") {
    889523592379
} else {
    11400714785074694791
};

/// Unchecked copy from `src[src_pos..src_pos+len]` to `dst[*dst_pos..*dst_pos+len]`.
#[inline]
pub(crate) fn copy_from_src(
    src: &[u8],
    src_pos: usize,
    dst: &mut [u8],
    dst_pos: &mut usize,
    len: usize,
) {
    debug_assert!(src_pos + len <= src.len());
    debug_assert!(*dst_pos + len <= dst.len());
    // SAFETY: caller ensures both ranges are within their respective slices.
    // Regions do not alias (src is input, dst is output).
    unsafe {
        core::ptr::copy_nonoverlapping(
            src.as_ptr().add(src_pos),
            dst.as_mut_ptr().add(*dst_pos),
            len,
        );
    }
    *dst_pos += len;
}

/// Overlapping match copy for offset >= 2. Seeds with `offset` bytes via
/// non-overlapping copy, then doubles the written region each iteration
/// until `match_len` is reached. O(log(match_len/offset)) memcpy calls.
#[inline]
pub(crate) fn copy_within_overlapping(
    buf: &mut [u8],
    start: usize,
    dst_pos: &mut usize,
    match_len: usize,
    offset: usize,
) {
    debug_assert!(offset >= 2);
    debug_assert!(*dst_pos == start + offset);
    debug_assert!(*dst_pos + match_len <= buf.len());

    let dst = *dst_pos;
    let initial = offset.min(match_len);

    // SAFETY: `*dst_pos + match_len <= buf.len()` (checked above).
    // The initial copy is `offset` bytes from `[start..start+offset)` to
    // `[dst..dst+offset)` which are adjacent and non-overlapping.
    // Each doubling copy reads from `[dst..)` (already written) and writes
    // to `[dst+written..)` with `copy_len <= written`, so source and
    // destination never overlap.
    unsafe {
        let ptr = buf.as_mut_ptr();
        core::ptr::copy_nonoverlapping(ptr.add(start), ptr.add(dst), initial);

        let mut written = initial;
        while written < match_len {
            let copy_len = written.min(match_len - written);
            core::ptr::copy_nonoverlapping(ptr.add(dst), ptr.add(dst + written), copy_len);
            written += copy_len;
        }
    }

    *dst_pos += match_len;
}

/// Inline literal wildcopy: copy `len` bytes from `src[src_pos..]` to
/// `dst[*dst_pos..]` in 16-byte chunks, overcopying up to 15 bytes past the end.
/// Avoids the `memmove` call that `copy_from_src` (a sized `copy_nonoverlapping`)
/// lowers to — a large win for slow-path sequences whose literals are short or
/// medium-length, where the call overhead dominates.
///
/// Explicit u128 load/store (rather than `copy_nonoverlapping(.., 16)`) keeps the
/// loop body as vector ld/st and prevents LLVM from re-rolling it into a memcpy.
///
/// Caller must ensure `src_pos + len + 32 <= src.len()` and
/// `*dst_pos + len + 32 <= dst.len()` (room for the trailing overcopy). Regions
/// do not alias (src is input, dst is output).
#[inline]
pub(crate) fn wild_copy_literals(
    src: &[u8],
    src_pos: usize,
    dst: &mut [u8],
    dst_pos: &mut usize,
    len: usize,
) {
    debug_assert!(src_pos + len + 32 <= src.len());
    debug_assert!(*dst_pos + len + 32 <= dst.len());
    let d = *dst_pos;
    // SAFETY: caller guarantees `len + 32` bytes of headroom in both slices.
    unsafe {
        let sp = src.as_ptr();
        let dp = dst.as_mut_ptr();
        let mut done = 0;
        loop {
            let v0 = (sp.add(src_pos + done) as *const u128).read_unaligned();
            let v1 = (sp.add(src_pos + done + 16) as *const u128).read_unaligned();
            (dp.add(d + done) as *mut u128).write_unaligned(v0);
            (dp.add(d + done + 16) as *mut u128).write_unaligned(v1);
            done += 32;
            if done >= len {
                break;
            }
        }
    }
    *dst_pos += len;
}

/// Inline match wildcopy for `offset >= 8`: copy `len` bytes from `buf[src..]` to
/// `buf[*dst_pos..]` in 8-byte chunks, overcopying up to 7 bytes. Correct for any
/// `offset >= 8` (each 8-byte read lies wholly within already-written data, so the
/// overlapping-match pattern is reproduced). Replaces the `memmove` call that the
/// exact slow-path copy lowers to.
///
/// Caller must ensure `*dst_pos - src >= 8` and `*dst_pos + len + 8 <= buf.len()`.
#[inline]
pub(crate) fn wild_copy_match_8(buf: &mut [u8], src: usize, dst_pos: &mut usize, len: usize) {
    debug_assert!(*dst_pos >= src + 8);
    debug_assert!(*dst_pos + len + 8 <= buf.len());
    let dst = *dst_pos;
    // SAFETY: `offset = dst - src >= 8`, so each 8-byte read at `src + done` ends at
    // or before `dst + done` (already-written bytes). `len + 8` bytes of write
    // headroom are guaranteed by the caller.
    unsafe {
        let ptr = buf.as_mut_ptr();
        let mut done = 0;
        loop {
            let v = (ptr.add(src + done) as *const u64).read_unaligned();
            (ptr.add(dst + done) as *mut u64).write_unaligned(v);
            done += 8;
            if done >= len {
                break;
            }
        }
    }
    *dst_pos += len;
}

/// Inline match wildcopy for `offset >= 16`: like [`wild_copy_match_8`] but with
/// 16-byte chunks, overcopying up to 15 bytes. Faster for long matches (e.g.
/// hdfs, where matches average ~47 bytes and offsets are almost always >= 16).
///
/// Caller must ensure `*dst_pos - src >= 16` and `*dst_pos + len + 16 <= buf.len()`.
#[inline]
pub(crate) fn wild_copy_match_16(buf: &mut [u8], src: usize, dst_pos: &mut usize, len: usize) {
    debug_assert!(*dst_pos >= src + 16);
    debug_assert!(*dst_pos + len + 16 <= buf.len());
    let dst = *dst_pos;
    // SAFETY: `offset = dst - src >= 16`, so each 16-byte read at `src + done`
    // ends at or before `dst + done` (already-written bytes). `len + 16` bytes of
    // write headroom are guaranteed by the caller.
    unsafe {
        let ptr = buf.as_mut_ptr();
        let mut done = 0;
        loop {
            let v = (ptr.add(src + done) as *const u128).read_unaligned();
            (ptr.add(dst + done) as *mut u128).write_unaligned(v);
            done += 16;
            if done >= len {
                break;
            }
        }
    }
    *dst_pos += len;
}

/// Inline match wildcopy for `offset >= 32`: 32-byte chunks (two u128 ld/st),
/// overcopying up to 31 bytes. For long matches with far offsets (e.g. hdfs).
///
/// Caller must ensure `*dst_pos - src >= 32` and `*dst_pos + len + 32 <= buf.len()`.
#[inline]
pub(crate) fn wild_copy_match_32(buf: &mut [u8], src: usize, dst_pos: &mut usize, len: usize) {
    debug_assert!(*dst_pos >= src + 32);
    debug_assert!(*dst_pos + len + 32 <= buf.len());
    let dst = *dst_pos;
    // SAFETY: `offset = dst - src >= 32`, so each 32-byte read at `src + done`
    // ends at or before `dst + done` (already-written bytes). `len + 32` bytes of
    // write headroom are guaranteed by the caller.
    unsafe {
        let ptr = buf.as_mut_ptr();
        let mut done = 0;
        loop {
            let v0 = (ptr.add(src + done) as *const u128).read_unaligned();
            let v1 = (ptr.add(src + done + 16) as *const u128).read_unaligned();
            (ptr.add(dst + done) as *mut u128).write_unaligned(v0);
            (ptr.add(dst + done + 16) as *mut u128).write_unaligned(v1);
            done += 32;
            if done >= len {
                break;
            }
        }
    }
    *dst_pos += len;
}

/// Unchecked non-overlapping copy within `buf`. Caller must ensure
/// `src + len <= *dst_pos` (no overlap).
#[inline]
pub(crate) fn copy_within_nonoverlap(buf: &mut [u8], src: usize, dst_pos: &mut usize, len: usize) {
    debug_assert!(src + len <= *dst_pos);
    debug_assert!(*dst_pos + len <= buf.len());
    // SAFETY: caller ensures `src + len <= *dst_pos` (no overlap) and
    // `*dst_pos + len <= buf.len()`.
    unsafe {
        let ptr = buf.as_mut_ptr();
        core::ptr::copy_nonoverlapping(ptr.add(src), ptr.add(*dst_pos), len);
    }
    *dst_pos += len;
}

pub(crate) trait HashTable {
    /// Look up a table entry by hash index. The index must come from
    /// `get_hash_at` / `get_hash_at_unchecked` on the same table type.
    fn get_at(&self, idx: usize) -> usize;
    fn put_at(&mut self, idx: usize, val: usize);
    fn clear(&mut self);
    fn get_hash_at(input: &[u8], pos: usize) -> usize;
    fn get_hash_at_unchecked(input: &[u8], pos: usize) -> usize;
}

const HASHTABLE_SIZE_4K: usize = 4 * 1024;
const HASHTABLE_SIZE_U16: usize = 8 * 1024;

// Hash byte width for U32 tables on 64-bit: 5 = PRIME5 (current), 4 = KNUTH (C lz4).
const U32_HASH_BYTES: usize = 5;

/// An 8K entry hash table using 16-bit values (16KB total, matching C lz4's byU16).
#[derive(Debug)]
#[repr(align(64))]
pub(crate) struct HashTable4KU16 {
    dict: Box<[u16; HASHTABLE_SIZE_U16]>,
}
impl HashTable4KU16 {
    #[inline]
    pub(crate) fn new() -> Self {
        let dict = alloc::vec![0; HASHTABLE_SIZE_U16]
            .into_boxed_slice()
            .try_into()
            .unwrap();
        Self { dict }
    }
}
impl HashTable for HashTable4KU16 {
    #[inline]
    fn get_at(&self, idx: usize) -> usize {
        debug_assert!(idx < HASHTABLE_SIZE_U16);
        // SAFETY: idx is a hash output masked to HASHTABLE_SIZE_U16 - 1.
        unsafe { *self.dict.get_unchecked(idx) as usize }
    }
    #[inline]
    fn put_at(&mut self, idx: usize, val: usize) {
        debug_assert!(idx < HASHTABLE_SIZE_U16);
        // SAFETY: idx is a hash output masked to HASHTABLE_SIZE_U16 - 1.
        unsafe {
            *self.dict.get_unchecked_mut(idx) = val as u16;
        }
    }
    #[inline]
    fn clear(&mut self) {
        self.dict.fill(0);
    }
    #[inline]
    #[cfg(target_pointer_width = "64")]
    fn get_hash_at(input: &[u8], pos: usize) -> usize {
        let batch = super::compress::get_batch_arch(input, pos);
        (batch << 24).wrapping_mul(PRIME5) >> 51
    }
    #[inline]
    #[cfg(target_pointer_width = "32")]
    fn get_hash_at(input: &[u8], pos: usize) -> usize {
        (super::get_batch(input, pos).wrapping_mul(KNUTH) >> 19) as usize
    }
    #[inline]
    #[cfg(target_pointer_width = "64")]
    fn get_hash_at_unchecked(input: &[u8], pos: usize) -> usize {
        debug_assert!(pos + 8 <= input.len());
        // SAFETY: caller ensures `pos + 8 <= input.len()`.
        let batch = unsafe { (input.as_ptr().add(pos) as *const usize).read_unaligned() };
        (batch << 24).wrapping_mul(PRIME5) >> 51
    }
    #[inline]
    #[cfg(target_pointer_width = "32")]
    fn get_hash_at_unchecked(input: &[u8], pos: usize) -> usize {
        // SAFETY: delegates to get_batch_unchecked which checks pos + 4.
        (get_batch_unchecked(input, pos).wrapping_mul(KNUTH) >> 19) as usize
    }
}

/// A 4K entry hash table using 32-bit values (16KB total, matching C lz4's byU32).
#[derive(Debug)]
pub(crate) struct HashTable4K {
    dict: Box<[u32; HASHTABLE_SIZE_4K]>,
}
impl HashTable4K {
    #[inline]
    pub(crate) fn new() -> Self {
        let dict = alloc::vec![0; HASHTABLE_SIZE_4K]
            .into_boxed_slice()
            .try_into()
            .unwrap();
        Self { dict }
    }

    #[cold]
    #[allow(dead_code)]
    pub(crate) fn reposition(&mut self, offset: u32) {
        for i in self.dict.iter_mut() {
            *i = i.saturating_sub(offset);
        }
    }

    /// Overwrite this table's entries with the contents of `other`. Reuses
    /// this table's existing allocation (no heap traffic).
    #[inline]
    pub(crate) fn copy_from(&mut self, other: &Self) {
        self.dict.copy_from_slice(&*other.dict);
    }
}
impl HashTable for HashTable4K {
    #[inline]
    fn get_at(&self, idx: usize) -> usize {
        debug_assert!(idx < HASHTABLE_SIZE_4K);
        // SAFETY: idx is a hash output masked to HASHTABLE_SIZE_4K - 1.
        unsafe { *self.dict.get_unchecked(idx) as usize }
    }
    #[inline]
    fn put_at(&mut self, idx: usize, val: usize) {
        debug_assert!(idx < HASHTABLE_SIZE_4K);
        // SAFETY: idx is a hash output masked to HASHTABLE_SIZE_4K - 1.
        unsafe {
            *self.dict.get_unchecked_mut(idx) = val as u32;
        }
    }
    #[inline]
    fn clear(&mut self) {
        self.dict.fill(0);
    }
    #[inline]
    #[cfg(target_pointer_width = "64")]
    fn get_hash_at(input: &[u8], pos: usize) -> usize {
        if U32_HASH_BYTES == 5 {
            let batch = super::compress::get_batch_arch(input, pos);
            (batch << 24).wrapping_mul(PRIME5) >> 52
        } else {
            let batch = u32::from_ne_bytes(input[pos..pos + 4].try_into().unwrap());
            (batch.wrapping_mul(KNUTH) >> (32 - HASHTABLE_SIZE_4K.ilog2())) as usize
        }
    }
    #[inline]
    #[cfg(target_pointer_width = "32")]
    fn get_hash_at(input: &[u8], pos: usize) -> usize {
        (super::compress::get_batch(input, pos).wrapping_mul(KNUTH) >> 20) as usize
    }
    #[inline]
    #[cfg(target_pointer_width = "64")]
    fn get_hash_at_unchecked(input: &[u8], pos: usize) -> usize {
        if U32_HASH_BYTES == 5 {
            debug_assert!(pos + 8 <= input.len());
            // SAFETY: caller ensures `pos + 8 <= input.len()`.
            let batch = unsafe { (input.as_ptr().add(pos) as *const usize).read_unaligned() };
            (batch << 24).wrapping_mul(PRIME5) >> 52
        } else {
            // SAFETY: delegates to get_batch_unchecked which checks pos + 4.
            let batch = get_batch_unchecked(input, pos);
            (batch.wrapping_mul(KNUTH) >> (32 - HASHTABLE_SIZE_4K.ilog2())) as usize
        }
    }
    #[inline]
    #[cfg(target_pointer_width = "32")]
    fn get_hash_at_unchecked(input: &[u8], pos: usize) -> usize {
        debug_assert!(pos + 4 <= input.len());
        // SAFETY: caller ensures `pos + 4 <= input.len()`.
        let batch = unsafe { (input.as_ptr().add(pos) as *const u32).read_unaligned() };
        (batch.wrapping_mul(KNUTH) >> 20) as usize
    }
}
