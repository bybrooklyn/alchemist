use std::fmt;

/// Quality metric type for comparing source and encoded frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityMetric {
    VmafCompatible,
    WhyThoNative,
}

/// Per-frame quality measurement result.
#[derive(Debug, Clone)]
pub struct FrameQuality {
    pub frame_index: usize,
    pub psnr_y: f64,
    pub psnr_u: f64,
    pub psnr_v: f64,
    pub mae_y: f64,
    pub max_error_y: u8,
}

impl fmt::Display for FrameQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "frame {}: PSNR Y={:.2} dB, U={:.2} dB, V={:.2} dB, MAE Y={:.2}, max err={}",
            self.frame_index, self.psnr_y, self.psnr_u, self.psnr_v, self.mae_y, self.max_error_y
        )
    }
}

/// Aggregate quality report across all compared frames.
#[derive(Debug, Clone)]
pub struct QualityReport {
    pub frames_compared: usize,
    pub avg_psnr_y: f64,
    pub min_psnr_y: f64,
    pub max_psnr_y: f64,
    pub avg_mae_y: f64,
    pub max_error_y: u8,
    pub per_frame: Vec<FrameQuality>,
}

impl QualityReport {
    /// Returns true if quality meets the given PSNR threshold.
    pub fn passes(&self, min_psnr: f64) -> bool {
        self.avg_psnr_y >= min_psnr
    }
}

impl fmt::Display for QualityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Quality ({} frames):", self.frames_compared)?;
        writeln!(
            f,
            "  PSNR Y: avg={:.2} dB, min={:.2}, max={:.2}",
            self.avg_psnr_y, self.min_psnr_y, self.max_psnr_y
        )?;
        writeln!(f, "  MAE Y:  avg={:.2}", self.avg_mae_y)?;
        writeln!(f, "  Max pixel error: {}", self.max_error_y)?;
        Ok(())
    }
}

/// Compute PSNR (Peak Signal-to-Noise Ratio) for a single plane.
/// Returns infinity if the planes are identical (MSE = 0).
pub fn psnr_plane(original: &[u8], encoded: &[u8]) -> f64 {
    assert_eq!(original.len(), encoded.len(), "plane size mismatch");
    let mse: f64 = original
        .iter()
        .zip(encoded.iter())
        .map(|(&o, &e)| {
            let diff = o as f64 - e as f64;
            diff * diff
        })
        .sum::<f64>()
        / original.len() as f64;
    if mse == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0f64.powi(2) / mse).log10()
    }
}

/// Compute Mean Absolute Error for a single plane.
pub fn mae_plane(original: &[u8], encoded: &[u8]) -> f64 {
    assert_eq!(original.len(), encoded.len(), "plane size mismatch");
    let sum: u64 = original
        .iter()
        .zip(encoded.iter())
        .map(|(&o, &e)| (o as i32 - e as i32).unsigned_abs() as u64)
        .sum();
    sum as f64 / original.len() as f64
}

/// Compute maximum absolute pixel error for a single plane.
pub fn max_error_plane(original: &[u8], encoded: &[u8]) -> u8 {
    original
        .iter()
        .zip(encoded.iter())
        .map(|(&o, &e)| (o as i32 - e as i32).unsigned_abs() as u8)
        .max()
        .unwrap_or(0)
}

/// Measure quality of encoded frames against original frames.
/// Compares up to `max_frames` frames. If the frame counts differ,
/// compares the overlapping portion.
pub fn measure_quality(
    original: &[crate::transcode::DecodedYuv],
    encoded: &[crate::transcode::DecodedYuv],
    max_frames: Option<usize>,
) -> QualityReport {
    let compare_count = original
        .len()
        .min(encoded.len())
        .min(max_frames.unwrap_or(usize::MAX));

    let mut per_frame = Vec::with_capacity(compare_count);
    let mut sum_psnr = 0.0f64;
    let mut min_psnr = f64::MAX;
    let mut max_psnr = 0.0f64;
    let mut sum_mae = 0.0f64;
    let mut global_max_err = 0u8;

    for i in 0..compare_count {
        let o = &original[i];
        let e = &encoded[i];

        let psnr_y = psnr_plane(&o.y, &e.y);
        let psnr_u = psnr_plane(&o.u, &e.u);
        let psnr_v = psnr_plane(&o.v, &e.v);
        let mae_y = mae_plane(&o.y, &e.y);
        let max_err = max_error_plane(&o.y, &e.y);

        sum_psnr += psnr_y;
        min_psnr = min_psnr.min(psnr_y);
        max_psnr = max_psnr.max(psnr_y);
        sum_mae += mae_y;
        global_max_err = global_max_err.max(max_err);

        per_frame.push(FrameQuality {
            frame_index: i,
            psnr_y,
            psnr_u,
            psnr_v,
            mae_y,
            max_error_y: max_err,
        });
    }

    QualityReport {
        frames_compared: compare_count,
        avg_psnr_y: if compare_count > 0 {
            sum_psnr / compare_count as f64
        } else {
            0.0
        },
        min_psnr_y: if compare_count > 0 { min_psnr } else { 0.0 },
        max_psnr_y: if compare_count > 0 { max_psnr } else { 0.0 },
        avg_mae_y: if compare_count > 0 {
            sum_mae / compare_count as f64
        } else {
            0.0
        },
        max_error_y: global_max_err,
        per_frame,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psnr_identical_is_infinite() {
        let a = vec![128u8; 100];
        assert_eq!(psnr_plane(&a, &a), f64::INFINITY);
    }

    #[test]
    fn psnr_decreases_with_error() {
        let a = vec![128u8; 100];
        let mut b = vec![128u8; 100];
        b[0] = 100;
        let psnr = psnr_plane(&a, &b);
        assert!(psnr < 100.0);
        assert!(psnr > 0.0);
    }

    #[test]
    fn mae_constant_offset() {
        let a = vec![100u8; 100];
        let b = vec![110u8; 100];
        assert!((mae_plane(&a, &b) - 10.0).abs() < 0.01);
    }

    #[test]
    fn max_error_finds_peak() {
        let a = vec![128u8; 100];
        let mut b = vec![128u8; 100];
        b[50] = 200;
        assert_eq!(max_error_plane(&a, &b), 72);
    }
}
