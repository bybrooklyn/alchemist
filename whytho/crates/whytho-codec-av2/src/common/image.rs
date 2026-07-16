//! YUV frame buffers and plane descriptors.
//!
//! Reference: `avm/avm/avm_image.h`. Samples are stored as `u16` for both 8-bit and
//! high-bit-depth input.

use crate::common::enums::MAX_SB_SIZE;

/// A visible image plane backed by stride-padded sample storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plane {
    /// Row-major sample storage, including stride padding.
    pub data: Vec<u16>,
    /// Samples between the starts of adjacent rows.
    pub stride: usize,
    /// Visible width in samples.
    pub width: usize,
    /// Visible height in samples.
    pub height: usize,
}

impl Plane {
    /// Allocate a zero-filled plane with its stride rounded up to `stride_align`.
    ///
    /// # Panics
    ///
    /// Panics if `stride_align` is zero or the padded allocation size overflows `usize`.
    pub fn new(width: usize, height: usize, stride_align: usize) -> Self {
        assert!(stride_align != 0, "plane stride alignment must be non-zero");

        let stride = width
            .checked_add(stride_align - 1)
            .expect("plane stride calculation overflowed")
            / stride_align
            * stride_align;
        let len = stride
            .checked_mul(height)
            .expect("plane allocation size overflowed");

        Self {
            data: vec![0; len],
            stride,
            width,
            height,
        }
    }

    /// Visible width in samples.
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Visible height in samples.
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Return a visible row, excluding stride padding.
    pub fn row(&self, y: usize) -> Option<&[u16]> {
        if y >= self.height {
            return None;
        }
        let start = y.checked_mul(self.stride)?;
        let end = start.checked_add(self.width)?;
        self.data.get(start..end)
    }

    /// Return a mutable visible row, excluding stride padding.
    pub fn row_mut(&mut self, y: usize) -> Option<&mut [u16]> {
        if y >= self.height {
            return None;
        }
        let start = y.checked_mul(self.stride)?;
        let end = start.checked_add(self.width)?;
        self.data.get_mut(start..end)
    }

    /// Read a visible sample.
    pub fn get(&self, x: usize, y: usize) -> Option<u16> {
        self.row(y).and_then(|row| row.get(x)).copied()
    }

    /// Write a visible sample, returning `None` for out-of-bounds coordinates.
    pub fn set(&mut self, x: usize, y: usize, value: u16) -> Option<()> {
        *self.row_mut(y)?.get_mut(x)? = value;
        Some(())
    }

    /// Fill the complete backing allocation, including stride padding.
    pub fn fill(&mut self, value: u16) {
        self.data.fill(value);
    }
}

/// A planar YUV frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    /// Luma plane.
    pub y: Plane,
    /// First chroma plane.
    pub u: Plane,
    /// Second chroma plane.
    pub v: Plane,
    /// Stored sample bit depth.
    pub bit_depth: u8,
    /// Horizontal chroma subsampling shift.
    pub subsampling_x: u8,
    /// Vertical chroma subsampling shift.
    pub subsampling_y: u8,
}

impl Frame {
    /// Allocate a zero-filled planar 4:2:0 frame.
    ///
    /// Odd luma dimensions use ceiling division for chroma dimensions.
    ///
    /// # Panics
    ///
    /// Panics unless `bit_depth` is 8 or 10.
    pub fn new_420(width: usize, height: usize, bit_depth: u8) -> Self {
        assert!(
            matches!(bit_depth, 8 | 10),
            "AV2 input bit depth must be 8 or 10"
        );

        const SUBSAMPLING_X: u8 = 1;
        const SUBSAMPLING_Y: u8 = 1;
        let chroma_width = ceil_div_pow2(width, SUBSAMPLING_X);
        let chroma_height = ceil_div_pow2(height, SUBSAMPLING_Y);
        let chroma_stride_align = MAX_SB_SIZE >> SUBSAMPLING_X;

        Self {
            y: Plane::new(width, height, MAX_SB_SIZE),
            u: Plane::new(chroma_width, chroma_height, chroma_stride_align),
            v: Plane::new(chroma_width, chroma_height, chroma_stride_align),
            bit_depth,
            subsampling_x: SUBSAMPLING_X,
            subsampling_y: SUBSAMPLING_Y,
        }
    }

    /// Luma width in samples.
    pub const fn width(&self) -> usize {
        self.y.width
    }

    /// Luma height in samples.
    pub const fn height(&self) -> usize {
        self.y.height
    }

    /// Chroma width in samples.
    pub const fn chroma_width(&self) -> usize {
        self.u.width
    }

    /// Chroma height in samples.
    pub const fn chroma_height(&self) -> usize {
        self.u.height
    }
}

const fn ceil_div_pow2(value: usize, shift: u8) -> usize {
    let divisor = 1usize << shift;
    value / divisor + if value.is_multiple_of(divisor) { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_allocation_aligns_stride() {
        let plane = Plane::new(130, 3, 128);
        assert_eq!(plane.stride, 256);
        assert_eq!(plane.data.len(), 256 * 3);
        assert_eq!(plane.row(0).unwrap().len(), 130);
        assert!(plane.row(3).is_none());
    }

    #[test]
    fn plane_accessors_are_bounds_checked() {
        let mut plane = Plane::new(3, 2, 4);
        assert_eq!(plane.set(2, 1, 511), Some(()));
        assert_eq!(plane.get(2, 1), Some(511));
        assert_eq!(plane.set(3, 1, 1), None);
        assert_eq!(plane.get(0, 2), None);

        plane.fill(77);
        assert_eq!(plane.get(0, 0), Some(77));
        assert!(plane.data.iter().all(|&sample| sample == 77));
    }

    #[test]
    fn plane_rows_tolerate_invalid_public_geometry() {
        let mut plane = Plane::new(3, 2, 4);
        plane.data.truncate(2);
        assert_eq!(plane.row(0), None);
        assert_eq!(plane.row_mut(1), None);
    }

    #[test]
    fn frame_420_uses_ceil_divided_chroma_geometry() {
        let frame = Frame::new_420(1919, 1079, 10);
        assert_eq!((frame.width(), frame.height()), (1919, 1079));
        assert_eq!((frame.chroma_width(), frame.chroma_height()), (960, 540));
        assert_eq!((frame.subsampling_x, frame.subsampling_y), (1, 1));
        assert_eq!(frame.y.stride % MAX_SB_SIZE, 0);
        assert_eq!(frame.u.stride % (MAX_SB_SIZE / 2), 0);
        assert_eq!(frame.v.stride, frame.u.stride);
        assert_eq!(frame.bit_depth, 10);
    }

    #[test]
    #[should_panic(expected = "bit depth")]
    fn frame_rejects_unsupported_bit_depth() {
        let _ = Frame::new_420(16, 16, 12);
    }
}
