#[allow(unused_imports)]
use alloc::vec::Vec;

use crate::fastcpy::slice_copy;

/// Returns a Sink backed by `vec[offset..]` for compression output.
#[inline]
#[cfg(feature = "frame")]
pub(crate) fn vec_sink_for_compression(
    vec: &mut Vec<u8>,
    offset: usize,
    pos: usize,
    required_capacity: usize,
) -> crate::verified_sink::VerifiedSliceSink<'_> {
    vec.resize(offset + required_capacity, 0);
    crate::verified_sink::VerifiedSliceSink::new(&mut vec[offset..], pos)
}

/// Returns a Sink backed by `vec[offset..]` for decompression output.
#[cfg(feature = "frame")]
#[inline]
pub(crate) fn vec_sink_for_decompression(
    vec: &mut Vec<u8>,
    offset: usize,
    pos: usize,
    required_capacity: usize,
) -> SliceSink<'_> {
    vec.resize(offset + required_capacity, 0);
    SliceSink::new(&mut vec[offset..], pos)
}

pub(crate) trait Sink {
    /// Pushes a byte to the end of the Sink.
    fn push(&mut self, byte: u8);

    fn pos(&self) -> usize;

    fn capacity(&self) -> usize;

    /// Extends the Sink with `data`.
    fn extend_from_slice(&mut self, data: &[u8]);

    fn extend_from_slice_wild(&mut self, data: &[u8], copy_len: usize);

    fn extend_from_within_overlapping(&mut self, start: usize, num_bytes: usize);

    fn output_mut_with_pos(&mut self) -> (&mut [u8], &mut usize);
}

/// SliceSink writes into a preallocated `&mut [u8]`.
///
/// # Handling of Capacity
/// Extend methods will panic if there's insufficient capacity left in the Sink.
///
/// # Invariants
///   - Bytes `[..pos()]` are always initialized.
pub(crate) struct SliceSink<'a> {
    output: &'a mut [u8],
    pos: usize,
}

impl<'a> SliceSink<'a> {
    /// Creates a `Sink` backed by the given byte slice.
    /// `pos` defines the initial output position in the Sink.
    /// # Panics
    /// Panics if `pos` is out of bounds.
    #[inline]
    pub(crate) fn new(output: &'a mut [u8], pos: usize) -> Self {
        let _ = &mut output[..pos]; // bounds check pos
        SliceSink { output, pos }
    }
}

impl Sink for SliceSink<'_> {
    #[inline]
    fn push(&mut self, byte: u8) {
        self.output[self.pos] = byte;
        self.pos += 1;
    }

    #[inline]
    fn pos(&self) -> usize {
        self.pos
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.output.len()
    }

    #[inline]
    fn extend_from_slice(&mut self, data: &[u8]) {
        self.extend_from_slice_wild(data, data.len())
    }

    #[inline]
    fn extend_from_slice_wild(&mut self, data: &[u8], copy_len: usize) {
        assert!(copy_len <= data.len());
        slice_copy(data, &mut self.output[self.pos..(self.pos) + data.len()]);
        self.pos += copy_len;
    }

    #[inline]
    #[cfg_attr(feature = "nightly", optimize(size))]
    fn extend_from_within_overlapping(&mut self, start: usize, num_bytes: usize) {
        let offset = self.pos - start;
        for i in start + offset..start + offset + num_bytes {
            self.output[i] = self.output[i - offset];
        }
        self.pos += num_bytes;
    }

    #[inline]
    fn output_mut_with_pos(&mut self) -> (&mut [u8], &mut usize) {
        (self.output, &mut self.pos)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_sink_slice() {
        use crate::sink::Sink;
        use crate::sink::SliceSink;
        let mut data = vec![0; 5];
        let sink = SliceSink::new(&mut data, 1);
        assert_eq!(sink.pos(), 1);
        assert_eq!(sink.capacity(), 5);
    }
}
