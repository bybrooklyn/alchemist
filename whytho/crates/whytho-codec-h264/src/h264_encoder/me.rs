//! Motion estimation for H.264 encoder.
//!
//! Implements full-pel block matching with SAD cost function.
//! Used for P-frame macroblock encoding to find motion vectors.

use super::dpb::ReferenceFrame;

/// A motion vector (dx, dy) in full-pel units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionVector {
    pub dx: i16,
    pub dy: i16,
}

impl MotionVector {
    pub const ZERO: Self = Self { dx: 0, dy: 0 };

    pub fn new(dx: i16, dy: i16) -> Self {
        Self { dx, dy }
    }
}

/// Compute SAD (Sum of Absolute Differences) between two blocks.
///
/// `src` is the source block at (src_x, src_y) with stride `src_stride`.
/// `ref_block` is the reference block at (ref_x, ref_y) with stride `ref_stride`.
/// `block_w` and `block_h` are the block dimensions.
pub fn sad_block(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_frame: &ReferenceFrame,
    ref_x: i32,
    ref_y: i32,
    block_w: usize,
    block_h: usize,
) -> u32 {
    let mut sad = 0u32;
    for row in 0..block_h {
        for col in 0..block_w {
            let src_px = src[(src_y + row) * src_stride + src_x + col] as i32;
            let ref_px = ref_frame.luma(ref_x + col as i32, ref_y + row as i32) as i32;
            sad += (src_px - ref_px).unsigned_abs();
        }
    }
    sad
}

/// Search result from motion estimation.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub mv: MotionVector,
    pub cost: u32,
}

/// Full-pel motion vector search for a macroblock (16x16).
///
/// Searches within `search_range` pixels of the origin position (0,0)
/// in the reference frame. Returns the motion vector with lowest SAD.
pub fn search_p16x16(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_frame: &ReferenceFrame,
    search_range: i16,
) -> SearchResult {
    let mut best_mv = MotionVector::ZERO;

    // Cost at zero MV (no motion)
    let mut best_cost = sad_block(
        src,
        src_x,
        src_y,
        src_stride,
        ref_frame,
        src_x as i32,
        src_y as i32,
        16,
        16,
    );

    // Search in a diamond pattern around the origin
    for dy in -search_range..=search_range {
        for dx in -search_range..=search_range {
            let ref_x = src_x as i32 + dx as i32;
            let ref_y = src_y as i32 + dy as i32;

            // Skip if reference block goes out of bounds
            if ref_x < 0
                || ref_y < 0
                || ref_x + 16 > ref_frame.width as i32
                || ref_y + 16 > ref_frame.height as i32
            {
                continue;
            }

            let cost = sad_block(
                src, src_x, src_y, src_stride, ref_frame, ref_x, ref_y, 16, 16,
            );
            if cost < best_cost {
                best_cost = cost;
                best_mv = MotionVector::new(dx, dy);
            }
        }
    }

    SearchResult {
        mv: best_mv,
        cost: best_cost,
    }
}

/// Full-pel motion vector search for an 8x8 block.
pub fn search_p8x8(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_frame: &ReferenceFrame,
    search_range: i16,
) -> SearchResult {
    let mut best_mv = MotionVector::ZERO;

    let mut best_cost = sad_block(
        src,
        src_x,
        src_y,
        src_stride,
        ref_frame,
        src_x as i32,
        src_y as i32,
        8,
        8,
    );

    for dy in -search_range..=search_range {
        for dx in -search_range..=search_range {
            let ref_x = src_x as i32 + dx as i32;
            let ref_y = src_y as i32 + dy as i32;

            if ref_x < 0
                || ref_y < 0
                || ref_x + 8 > ref_frame.width as i32
                || ref_y + 8 > ref_frame.height as i32
            {
                continue;
            }

            let cost = sad_block(src, src_x, src_y, src_stride, ref_frame, ref_x, ref_y, 8, 8);
            if cost < best_cost {
                best_cost = cost;
                best_mv = MotionVector::new(dx, dy);
            }
        }
    }

    SearchResult {
        mv: best_mv,
        cost: best_cost,
    }
}

/// Predict motion vector from neighboring blocks (median predictor).
///
/// Per H.264 spec 8.4.1.1: MV = median(left, top, top-right) with
/// special handling for unavailable neighbors.
pub fn predict_mv(
    left_mv: Option<MotionVector>,
    top_mv: Option<MotionVector>,
    top_right_mv: Option<MotionVector>,
) -> MotionVector {
    match (left_mv, top_mv, top_right_mv) {
        (Some(l), Some(t), Some(tr)) => {
            // Median of three
            MotionVector::new(median3(l.dx, t.dx, tr.dx), median3(l.dy, t.dy, tr.dy))
        }
        (Some(l), Some(t), None) => {
            // Only left and top available — use top-right from top-left
            MotionVector::new(median3(l.dx, t.dx, l.dx), median3(l.dy, t.dy, l.dy))
        }
        (Some(mv), None, None) | (None, Some(mv), None) => mv,
        _ => MotionVector::ZERO,
    }
}

fn median3(a: i16, b: i16, c: i16) -> i16 {
    let mut vals = [a, b, c];
    vals.sort();
    vals[1]
}

/// Motion vector cost for rate-distortion optimization.
/// Uses a simple bits * lambda approximation.
pub fn mv_cost(mv: MotionVector, pred_mv: MotionVector, lambda: u32) -> u32 {
    let dx = (mv.dx - pred_mv.dx).unsigned_abs() as u32;
    let dy = (mv.dy - pred_mv.dy).unsigned_abs() as u32;
    // Approximate bit cost: each component uses exp-golomb coding
    let bits = ue_bits(dx) + ue_bits(dy);
    bits * lambda
}

fn ue_bits(value: u32) -> u32 {
    if value == 0 {
        1
    } else {
        2 * (32 - (value + 1).leading_zeros()) + 1
    }
}

/// Check if a macroblock should be coded as P_SKIP.
///
/// P_SKIP is used when:
/// 1. The predicted MV equals the best MV (or zero MV cost is very low)
/// 2. The residual is all zeros (or very small)
pub fn should_skip(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_frame: &ReferenceFrame,
    pred_mv: MotionVector,
    skip_threshold: u32,
) -> bool {
    let ref_x = src_x as i32 + pred_mv.dx as i32;
    let ref_y = src_y as i32 + pred_mv.dy as i32;

    if ref_x < 0
        || ref_y < 0
        || ref_x + 16 > ref_frame.width as i32
        || ref_y + 16 > ref_frame.height as i32
    {
        return false;
    }

    let cost = sad_block(
        src, src_x, src_y, src_stride, ref_frame, ref_x, ref_y, 16, 16,
    );
    cost <= skip_threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ref_frame(width: u32, height: u32, value: u8) -> ReferenceFrame {
        ReferenceFrame::from_data(
            0,
            width,
            height,
            &vec![value; (width * height) as usize],
            &vec![128u8; ((width / 2) * (height / 2)) as usize],
            &vec![128u8; ((width / 2) * (height / 2)) as usize],
        )
    }

    #[test]
    fn sad_identical_blocks() {
        let ref_frame = make_ref_frame(32, 32, 128);
        let src = vec![128u8; 32 * 32];
        let sad = sad_block(&src, 0, 0, 32, &ref_frame, 0, 0, 16, 16);
        assert_eq!(sad, 0);
    }

    #[test]
    fn sad_constant_difference() {
        let ref_frame = make_ref_frame(32, 32, 100);
        let src = vec![128u8; 32 * 32];
        let sad = sad_block(&src, 0, 0, 32, &ref_frame, 0, 0, 16, 16);
        assert_eq!(sad, 28 * 256); // |128-100| * 16*16
    }

    #[test]
    fn search_finds_best_match() {
        let mut ref_frame = make_ref_frame(64, 64, 128);
        // Place a distinct pattern at (10, 10)
        for row in 0..16 {
            for col in 0..16 {
                ref_frame.y[(10 + row) * 64 + 10 + col] = 200;
            }
        }
        // Source matches the pattern at (10, 10)
        let mut src = vec![128u8; 64 * 64];
        for row in 0..16 {
            for col in 0..16 {
                src[(row * 64) + col] = 200;
            }
        }

        let result = search_p16x16(&src, 0, 0, 64, &ref_frame, 16);
        assert_eq!(result.mv, MotionVector::new(10, 10));
        assert_eq!(result.cost, 0);
    }

    #[test]
    fn search_zero_mv_for_identical() {
        let ref_frame = make_ref_frame(32, 32, 128);
        let src = vec![128u8; 32 * 32];
        let result = search_p16x16(&src, 0, 0, 32, &ref_frame, 8);
        assert_eq!(result.mv, MotionVector::ZERO);
    }

    #[test]
    fn median_mv_prediction() {
        let left = MotionVector::new(10, 0);
        let top = MotionVector::new(0, 10);
        let tr = MotionVector::new(20, 20);
        let pred = predict_mv(Some(left), Some(top), Some(tr));
        assert_eq!(pred.dx, 10); // median(10, 0, 20) = 10
        assert_eq!(pred.dy, 10); // median(0, 10, 20) = 10
    }

    #[test]
    fn median_mv_two_neighbors() {
        let left = MotionVector::new(4, 0);
        let top = MotionVector::new(0, 8);
        let pred = predict_mv(Some(left), Some(top), None);
        // With only left and top, median uses left twice: median(4, 0, 4) = 4
        assert_eq!(pred.dx, 4);
        assert_eq!(pred.dy, 0);
    }

    #[test]
    fn skip_check_identical() {
        let ref_frame = make_ref_frame(32, 32, 128);
        let src = vec![128u8; 32 * 32];
        assert!(should_skip(
            &src,
            0,
            0,
            32,
            &ref_frame,
            MotionVector::ZERO,
            100
        ));
    }

    #[test]
    fn skip_check_different() {
        let ref_frame = make_ref_frame(32, 32, 100);
        let src = vec![200u8; 32 * 32];
        assert!(!should_skip(
            &src,
            0,
            0,
            32,
            &ref_frame,
            MotionVector::ZERO,
            100
        ));
    }
}

// ============================================================
// Half-pel interpolation for sub-pixel motion estimation
// ============================================================

/// H.264 6-tap filter coefficients for half-pel interpolation.
/// Filter: [1, -5, 20, 20, -5, 1] / 32
fn filter_6tap(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) -> u8 {
    let val = a - 5 * b + 20 * c + 20 * d - 5 * e + f;
    ((val + 16) >> 5).clamp(0, 255) as u8
}

/// Interpolate a half-pel sample horizontally.
fn halfpel_h(ref_frame: &ReferenceFrame, x: i32, y: i32) -> u8 {
    filter_6tap(
        ref_frame.luma(x - 2, y) as i32,
        ref_frame.luma(x - 1, y) as i32,
        ref_frame.luma(x, y) as i32,
        ref_frame.luma(x + 1, y) as i32,
        ref_frame.luma(x + 2, y) as i32,
        ref_frame.luma(x + 3, y) as i32,
    )
}

/// Interpolate a half-pel sample vertically.
fn halfpel_v(ref_frame: &ReferenceFrame, x: i32, y: i32) -> u8 {
    filter_6tap(
        ref_frame.luma(x, y - 2) as i32,
        ref_frame.luma(x, y - 1) as i32,
        ref_frame.luma(x, y) as i32,
        ref_frame.luma(x, y + 1) as i32,
        ref_frame.luma(x, y + 2) as i32,
        ref_frame.luma(x, y + 3) as i32,
    )
}

/// Interpolate a half-pel sample diagonally (both H and V).
fn halfpel_hv(ref_frame: &ReferenceFrame, x: i32, y: i32) -> u8 {
    // Apply horizontal filter to 6 vertical samples, then vertical filter
    let h0 = halfpel_h(ref_frame, x, y - 2);
    let h1 = halfpel_h(ref_frame, x, y - 1);
    let h2 = halfpel_h(ref_frame, x, y);
    let h3 = halfpel_h(ref_frame, x, y + 1);
    let h4 = halfpel_h(ref_frame, x, y + 2);
    let h5 = halfpel_h(ref_frame, x, y + 3);
    filter_6tap(
        h0 as i32, h1 as i32, h2 as i32, h3 as i32, h4 as i32, h5 as i32,
    )
}

/// Get a reference sample at half-pel precision.
/// `x` and `y` are in half-pel units (multiply by 2 for full-pel coordinates).
fn ref_sample_halfpel(ref_frame: &ReferenceFrame, hx: i32, hy: i32) -> u8 {
    let full_x = hx / 2;
    let full_y = hy / 2;
    let frac_x = hx % 2;
    let frac_y = hy % 2;

    match (frac_x, frac_y) {
        (0, 0) => ref_frame.luma(full_x, full_y),        // full-pel
        (1, 0) => halfpel_h(ref_frame, full_x, full_y),  // half-pel H
        (0, 1) => halfpel_v(ref_frame, full_x, full_y),  // half-pel V
        (1, 1) => halfpel_hv(ref_frame, full_x, full_y), // half-pel HV
        _ => ref_frame.luma(full_x, full_y),             // shouldn't happen
    }
}

/// Compute SAD using half-pel precision reference.
fn sad_block_halfpel(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_frame: &ReferenceFrame,
    ref_hx: i32, // half-pel x coordinate
    ref_hy: i32, // half-pel y coordinate
    block_w: usize,
    block_h: usize,
) -> u32 {
    let mut sad = 0u32;
    for row in 0..block_h {
        for col in 0..block_w {
            let src_px = src[(src_y + row) * src_stride + src_x + col] as i32;
            let ref_px =
                ref_sample_halfpel(ref_frame, ref_hx + col as i32 * 2, ref_hy + row as i32 * 2)
                    as i32;
            sad += (src_px - ref_px).unsigned_abs();
        }
    }
    sad
}

/// Refine a motion vector with half-pel precision.
///
/// Given a full-pel MV, searches the 8 surrounding half-pel positions
/// and returns the best match. The returned MV is in half-pel units
/// (multiply by 2 and divide by 2 to get full-pel).
pub fn refine_halfpel(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_frame: &ReferenceFrame,
    fullpel_mv: MotionVector,
    block_size: usize,
) -> SearchResult {
    // Convert full-pel MV to half-pel coordinates
    let base_hx = (src_x as i32 + fullpel_mv.dx as i32) * 2;
    let base_hy = (src_y as i32 + fullpel_mv.dy as i32) * 2;

    // Search all 9 half-pel positions (including the full-pel center)
    let mut best_mv = fullpel_mv;
    let mut best_cost = sad_block_halfpel(
        src, src_x, src_y, src_stride, ref_frame, base_hx, base_hy, block_size, block_size,
    );

    for dy in -1..=1i16 {
        for dx in -1..1i16 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let hx = base_hx + dx as i32;
            let hy = base_hy + dy as i32;

            // Bounds check
            let full_x = hx / 2;
            let full_y = hy / 2;
            if full_x - 2 < 0
                || full_y - 2 < 0
                || full_x + block_size as i32 + 3 > ref_frame.width as i32
                || full_y + block_size as i32 + 3 > ref_frame.height as i32
            {
                continue;
            }

            let cost = sad_block_halfpel(
                src, src_x, src_y, src_stride, ref_frame, hx, hy, block_size, block_size,
            );

            if cost < best_cost {
                best_cost = cost;
                best_mv = MotionVector::new(fullpel_mv.dx + dx, fullpel_mv.dy + dy);
            }
        }
    }

    SearchResult {
        mv: best_mv,
        cost: best_cost,
    }
}

/// Full search with half-pel refinement: full-pel search followed by half-pel refinement.
pub fn search_p16x16_subpel(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_frame: &ReferenceFrame,
    search_range: i16,
) -> SearchResult {
    // First: full-pel search
    let fullpel_result = search_p16x16(src, src_x, src_y, src_stride, ref_frame, search_range);

    // Then: half-pel refinement
    refine_halfpel(
        src,
        src_x,
        src_y,
        src_stride,
        ref_frame,
        fullpel_result.mv,
        16,
    )
}

/// Search result for P8x8 partition (4 separate 8x8 blocks).
#[derive(Debug, Clone)]
pub struct P8x8SearchResult {
    pub mvs: [MotionVector; 4], // one MV per 8x8 block (top-left, top-right, bottom-left, bottom-right)
    pub total_cost: u32,
}

/// Search for P8x8 partition: 4 separate 8x8 blocks with independent motion vectors.
///
/// This provides better compression than P16x16 for regions with varying motion.
pub fn search_p8x8_partition(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_frame: &ReferenceFrame,
    search_range: i16,
) -> P8x8SearchResult {
    let mut mvs = [MotionVector::ZERO; 4];
    let mut total_cost = 0u32;

    // Search each 8x8 block independently
    for (i, &(block_row, block_col)) in [(0, 0), (0, 1), (1, 0), (1, 1)].iter().enumerate() {
        let bx = src_x + block_col * 8;
        let by = src_y + block_row * 8;

        let result = search_p8x8_block(src, bx, by, src_stride, ref_frame, search_range);
        mvs[i] = result.mv;
        total_cost += result.cost;
    }

    P8x8SearchResult { mvs, total_cost }
}

/// Search for a single 8x8 block (used by P8x8 partition search).
fn search_p8x8_block(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_frame: &ReferenceFrame,
    search_range: i16,
) -> SearchResult {
    let mut best_mv = MotionVector::ZERO;

    // Cost at zero MV
    let mut best_cost = sad_block(
        src,
        src_x,
        src_y,
        src_stride,
        ref_frame,
        src_x as i32,
        src_y as i32,
        8,
        8,
    );

    // Search
    for dy in -search_range..=search_range {
        for dx in -search_range..=search_range {
            let ref_x = src_x as i32 + dx as i32;
            let ref_y = src_y as i32 + dy as i32;

            if ref_x < 0
                || ref_y < 0
                || ref_x + 8 > ref_frame.width as i32
                || ref_y + 8 > ref_frame.height as i32
            {
                continue;
            }

            let cost = sad_block(src, src_x, src_y, src_stride, ref_frame, ref_x, ref_y, 8, 8);
            if cost < best_cost {
                best_cost = cost;
                best_mv = MotionVector::new(dx, dy);
            }
        }
    }

    SearchResult {
        mv: best_mv,
        cost: best_cost,
    }
}

/// B-frame search result with bi-directional prediction.
#[derive(Debug, Clone)]
pub struct BFrameSearchResult {
    pub mv_l0: MotionVector,
    pub mv_l1: MotionVector,
    pub cost_l0: u32,
    pub cost_l1: u32,
    pub cost_bi: u32,
    pub best_mode: u8,
}

/// Search for B-frame prediction using L0 (past) and L1 (future) references.
pub fn search_bframe_p16x16(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_l0: &ReferenceFrame,
    ref_l1: &ReferenceFrame,
    search_range: i16,
) -> BFrameSearchResult {
    let result_l0 = search_p16x16(src, src_x, src_y, src_stride, ref_l0, search_range);
    let result_l1 = search_p16x16(src, src_x, src_y, src_stride, ref_l1, search_range);

    let cost_bi = compute_bi_cost(
        src,
        src_x,
        src_y,
        src_stride,
        ref_l0,
        result_l0.mv,
        ref_l1,
        result_l1.mv,
        16,
    );

    let best_mode = if cost_bi <= result_l0.cost && cost_bi <= result_l1.cost {
        2
    } else if result_l0.cost <= result_l1.cost {
        0
    } else {
        1
    };

    BFrameSearchResult {
        mv_l0: result_l0.mv,
        mv_l1: result_l1.mv,
        cost_l0: result_l0.cost,
        cost_l1: result_l1.cost,
        cost_bi,
        best_mode,
    }
}

fn compute_bi_cost(
    src: &[u8],
    src_x: usize,
    src_y: usize,
    src_stride: usize,
    ref_l0: &ReferenceFrame,
    mv_l0: MotionVector,
    ref_l1: &ReferenceFrame,
    mv_l1: MotionVector,
    block_size: usize,
) -> u32 {
    let mut sad = 0u32;
    for row in 0..block_size {
        for col in 0..block_size {
            let src_px = src[(src_y + row) * src_stride + src_x + col] as i32;
            let pred_l0 = ref_l0.luma(
                src_x as i32 + mv_l0.dx as i32 + col as i32,
                src_y as i32 + mv_l0.dy as i32 + row as i32,
            ) as i32;
            let pred_l1 = ref_l1.luma(
                src_x as i32 + mv_l1.dx as i32 + col as i32,
                src_y as i32 + mv_l1.dy as i32 + row as i32,
            ) as i32;
            let pred_bi = (pred_l0 + pred_l1 + 1) >> 1;
            sad += (src_px - pred_bi).unsigned_abs();
        }
    }
    sad
}
