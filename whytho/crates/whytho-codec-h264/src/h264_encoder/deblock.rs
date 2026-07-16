//! Deblocking filter for H.264 encoder.
//!
//! Implements the H.264 in-loop deblocking filter (spec 8.7).
//! Applied to 4x4 block boundaries to reduce blocking artifacts.
//!
//! The filter has two modes:
//! - Strong filter: for edges with large discontinuities (bS=4)
//! - Normal filter: for other edges (bS=1-3)
//!
//! Reference: H.264 spec section 8.7 (Deblocking filter process)

/// Deblock strength for a block edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeblockStrength {
    None,   // bS=0: no filtering
    Weak,   // bS=1-3: normal filter
    Strong, // bS=4: strong filter
}

/// Apply deblocking filter to a reconstructed frame.
///
/// Filters horizontal and vertical edges at 4x4 block boundaries.
/// `y` - luma plane (mutable)
/// `width`, `height` - frame dimensions
/// `qp` - quantization parameter (controls filter strength)
pub fn deblock_frame(y: &mut [u8], width: u32, height: u32, qp: i8) {
    // Filter vertical edges (left-to-right)
    for mb_y in 0..height / 16 {
        for mb_x in 0..width / 16 {
            for edge in 0..4 {
                let x = mb_x * 16 + edge * 4;
                for row in 0..16 {
                    let py = (mb_y * 16 + row) as usize;
                    let px = x as usize;
                    if px > 0 && px < width as usize {
                        deblock_vertical_edge(y, width as usize, px, py, qp);
                    }
                }
            }
        }
    }

    // Filter horizontal edges (top-to-bottom)
    for mb_y in 0..height / 16 {
        for mb_x in 0..width / 16 {
            for edge in 0..4 {
                let y_pos = mb_y * 16 + edge * 4;
                for col in 0..16 {
                    let px = (mb_x * 16 + col) as usize;
                    let py = y_pos as usize;
                    if py > 0 && py < height as usize {
                        deblock_horizontal_edge(y, width as usize, px, py, qp);
                    }
                }
            }
        }
    }
}

/// Apply vertical deblocking filter at position (x, y).
///
/// Filters the edge between columns x-1 and x.
fn deblock_vertical_edge(y: &mut [u8], stride: usize, x: usize, y_pos: usize, qp: i8) {
    let idx = y_pos * stride + x;
    if x == 0 || x >= stride {
        return;
    }

    let _p3 = y[idx - 4 * stride] as i32;
    let p2 = y[idx - 3 * stride] as i32;
    let p1 = y[idx - 2 * stride] as i32;
    let p0 = y[idx - stride] as i32;
    let q0 = y[idx] as i32;
    let q1 = y[idx + stride] as i32;
    let q2 = y[idx + 2 * stride] as i32;
    let _q3 = y[idx + 3 * stride] as i32;

    let (p0_new, q0_new, p1_new, q1_new) = filter_edge(p0, q0, p1, q1, p2, q2, qp);

    y[idx - stride] = p0_new as u8;
    y[idx] = q0_new as u8;
    y[idx - 2 * stride] = p1_new as u8;
    y[idx + stride] = q1_new as u8;
}

/// Apply horizontal deblocking filter at position (x, y).
///
/// Filters the edge between rows y-1 and y.
fn deblock_horizontal_edge(y: &mut [u8], stride: usize, x: usize, y_pos: usize, qp: i8) {
    let idx = y_pos * stride + x;
    if y_pos == 0 || y_pos >= stride {
        return;
    }

    let _p3 = y[idx - 4] as i32;
    let p2 = y[idx - 3] as i32;
    let p1 = y[idx - 2] as i32;
    let p0 = y[idx - 1] as i32;
    let q0 = y[idx] as i32;
    let q1 = y[idx + 1] as i32;
    let q2 = y[idx + 2] as i32;
    let _q3 = y[idx + 3] as i32;

    let (p0_new, q0_new, p1_new, q1_new) = filter_edge(p0, q0, p1, q1, p2, q2, qp);

    y[idx - 1] = p0_new as u8;
    y[idx] = q0_new as u8;
    y[idx - 2] = p1_new as u8;
    y[idx + 1] = q1_new as u8;
}

/// Core deblocking filter for a single edge.
///
/// Returns filtered values (p0, q0, p1, q1).
fn filter_edge(
    p0: i32,
    q0: i32,
    p1: i32,
    q1: i32,
    p2: i32,
    q2: i32,
    qp: i8,
) -> (i32, i32, i32, i32) {
    // H.264 spec 8.7.2: filter decision
    let index_a = qp.clamp(0, 51) as usize;
    let index_b = qp.clamp(0, 51) as usize;

    let alpha = DEBLOCK_ALPHA[index_a] as i32;
    let beta = DEBLOCK_BETA[index_b] as i32;

    // Filter decision
    let ap = (p0 - p1).unsigned_abs() as i32;
    let aq = (q0 - q1).unsigned_abs() as i32;

    if ap >= alpha || aq >= beta {
        // No filtering
        return (p0, q0, p1, q1);
    }

    // Strong filter decision
    let ap2 = (p0 - p2).unsigned_abs() as i32;
    let aq2 = (q0 - q2).unsigned_abs() as i32;

    if ap2 < beta && aq2 < beta {
        // Strong filter (bS=4) - H.264 spec 8.7.2.4
        let p0_new = (p2 + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3;
        let q0_new = (q2 + 2 * q1 + 2 * q0 + 2 * p0 + p1 + 4) >> 3;
        let p1_new = (p2 + p1 + p0 + q0 + 2) >> 2;
        let q1_new = (q2 + q1 + q0 + p0 + 2) >> 2;
        (p0_new, q0_new, p1_new, q1_new)
    } else {
        // Normal filter (bS=1-3)
        let delta = (((q0 - p0) << 2) + (p1 - q1) + 4) >> 3;
        let tc0 = DEBLOCK_TC0[index_a] as i32;
        let tc = tc0 + (if ap < beta { 1 } else { 0 }) + (if aq2 < beta { 1 } else { 0 });

        let delta_clamped = delta.clamp(-tc, tc);
        let p0_new = (p0 + delta_clamped).clamp(0, 255);
        let q0_new = (q0 - delta_clamped).clamp(0, 255);

        // Filter p1, q1 only for stronger filtering
        let p1_new = if ap < beta {
            let delta_p = (p1 + ((p0 + q0 + 1) >> 1) - 2 * p1).clamp(-tc0, tc0);
            (p1 + delta_p).clamp(0, 255)
        } else {
            p1
        };

        let q1_new = if aq < beta {
            let delta_q = (q1 + ((p0 + q0 + 1) >> 1) - 2 * q1).clamp(-tc0, tc0);
            (q1 + delta_q).clamp(0, 255)
        } else {
            q1
        };

        (p0_new, q0_new, p1_new, q1_new)
    }
}

/// Alpha threshold table for deblocking filter (H.264 spec Table 8-16).
const DEBLOCK_ALPHA: [u8; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 4, 5, 6, 7, 8, 9, 10, 12, 13, 15, 17, 20,
    22, 25, 28, 32, 36, 40, 45, 50, 56, 63, 71, 80, 90, 101, 113, 127, 144, 162, 182, 203, 226,
    255, 255,
];

/// Beta threshold table for deblocking filter (H.264 spec Table 8-16).
const DEBLOCK_BETA: [u8; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 6, 6, 7, 7, 8, 8,
    9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18,
];

/// TC0 table for deblocking filter (H.264 spec Table 8-17).
const DEBLOCK_TC0: [u8; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 6, 6, 7, 8, 9, 10, 11, 13,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_edge_identity() {
        // When there's no edge (all same), filter should not change values
        let (p0, q0, p1, q1) = filter_edge(128, 128, 128, 128, 128, 128, 26);
        assert_eq!(p0, 128);
        assert_eq!(q0, 128);
        assert_eq!(p1, 128);
        assert_eq!(q1, 128);
    }

    #[test]
    fn filter_edge_reduces_discontinuity() {
        // Test that the filter reduces a sharp edge
        let p0 = 100i32;
        let q0 = 200i32;
        let p1 = 100i32;
        let q1 = 200i32;
        let p2 = 100i32;
        let q2 = 200i32;

        let (p0_new, q0_new, _, _) = filter_edge(p0, q0, p1, q1, p2, q2, 26);

        // The filter should move p0 up and q0 down
        assert!(p0_new > p0, "p0 should increase");
        assert!(q0_new < q0, "q0 should decrease");
    }

    #[test]
    fn alpha_beta_tables_valid() {
        assert_eq!(DEBLOCK_ALPHA.len(), 52);
        assert_eq!(DEBLOCK_BETA.len(), 52);
        assert_eq!(DEBLOCK_TC0.len(), 52);
    }
}
