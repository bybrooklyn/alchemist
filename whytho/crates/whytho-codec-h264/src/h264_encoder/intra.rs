//! Intra prediction for H.264 encoder.
//!
//! Implements all 9 I4x4 prediction modes, 4 I16x16 modes,
//! chroma DC prediction, and SATD-based mode decision.

use super::{Intra4x4Mode, Intra16x16Mode, Macroblock};
use crate::DecodedFrame;

pub struct Neighbors4x4 {
    pub above: [u8; 4],
    pub left: [u8; 4],
    pub top_left: u8,
    pub above_right: [u8; 4],
    pub has_above: bool,
    pub has_left: bool,
    pub has_above_right: bool,
}

/// Gather the source-frame neighbour samples for the 4x4 block at (bx, by) within the
/// macroblock at (mb_x, mb_y), reading absolute picture coordinates directly from
/// `frame`. Blocks on the macroblock's own top row / left column need samples from the
/// already-encoded neighbouring macroblock, not the current macroblock's own buffer —
/// sampling `frame` directly (like `extract_luma_neighbors` does for I16x16) gets both
/// the interior (same-MB) and boundary (cross-MB) cases right uniformly. These are
/// *source* (not reconstructed) neighbours — the same documented approximation used for
/// I16x16, acceptable for intra-only output.
pub fn extract_neighbors_4x4(
    frame: &DecodedFrame,
    mb_x: u32,
    mb_y: u32,
    bx: usize,
    by: usize,
) -> Neighbors4x4 {
    let w = frame.width as usize;
    let h = frame.height as usize;
    let px = |x: usize, y: usize| -> u8 {
        let xx = x.min(w.saturating_sub(1));
        let yy = y.min(h.saturating_sub(1));
        frame.y.get(yy * w + xx).copied().unwrap_or(128)
    };

    let abs_x = mb_x as usize * 16 + bx * 4;
    let abs_y = mb_y as usize * 16 + by * 4;

    let has_above = abs_y > 0;
    let has_left = abs_x > 0;
    let has_above_right = abs_y > 0 && (abs_x + 7) < w;

    let mut above = [128u8; 4];
    let mut left = [128u8; 4];
    let mut top_left = 128u8;
    let mut above_right = [128u8; 4];

    if has_above {
        for (i, s) in above.iter_mut().enumerate() {
            *s = px(abs_x + i, abs_y - 1);
        }
    }

    if has_left {
        for (i, s) in left.iter_mut().enumerate() {
            *s = px(abs_x - 1, abs_y + i);
        }
    }

    if has_above && has_left {
        top_left = px(abs_x - 1, abs_y - 1);
    }

    if has_above_right {
        for (i, s) in above_right.iter_mut().enumerate() {
            *s = px(abs_x + 4 + i, abs_y - 1);
        }
    }

    Neighbors4x4 {
        above,
        left,
        top_left,
        above_right,
        has_above,
        has_left,
        has_above_right,
    }
}

pub fn predict_4x4(mode: Intra4x4Mode, n: &Neighbors4x4) -> [[u8; 4]; 4] {
    match mode {
        Intra4x4Mode::Vertical => predict_vertical_4x4(&n.above),
        Intra4x4Mode::Horizontal => predict_horizontal_4x4(&n.left),
        Intra4x4Mode::Dc => predict_dc_4x4(
            if n.has_above { Some(&n.above) } else { None },
            if n.has_left { Some(&n.left) } else { None },
        ),
        Intra4x4Mode::DiagonalDownLeft => predict_diagonal_down_left_4x4(&n.above, &n.above_right),
        Intra4x4Mode::DiagonalDownRight => {
            predict_diagonal_down_right_4x4(&n.above, &n.left, n.top_left)
        }
        Intra4x4Mode::VerticalRight => predict_vertical_right_4x4(&n.above, &n.left, n.top_left),
        Intra4x4Mode::HorizontalDown => predict_horizontal_down_4x4(&n.above, &n.left, n.top_left),
        Intra4x4Mode::VerticalLeft => predict_vertical_left_4x4(&n.above, &n.above_right),
        Intra4x4Mode::HorizontalUp => predict_horizontal_up_4x4(&n.left),
    }
}

fn predict_vertical_4x4(above: &[u8; 4]) -> [[u8; 4]; 4] {
    let mut pred = [[0u8; 4]; 4];
    for row in 0..4 {
        pred[row] = *above;
    }
    pred
}

fn predict_horizontal_4x4(left: &[u8; 4]) -> [[u8; 4]; 4] {
    let mut pred = [[0u8; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            pred[row][col] = left[row];
        }
    }
    pred
}

pub fn predict_dc_4x4(above: Option<&[u8; 4]>, left: Option<&[u8; 4]>) -> [[u8; 4]; 4] {
    let mut sum = 0u32;
    let mut count = 0u32;

    if let Some(a) = above {
        for &v in a.iter() {
            sum += v as u32;
            count += 1;
        }
    }
    if let Some(l) = left {
        for &v in l.iter() {
            sum += v as u32;
            count += 1;
        }
    }

    let dc = if count > 0 {
        ((sum + count / 2) / count) as u8
    } else {
        128
    };

    [[dc; 4]; 4]
}

fn predict_diagonal_down_left_4x4(above: &[u8; 4], above_right: &[u8; 4]) -> [[u8; 4]; 4] {
    let mut pred = [[0u8; 4]; 4];
    let mut p = [0u8; 9];
    p[..4].copy_from_slice(above);
    p[4..8].copy_from_slice(above_right);
    p[8] = above_right[3];

    pred[0][0] = avg3(p[0], p[1], p[2]);
    pred[0][1] = avg3(p[1], p[2], p[3]);
    pred[0][2] = avg3(p[2], p[3], p[4]);
    pred[0][3] = avg3(p[3], p[4], p[5]);
    pred[1][0] = avg3(p[1], p[2], p[3]);
    pred[1][1] = avg3(p[2], p[3], p[4]);
    pred[1][2] = avg3(p[3], p[4], p[5]);
    pred[1][3] = avg3(p[4], p[5], p[6]);
    pred[2][0] = avg3(p[2], p[3], p[4]);
    pred[2][1] = avg3(p[3], p[4], p[5]);
    pred[2][2] = avg3(p[4], p[5], p[6]);
    pred[2][3] = avg3(p[5], p[6], p[7]);
    pred[3][0] = avg3(p[3], p[4], p[5]);
    pred[3][1] = avg3(p[4], p[5], p[6]);
    pred[3][2] = avg3(p[5], p[6], p[7]);
    pred[3][3] = avg3(p[6], p[7], p[8]);
    pred
}

fn predict_diagonal_down_right_4x4(above: &[u8; 4], left: &[u8; 4], top_left: u8) -> [[u8; 4]; 4] {
    let mut pred = [[0u8; 4]; 4];

    pred[0][0] = avg3(left[0], top_left, above[0]);
    pred[0][1] = avg3(top_left, above[0], above[1]);
    pred[0][2] = avg3(above[0], above[1], above[2]);
    pred[0][3] = avg3(above[1], above[2], above[3]);
    pred[1][0] = avg3(top_left, left[0], left[1]);
    pred[1][1] = avg3(left[0], top_left, above[0]);
    pred[1][2] = pred[0][0];
    pred[1][3] = pred[0][1];
    pred[2][0] = avg3(left[1], left[0], top_left);
    pred[2][1] = pred[1][0];
    pred[2][2] = pred[1][1];
    pred[2][3] = pred[0][0];
    pred[3][0] = avg3(left[2], left[1], left[0]);
    pred[3][1] = pred[2][0];
    pred[3][2] = pred[1][0];
    pred[3][3] = pred[1][1];
    pred
}

fn predict_vertical_right_4x4(above: &[u8; 4], left: &[u8; 4], top_left: u8) -> [[u8; 4]; 4] {
    let mut pred = [[0u8; 4]; 4];

    pred[0][0] = avg2(top_left, above[0]);
    pred[0][1] = avg2(above[0], above[1]);
    pred[0][2] = avg2(above[1], above[2]);
    pred[0][3] = avg2(above[2], above[3]);
    pred[1][0] = avg3(left[0], top_left, above[0]);
    pred[1][1] = avg3(top_left, above[0], above[1]);
    pred[1][2] = avg3(above[0], above[1], above[2]);
    pred[1][3] = avg3(above[1], above[2], above[3]);
    pred[2][0] = avg3(top_left, left[0], left[1]);
    pred[2][1] = avg3(left[0], top_left, above[0]);
    pred[2][2] = pred[0][0];
    pred[2][3] = pred[0][1];
    pred[3][0] = avg3(left[1], left[0], top_left);
    pred[3][1] = pred[1][0];
    pred[3][2] = pred[1][1];
    pred[3][3] = pred[1][2];
    pred
}

fn predict_horizontal_down_4x4(above: &[u8; 4], left: &[u8; 4], top_left: u8) -> [[u8; 4]; 4] {
    let mut pred = [[0u8; 4]; 4];

    pred[0][0] = avg3(left[0], top_left, above[0]);
    pred[0][1] = avg3(top_left, above[0], above[1]);
    pred[0][2] = avg3(above[0], above[1], above[2]);
    pred[0][3] = avg3(above[1], above[2], above[3]);
    pred[1][0] = avg2(top_left, left[0]);
    pred[1][1] = pred[0][0];
    pred[1][2] = pred[0][1];
    pred[1][3] = pred[0][2];
    pred[2][0] = avg2(left[0], left[1]);
    pred[2][1] = avg3(left[0], top_left, above[0]);
    pred[2][2] = pred[1][1];
    pred[2][3] = pred[1][2];
    pred[3][0] = avg2(left[1], left[2]);
    pred[3][1] = pred[2][0];
    pred[3][2] = pred[2][1];
    pred[3][3] = pred[2][2];
    pred
}

fn predict_vertical_left_4x4(above: &[u8; 4], above_right: &[u8; 4]) -> [[u8; 4]; 4] {
    let mut pred = [[0u8; 4]; 4];

    pred[0][0] = avg2(above[0], above[1]);
    pred[0][1] = avg2(above[1], above[2]);
    pred[0][2] = avg2(above[2], above[3]);
    pred[0][3] = avg2(above[3], above_right[0]);
    pred[1][0] = avg3(above[0], above[1], above[2]);
    pred[1][1] = avg3(above[1], above[2], above[3]);
    pred[1][2] = avg3(above[2], above[3], above_right[0]);
    pred[1][3] = avg3(above[3], above_right[0], above_right[1]);
    pred[2][0] = avg2(above[1], above[2]);
    pred[2][1] = pred[0][2];
    pred[2][2] = avg2(above[3], above_right[0]);
    pred[2][3] = avg2(above_right[0], above_right[1]);
    pred[3][0] = avg3(above[1], above[2], above[3]);
    pred[3][1] = pred[1][2];
    pred[3][2] = avg3(above[3], above_right[0], above_right[1]);
    pred[3][3] = avg3(above_right[0], above_right[1], above_right[2]);
    pred
}

fn predict_horizontal_up_4x4(left: &[u8; 4]) -> [[u8; 4]; 4] {
    let mut pred = [[0u8; 4]; 4];

    pred[0][0] = avg2(left[0], left[1]);
    pred[0][1] = avg3(left[0], left[1], left[2]);
    pred[0][2] = avg2(left[1], left[2]);
    pred[0][3] = avg3(left[1], left[2], left[3]);
    pred[1][0] = avg2(left[1], left[2]);
    pred[1][1] = avg3(left[1], left[2], left[3]);
    pred[1][2] = avg2(left[2], left[3]);
    pred[1][3] = avg3(left[2], left[3], left[3]);
    pred[2][0] = avg2(left[2], left[3]);
    pred[2][1] = avg3(left[2], left[3], left[3]);
    pred[2][2] = left[3];
    pred[2][3] = left[3];
    pred[3][0] = avg3(left[2], left[3], left[3]);
    pred[3][1] = left[3];
    pred[3][2] = left[3];
    pred[3][3] = left[3];
    pred
}

pub fn predict_dc_16x16(mb: &Macroblock) -> [[u8; 16]; 16] {
    let mut sum = 0u32;
    let mut count = 0u32;

    for row in 0..16 {
        sum += mb.y[row][0] as u32;
        count += 1;
    }

    for col in 0..16 {
        sum += mb.y[0][col] as u32;
        count += 1;
    }

    sum -= mb.y[0][0] as u32;
    count -= 1;

    let dc = if count > 0 {
        ((sum + count / 2) / count) as u8
    } else {
        128
    };

    [[dc; 16]; 16]
}

pub fn predict_vertical_16x16(mb: &Macroblock) -> [[u8; 16]; 16] {
    let mut pred = [[0u8; 16]; 16];
    for row in 0..16 {
        pred[row] = mb.y[0];
    }
    pred
}

pub fn predict_horizontal_16x16(mb: &Macroblock) -> [[u8; 16]; 16] {
    let mut pred = [[0u8; 16]; 16];
    for row in 0..16 {
        for col in 0..16 {
            pred[row][col] = mb.y[row][0];
        }
    }
    pred
}

pub fn predict_plane_16x16(mb: &Macroblock) -> [[u8; 16]; 16] {
    let mut pred = [[0u8; 16]; 16];

    let mut h = 0i32;
    for i in 0..8 {
        let right = mb.y[0][8 + i] as i32;
        let left = if i < 7 { mb.y[0][6 - i] as i32 } else { 128 };
        h += (i as i32 + 1) * (right - left);
    }

    let mut v = 0i32;
    for i in 0..8 {
        let below = mb.y[8 + i][0] as i32;
        let above = if i < 7 { mb.y[6 - i][0] as i32 } else { 128 };
        v += (i as i32 + 1) * (below - above);
    }

    let a = 16 * (mb.y[15][0] as i32 + mb.y[0][15] as i32);
    let b = (5 * h + 32) >> 6;
    let c = (5 * v + 32) >> 6;

    for row in 0..16i32 {
        for col in 0..16i32 {
            let val = (a + b * (col - 7) + c * (row - 7) + 16) >> 5;
            pred[row as usize][col as usize] = val.clamp(0, 255) as u8;
        }
    }
    pred
}

pub fn predict_16x16(mode: Intra16x16Mode, mb: &Macroblock) -> [[u8; 16]; 16] {
    match mode {
        Intra16x16Mode::Vertical => predict_vertical_16x16(mb),
        Intra16x16Mode::Horizontal => predict_horizontal_16x16(mb),
        Intra16x16Mode::Dc => predict_dc_16x16(mb),
        Intra16x16Mode::Plane => predict_plane_16x16(mb),
    }
}

/// Reconstructed/source neighbour samples for a macroblock, used to drive intra
/// prediction the same way the decoder does (H.264 8.3.3). `None` means the
/// neighbour is outside the frame and the decoder substitutes 128.
pub struct LumaNeighbors {
    pub above: Option<[u8; 16]>,
    pub left: Option<[u8; 16]>,
    pub above_left: Option<u8>,
    /// Row above, columns of the next macroblock to the right (16 samples).
    pub above_right: Option<[u8; 16]>,
}

/// I16x16 intra prediction from neighbour samples, byte-for-byte matching the
/// reference decoder's `predict_intra_16x16` (spec 8.3.3).
pub fn predict_intra_16x16_n(mode: Intra16x16Mode, n: &LumaNeighbors) -> [[u8; 16]; 16] {
    let mut out = [[0u8; 16]; 16];
    match mode {
        Intra16x16Mode::Vertical => {
            let a = n.above.unwrap_or([128; 16]);
            for row in out.iter_mut() {
                *row = a;
            }
        }
        Intra16x16Mode::Horizontal => {
            let l = n.left.unwrap_or([128; 16]);
            for row in 0..16 {
                out[row] = [l[row]; 16];
            }
        }
        Intra16x16Mode::Dc => {
            let dc = match (n.above, n.left) {
                (Some(a), Some(l)) => {
                    let sum: u32 = a.iter().map(|&x| x as u32).sum::<u32>()
                        + l.iter().map(|&x| x as u32).sum::<u32>();
                    ((sum + 16) >> 5) as u8
                }
                (Some(a), None) => {
                    let sum: u32 = a.iter().map(|&x| x as u32).sum();
                    ((sum + 8) >> 4) as u8
                }
                (None, Some(l)) => {
                    let sum: u32 = l.iter().map(|&x| x as u32).sum();
                    ((sum + 8) >> 4) as u8
                }
                (None, None) => 128,
            };
            out = [[dc; 16]; 16];
        }
        Intra16x16Mode::Plane => {
            let a = n.above.unwrap_or([128; 16]);
            let l = n.left.unwrap_or([128; 16]);
            let p = n.above_left.unwrap_or(128);

            let mut h = 0i32;
            let mut v = 0i32;
            for i in 0..8 {
                let above_neg = if i < 7 { a[6 - i] as i32 } else { p as i32 };
                let left_neg = if i < 7 { l[6 - i] as i32 } else { p as i32 };
                h += (i as i32 + 1) * (a[8 + i] as i32 - above_neg);
                v += (i as i32 + 1) * (l[8 + i] as i32 - left_neg);
            }
            let a_val = 16 * (a[15] as i32 + l[15] as i32);
            let b_val = (5 * h + 32) >> 6;
            let c_val = (5 * v + 32) >> 6;
            for (y, row) in out.iter_mut().enumerate() {
                for (x, px) in row.iter_mut().enumerate() {
                    let val = (a_val + b_val * (x as i32 - 7) + c_val * (y as i32 - 7) + 16) >> 5;
                    *px = val.clamp(0, 255) as u8;
                }
            }
        }
    }
    out
}

/// Pick the I16x16 mode (from neighbour-based prediction) with the lowest SAD
/// against the source macroblock. Returns the mode and its prediction.
pub fn choose_best_i16x16_n(
    mb: &Macroblock,
    n: &LumaNeighbors,
) -> (Intra16x16Mode, [[u8; 16]; 16]) {
    let modes = [
        Intra16x16Mode::Dc,
        Intra16x16Mode::Vertical,
        Intra16x16Mode::Horizontal,
        Intra16x16Mode::Plane,
    ];
    let mut best = (
        Intra16x16Mode::Dc,
        predict_intra_16x16_n(Intra16x16Mode::Dc, n),
    );
    let mut best_cost = u32::MAX;
    for &mode in &modes {
        let pred = predict_intra_16x16_n(mode, n);
        let mut cost = 0u32;
        for row in 0..16 {
            for col in 0..16 {
                cost += (mb.y[row][col] as i32 - pred[row][col] as i32).unsigned_abs();
            }
        }
        if cost < best_cost {
            best_cost = cost;
            best = (mode, pred);
        }
    }
    best
}

pub fn predict_dc_chroma(block: &[[u8; 8]; 8], width: usize, height: usize) -> [[u8; 8]; 8] {
    let mut sum = 0u32;
    let mut count = 0u32;

    for row in 0..height {
        sum += block[row][0] as u32;
        count += 1;
    }

    for col in 0..width {
        sum += block[0][col] as u32;
        count += 1;
    }

    sum -= block[0][0] as u32;
    count -= 1;

    let dc = if count > 0 {
        ((sum + count / 2) / count) as u8
    } else {
        128
    };

    [[dc; 8]; 8]
}

fn avg2(a: u8, b: u8) -> u8 {
    ((a as u32 + b as u32 + 1) >> 1) as u8
}

fn avg3(a: u8, b: u8, c: u8) -> u8 {
    ((a as u32 + 2 * b as u32 + c as u32 + 2) >> 2) as u8
}

pub fn satd_4x4(a: &[[u8; 4]; 4], b: &[[u8; 4]; 4]) -> u32 {
    let mut diff = [[0i16; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            diff[row][col] = a[row][col] as i16 - b[row][col] as i16;
        }
    }

    hadamard_4x4_inplace(&mut diff);

    let mut satd = 0u32;
    for row in 0..4 {
        for col in 0..4 {
            satd += diff[row][col].unsigned_abs() as u32;
        }
    }
    (satd + 1) >> 1
}

fn hadamard_4x4_inplace(block: &mut [[i16; 4]; 4]) {
    for row in 0..4 {
        let a = block[row][0] as i32 + block[row][3] as i32;
        let b = block[row][1] as i32 + block[row][2] as i32;
        let c = block[row][0] as i32 - block[row][3] as i32;
        let d = block[row][1] as i32 - block[row][2] as i32;
        block[row][0] = (a + b) as i16;
        block[row][1] = (c + d) as i16;
        block[row][2] = (a - b) as i16;
        block[row][3] = (c - d) as i16;
    }

    for col in 0..4 {
        let a = block[0][col] as i32 + block[3][col] as i32;
        let b = block[1][col] as i32 + block[2][col] as i32;
        let c = block[0][col] as i32 - block[3][col] as i32;
        let d = block[1][col] as i32 - block[2][col] as i32;
        block[0][col] = (a + b) as i16;
        block[1][col] = (c + d) as i16;
        block[2][col] = (a - b) as i16;
        block[3][col] = (c - d) as i16;
    }
}

pub fn choose_best_i4x4_mode(
    original: &[[u8; 4]; 4],
    neighbors: &Neighbors4x4,
) -> (Intra4x4Mode, [[u8; 4]; 4]) {
    let modes = [
        Intra4x4Mode::Dc,
        Intra4x4Mode::Vertical,
        Intra4x4Mode::Horizontal,
        Intra4x4Mode::DiagonalDownLeft,
        Intra4x4Mode::DiagonalDownRight,
        Intra4x4Mode::VerticalRight,
        Intra4x4Mode::HorizontalDown,
        Intra4x4Mode::VerticalLeft,
        Intra4x4Mode::HorizontalUp,
    ];

    let mut best_mode = Intra4x4Mode::Dc;
    let mut best_pred = [[0u8; 4]; 4];
    let mut best_cost = u32::MAX;

    for &mode in &modes {
        let pred = predict_4x4(mode, neighbors);
        let cost = satd_4x4(original, &pred);
        if cost < best_cost {
            best_cost = cost;
            best_mode = mode;
            best_pred = pred;
        }
    }

    (best_mode, best_pred)
}

pub fn choose_best_i16x16_mode(mb: &Macroblock) -> (Intra16x16Mode, [[u8; 16]; 16]) {
    let modes = [
        Intra16x16Mode::Dc,
        Intra16x16Mode::Vertical,
        Intra16x16Mode::Horizontal,
        Intra16x16Mode::Plane,
    ];

    let mut best_mode = Intra16x16Mode::Dc;
    let mut best_pred = [[0u8; 16]; 16];
    let mut best_cost = u32::MAX;

    for &mode in &modes {
        let pred = predict_16x16(mode, mb);
        let mut cost = 0u32;
        for row in 0..16 {
            for col in 0..16 {
                let diff = mb.y[row][col] as i32 - pred[row][col] as i32;
                cost += diff.unsigned_abs();
            }
        }
        if cost < best_cost {
            best_cost = cost;
            best_mode = mode;
            best_pred = pred;
        }
    }

    (best_mode, best_pred)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::h264_encoder::MbType;
    use proptest::prelude::*;

    /// Build a Macroblock whose luma plane is filled from a simple LCG seed.
    fn mb_from_seed(seed: u64) -> Macroblock {
        let mut state = seed | 1;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            (state >> 33) as u8
        };
        let mut y = [[0u8; 16]; 16];
        for row in y.iter_mut() {
            for px in row.iter_mut() {
                *px = next();
            }
        }
        Macroblock {
            mb_type: MbType::I4x4,
            mb_x: 0,
            mb_y: 0,
            qp: 26,
            y,
            cb: [[128u8; 8]; 8],
            cr: [[128u8; 8]; 8],
        }
    }

    #[test]
    fn dc_prediction_4x4_with_above_and_left() {
        let above = [10, 20, 30, 40];
        let left = [10, 20, 30, 40];
        let pred = predict_dc_4x4(Some(&above), Some(&left));
        assert_eq!(pred[0][0], 25);
        assert!(pred.iter().all(|row| row.iter().all(|&v| v == 25)));
    }

    #[test]
    fn dc_prediction_4x4_no_neighbors() {
        let pred = predict_dc_4x4(None, None);
        assert_eq!(pred[0][0], 128);
    }

    #[test]
    fn vertical_prediction_4x4() {
        let above = [10, 20, 30, 40];
        let pred = predict_vertical_4x4(&above);
        assert!(pred.iter().all(|row| *row == above));
    }

    #[test]
    fn horizontal_prediction_4x4() {
        let left = [10, 20, 30, 40];
        let pred = predict_horizontal_4x4(&left);
        for row in 0..4 {
            assert!(pred[row].iter().all(|&v| v == left[row]));
        }
    }

    #[test]
    fn diagonal_down_left_basic() {
        let above = [10, 20, 30, 40];
        let above_right = [50, 60, 70, 80];
        let pred = predict_diagonal_down_left_4x4(&above, &above_right);
        assert_eq!(pred[0][0], avg3(10, 20, 30));
        assert_eq!(pred[0][3], avg3(40, 50, 60));
    }

    #[test]
    fn diagonal_down_right_basic() {
        let above = [10, 20, 30, 40];
        let left = [50, 60, 70, 80];
        let top_left = 5;
        let pred = predict_diagonal_down_right_4x4(&above, &left, top_left);
        assert_eq!(pred[0][0], avg3(50, 5, 10));
        assert_eq!(pred[3][0], avg3(70, 60, 50));
    }

    #[test]
    fn satd_identical_blocks() {
        let a = [[100u8; 4]; 4];
        assert_eq!(satd_4x4(&a, &a), 0);
    }

    #[test]
    fn satd_constant_difference() {
        let a = [[100u8; 4]; 4];
        let b = [[50u8; 4]; 4];
        let satd = satd_4x4(&a, &b);
        assert!(satd > 0);
    }

    #[test]
    fn choose_best_mode_dc_for_constant() {
        let original = [[128u8; 4]; 4];
        let neighbors = Neighbors4x4 {
            above: [128; 4],
            left: [128; 4],
            top_left: 128,
            above_right: [128; 4],
            has_above: true,
            has_left: true,
            has_above_right: true,
        };
        let (mode, pred) = choose_best_i4x4_mode(&original, &neighbors);
        assert_eq!(mode, Intra4x4Mode::Dc);
        assert!(pred.iter().all(|row| row.iter().all(|&v| v == 128)));
    }

    #[test]
    fn choose_best_mode_vertical_for_top_aligned() {
        let above = [50, 60, 70, 80];
        let original = [above, above, above, above];
        let neighbors = Neighbors4x4 {
            above,
            left: [200; 4],
            top_left: 200,
            above_right: [200; 4],
            has_above: true,
            has_left: true,
            has_above_right: true,
        };
        let (mode, _pred) = choose_best_i4x4_mode(&original, &neighbors);
        assert_eq!(mode, Intra4x4Mode::Vertical);
    }

    #[test]
    fn predict_vertical_16x16_broadcasts_top_row() {
        let mut mb = mb_from_seed(1);
        let top: [u8; 16] = core::array::from_fn(|i| (i * 7) as u8);
        mb.y[0] = top;
        let pred = predict_vertical_16x16(&mb);
        assert!(pred.iter().all(|row| *row == top));
    }

    #[test]
    fn predict_horizontal_16x16_broadcasts_left_col() {
        let mut mb = mb_from_seed(2);
        for (r, row) in mb.y.iter_mut().enumerate() {
            row[0] = (r * 9) as u8;
        }
        let pred = predict_horizontal_16x16(&mb);
        for r in 0..16 {
            assert!(pred[r].iter().all(|&v| v == mb.y[r][0]));
        }
    }

    #[test]
    fn predict_dc_16x16_constant() {
        let mb = Macroblock {
            mb_type: MbType::I4x4,
            mb_x: 0,
            mb_y: 0,
            qp: 26,
            y: [[128u8; 16]; 16],
            cb: [[128u8; 8]; 8],
            cr: [[128u8; 8]; 8],
        };
        let pred = predict_dc_16x16(&mb);
        assert!(pred.iter().all(|row| row.iter().all(|&v| v == 128)));
    }

    #[test]
    fn predict_dc_chroma_constant() {
        let pred = predict_dc_chroma(&[[64u8; 8]; 8], 8, 8);
        assert!(pred.iter().all(|row| row.iter().all(|&v| v == 64)));
    }

    #[test]
    fn horizontal_up_4x4_known() {
        let left = [10u8, 20, 30, 40];
        let pred = predict_horizontal_up_4x4(&left);
        assert_eq!(pred[0][0], avg2(10, 20));
        assert_eq!(pred[2][2], 40); // left[3]
        assert_eq!(pred[3][3], 40); // left[3]
    }

    #[test]
    fn vertical_left_4x4_known() {
        let above = [10u8, 20, 30, 40];
        let above_right = [50u8, 60, 70, 80];
        let pred = predict_vertical_left_4x4(&above, &above_right);
        assert_eq!(pred[0][0], avg2(10, 20));
        assert_eq!(pred[0][3], avg2(40, 50));
    }

    proptest! {
        /// SATD is symmetric in its two operands (it sums |Hadamard of difference|).
        #[test]
        fn satd_symmetric(
            a in prop::array::uniform4(prop::array::uniform4(any::<u8>())),
            b in prop::array::uniform4(prop::array::uniform4(any::<u8>())),
        ) {
            prop_assert_eq!(satd_4x4(&a, &b), satd_4x4(&b, &a));
        }

        /// The I4x4 selector must return a prediction consistent with the mode it chose.
        #[test]
        fn choose_best_i4x4_returns_matching_pred(
            original in prop::array::uniform4(prop::array::uniform4(any::<u8>())),
            above in prop::array::uniform4(any::<u8>()),
            left in prop::array::uniform4(any::<u8>()),
            top_left in any::<u8>(),
            above_right in prop::array::uniform4(any::<u8>()),
            has_above in any::<bool>(),
            has_left in any::<bool>(),
            has_above_right in any::<bool>(),
        ) {
            let neighbors = Neighbors4x4 {
                above, left, top_left, above_right,
                has_above, has_left, has_above_right,
            };
            let (mode, pred) = choose_best_i4x4_mode(&original, &neighbors);
            prop_assert_eq!(pred, predict_4x4(mode, &neighbors));
        }

        /// The I16x16 selector must return a prediction consistent with the mode it chose.
        #[test]
        fn choose_best_i16x16_returns_matching_pred(seed in any::<u64>()) {
            let mb = mb_from_seed(seed);
            let (mode, pred) = choose_best_i16x16_mode(&mb);
            prop_assert_eq!(pred, predict_16x16(mode, &mb));
        }
    }
}
