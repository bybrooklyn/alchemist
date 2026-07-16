//! Decoded Picture Buffer for H.264 encoder.
//!
//! Stores reconstructed frames as reference for inter prediction.
//! Supports multiple reference frames for P-frame and B-frame encoding.

/// A reconstructed frame stored as a reference.
#[derive(Debug, Clone)]
pub struct ReferenceFrame {
    pub frame_num: u32,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl ReferenceFrame {
    pub fn new(frame_num: u32, width: u32, height: u32) -> Self {
        let luma_size = (width * height) as usize;
        let chroma_size = ((width / 2) * (height / 2)) as usize;
        Self {
            frame_num,
            y: vec![0u8; luma_size],
            u: vec![128u8; chroma_size],
            v: vec![128u8; chroma_size],
            width,
            height,
        }
    }

    /// Create a reference frame from raw YUV data.
    pub fn from_data(
        frame_num: u32,
        width: u32,
        height: u32,
        y: &[u8],
        u: &[u8],
        v: &[u8],
    ) -> Self {
        Self {
            frame_num,
            y: y.to_vec(),
            u: u.to_vec(),
            v: v.to_vec(),
            width,
            height,
        }
    }

    /// Get luma sample at (x, y), clamped to frame boundaries.
    pub fn luma(&self, x: i32, y: i32) -> u8 {
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        self.y[y * self.width as usize + x]
    }

    /// Get chroma sample at (x, y) for the given plane (0=Cb, 1=Cr).
    pub fn chroma(&self, plane: usize, x: i32, y: i32) -> u8 {
        let cw = (self.width / 2) as i32;
        let ch = (self.height / 2) as i32;
        let x = x.clamp(0, cw - 1) as usize;
        let y = y.clamp(0, ch - 1) as usize;
        let data = if plane == 0 { &self.u } else { &self.v };
        data[y * cw as usize + x]
    }
}

/// DPB that stores reference frames for P-frame and B-frame encoding.
///
/// For P-frames: single reference (most recent frame).
/// For B-frames: two references (L0 = past, L1 = future).
#[derive(Debug)]
pub struct Dpb {
    /// Stored reference frames, ordered by frame_num.
    refs: Vec<ReferenceFrame>,
    /// Maximum number of reference frames.
    max_refs: usize,
}

impl Dpb {
    pub fn new(max_refs: usize) -> Self {
        Self {
            refs: Vec::with_capacity(max_refs),
            max_refs,
        }
    }

    /// Store a reconstructed frame as a reference.
    pub fn store(&mut self, frame: ReferenceFrame) {
        if self.refs.len() >= self.max_refs {
            // Remove oldest reference (sliding window)
            self.refs.remove(0);
        }
        self.refs.push(frame);
    }

    /// Get the most recent reference frame (for single-ref P-frames).
    pub fn last_ref(&self) -> Option<&ReferenceFrame> {
        self.refs.last()
    }

    /// Get a reference frame by frame_num.
    pub fn get_by_frame_num(&self, frame_num: u32) -> Option<&ReferenceFrame> {
        self.refs.iter().find(|r| r.frame_num == frame_num)
    }

    /// Get reference frame by index (0 = most recent, 1 = second most recent, etc.).
    pub fn get_by_index(&self, index: usize) -> Option<&ReferenceFrame> {
        if index < self.refs.len() {
            Some(&self.refs[self.refs.len() - 1 - index])
        } else {
            None
        }
    }

    /// Get the L0 reference (past, most recent) for B-frame encoding.
    pub fn l0_ref(&self) -> Option<&ReferenceFrame> {
        self.get_by_index(0)
    }

    /// Get the L1 reference (future, second most recent) for B-frame encoding.
    pub fn l1_ref(&self) -> Option<&ReferenceFrame> {
        self.get_by_index(1)
    }

    /// Number of stored reference frames.
    pub fn count(&self) -> usize {
        self.refs.len()
    }

    /// Clear all references (e.g., on IDR).
    pub fn clear(&mut self) {
        self.refs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpb_store_and_retrieve() {
        let mut dpb = Dpb::new(4);
        let ref0 = ReferenceFrame::new(0, 16, 16);
        let ref1 = ReferenceFrame::new(1, 16, 16);
        dpb.store(ref0);
        dpb.store(ref1);
        assert_eq!(dpb.count(), 2);
        assert_eq!(dpb.last_ref().unwrap().frame_num, 1);
    }

    #[test]
    fn dpb_sliding_window() {
        let mut dpb = Dpb::new(2);
        dpb.store(ReferenceFrame::new(0, 16, 16));
        dpb.store(ReferenceFrame::new(1, 16, 16));
        dpb.store(ReferenceFrame::new(2, 16, 16));
        assert_eq!(dpb.count(), 2);
        assert_eq!(dpb.refs[0].frame_num, 1);
        assert_eq!(dpb.refs[1].frame_num, 2);
    }

    #[test]
    fn dpb_clear() {
        let mut dpb = Dpb::new(4);
        dpb.store(ReferenceFrame::new(0, 16, 16));
        dpb.clear();
        assert_eq!(dpb.count(), 0);
        assert!(dpb.last_ref().is_none());
    }

    #[test]
    fn reference_frame_luma_clamp() {
        let ref_frame = ReferenceFrame::new(0, 4, 4);
        assert_eq!(ref_frame.luma(-1, -1), 0); // clamped to (0,0)
        assert_eq!(ref_frame.luma(10, 10), 0); // clamped to (3,3)
    }

    #[test]
    fn reference_frame_from_data() {
        let y = vec![128u8; 16 * 16];
        let u = vec![64u8; 8 * 8];
        let v = vec![192u8; 8 * 8];
        let ref_frame = ReferenceFrame::from_data(0, 16, 16, &y, &u, &v);
        assert_eq!(ref_frame.luma(0, 0), 128);
        assert_eq!(ref_frame.chroma(0, 0, 0), 64);
        assert_eq!(ref_frame.chroma(1, 0, 0), 192);
    }
}
