/// Read 1 byte without bounds checking.
#[inline]
pub(crate) fn read_byte_unchecked(input: &[u8], n: usize) -> u8 {
    debug_assert!(n < input.len());
    unsafe { *input.get_unchecked(n) }
}

/// Read 2 bytes as little-endian u16 without bounds checking.
#[inline]
pub(crate) fn read_u16_unchecked(input: &[u8], n: usize) -> u16 {
    debug_assert!(n + 2 <= input.len());
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
    unsafe {
        let ptr = buf.as_mut_ptr();
        core::ptr::copy_nonoverlapping(ptr.add(src_pos), ptr.add(*dst_pos), 8);
        core::ptr::copy_nonoverlapping(ptr.add(src_pos + 8), ptr.add(*dst_pos + 8), 8);
        core::ptr::copy_nonoverlapping(ptr.add(src_pos + 16), ptr.add(*dst_pos + 16), 2);
    }
    *dst_pos += advance;
}

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

/// Inline literal wildcopy in 32-byte chunks.
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

/// Inline match wildcopy for `offset >= 8`.
#[inline]
pub(crate) fn wild_copy_match_8(buf: &mut [u8], src: usize, dst_pos: &mut usize, len: usize) {
    debug_assert!(*dst_pos >= src + 8);
    debug_assert!(*dst_pos + len + 8 <= buf.len());
    let dst = *dst_pos;
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

/// Inline match wildcopy for `offset >= 16`.
#[inline]
pub(crate) fn wild_copy_match_16(buf: &mut [u8], src: usize, dst_pos: &mut usize, len: usize) {
    debug_assert!(*dst_pos >= src + 16);
    debug_assert!(*dst_pos + len + 16 <= buf.len());
    let dst = *dst_pos;
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

/// Inline match wildcopy for `offset >= 32`.
#[inline]
pub(crate) fn wild_copy_match_32(buf: &mut [u8], src: usize, dst_pos: &mut usize, len: usize) {
    debug_assert!(*dst_pos >= src + 32);
    debug_assert!(*dst_pos + len + 32 <= buf.len());
    let dst = *dst_pos;
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

/// Unchecked non-overlapping copy within `buf`.
#[inline]
pub(crate) fn copy_within_nonoverlap(buf: &mut [u8], src: usize, dst_pos: &mut usize, len: usize) {
    debug_assert!(src + len <= *dst_pos);
    debug_assert!(*dst_pos + len <= buf.len());
    unsafe {
        let ptr = buf.as_mut_ptr();
        core::ptr::copy_nonoverlapping(ptr.add(src), ptr.add(*dst_pos), len);
    }
    *dst_pos += len;
}
