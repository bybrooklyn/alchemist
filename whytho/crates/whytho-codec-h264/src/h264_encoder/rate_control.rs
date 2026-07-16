//! Rate control for H.264 encoder.
//!
//! Implements adaptive QP selection based on target bitrate.
//! Uses a simple feedback controller: adjust QP based on whether
//! actual bits consumed exceed the target budget.

use std::collections::VecDeque;

/// Rate control state for the encoder.
#[derive(Debug)]
pub struct RateController {
    /// Target bitrate in bits per second.
    target_bitrate: u32,
    /// Frames per second.
    fps: f64,
    /// Current QP (updated each frame).
    current_qp: i8,
    /// Minimum QP allowed.
    min_qp: i8,
    /// Maximum QP allowed.
    max_qp: i8,
    /// Recent frame sizes (in bits) for averaging.
    recent_bits: VecDeque<u32>,
    /// Window size for averaging.
    window_size: usize,
    /// Target bits per frame.
    target_bits_per_frame: f64,
    /// Accumulated deviation from target (for integral control).
    accumulated_deviation: f64,
}

impl RateController {
    pub fn new(target_bitrate: u32, fps: f64, initial_qp: i8) -> Self {
        let target_bits_per_frame = target_bitrate as f64 / fps;
        Self {
            target_bitrate,
            fps,
            current_qp: initial_qp,
            min_qp: 1,
            max_qp: 51,
            recent_bits: VecDeque::with_capacity(30),
            window_size: 10,
            target_bits_per_frame,
            accumulated_deviation: 0.0,
        }
    }

    /// Get the current QP for encoding.
    pub fn qp(&self) -> i8 {
        self.current_qp
    }

    /// Update rate control after encoding a frame.
    /// `frame_bits` is the number of bits in the encoded frame.
    pub fn update(&mut self, frame_bits: u32) {
        self.recent_bits.push_back(frame_bits);
        if self.recent_bits.len() > self.window_size {
            self.recent_bits.pop_front();
        }

        // Compute average bits over recent window
        let avg_bits = self.recent_bits.iter().sum::<u32>() as f64 / self.recent_bits.len() as f64;

        // Deviation from target
        let deviation = avg_bits - self.target_bits_per_frame;
        let relative_deviation = deviation / self.target_bits_per_frame;

        // Accumulated deviation (integral term)
        self.accumulated_deviation += relative_deviation * 0.1;
        self.accumulated_deviation = self.accumulated_deviation.clamp(-5.0, 5.0);

        // Proportional + integral QP adjustment
        // Each QP step roughly doubles/halves the bitrate
        // So we adjust by ~1 QP per 50% deviation
        let qp_adjustment =
            (relative_deviation * 2.0 + self.accumulated_deviation * 0.5).round() as i8;

        self.current_qp =
            (self.current_qp.saturating_add(qp_adjustment)).clamp(self.min_qp, self.max_qp);
    }

    /// Get the target bitrate.
    pub fn target_bitrate(&self) -> u32 {
        self.target_bitrate
    }

    /// Get the target bits per frame.
    pub fn target_bits_per_frame(&self) -> f64 {
        self.target_bits_per_frame
    }

    /// Get the average bits per frame over the recent window.
    pub fn avg_bits_per_frame(&self) -> f64 {
        if self.recent_bits.is_empty() {
            return self.target_bits_per_frame;
        }
        self.recent_bits.iter().sum::<u32>() as f64 / self.recent_bits.len() as f64
    }

    /// Get the current bitrate estimate (based on recent frames).
    pub fn estimated_bitrate(&self) -> f64 {
        self.avg_bits_per_frame() * self.fps
    }
}

/// Map a target bitrate to an initial QP guess.
/// This provides a reasonable starting point for the rate controller.
pub fn initial_qp_for_bitrate(bitrate: u32, width: u32, height: u32) -> i8 {
    let pixels = (width * height) as f64;
    let bits_per_pixel = bitrate as f64 / (pixels * 24.0); // assume 24 fps

    // More granular mapping from bits/pixel to QP
    if bits_per_pixel > 10.0 {
        10 // lossless-quality
    } else if bits_per_pixel > 5.0 {
        14 // very high quality
    } else if bits_per_pixel > 2.0 {
        18 // high quality
    } else if bits_per_pixel > 1.0 {
        22 // good quality
    } else if bits_per_pixel > 0.5 {
        26 // medium quality
    } else if bits_per_pixel > 0.2 {
        30 // low-medium quality
    } else if bits_per_pixel > 0.1 {
        34 // low quality
    } else if bits_per_pixel > 0.05 {
        38 // very low quality
    } else {
        42 // minimum quality
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_controller_initial_qp() {
        let rc = RateController::new(2_000_000, 24.0, 26);
        assert_eq!(rc.qp(), 26);
    }

    #[test]
    fn rate_controller_increases_qp_on_overuse() {
        let mut rc = RateController::new(1_000_000, 24.0, 26);
        rc.target_bits_per_frame = 1000.0; // override for test

        // Feed frames that are way over budget
        for _ in 0..5 {
            rc.update(5000);
        }
        // QP should have increased
        assert!(
            rc.qp() > 26,
            "QP should increase when over budget, got {}",
            rc.qp()
        );
    }

    #[test]
    fn rate_controller_decreases_qp_on_underuse() {
        let mut rc = RateController::new(10_000_000, 24.0, 26);
        rc.target_bits_per_frame = 100000.0; // override for test

        // Feed frames that are way under budget
        for _ in 0..5 {
            rc.update(100);
        }
        // QP should have decreased
        assert!(
            rc.qp() < 26,
            "QP should decrease when under budget, got {}",
            rc.qp()
        );
    }

    #[test]
    fn rate_controller_stays_stable() {
        let mut rc = RateController::new(2_000_000, 24.0, 26);
        let target = rc.target_bits_per_frame();

        // Feed frames at exactly the target
        for _ in 0..20 {
            rc.update(target as u32);
        }
        // QP should be stable
        assert_eq!(rc.qp(), 26, "QP should stay stable at target");
    }

    #[test]
    fn rate_controller_qp_bounds() {
        let mut rc = RateController::new(100, 24.0, 26); // very low bitrate
        rc.target_bits_per_frame = 1.0;

        // Feed very large frames
        for _ in 0..20 {
            rc.update(100000);
        }
        assert!(rc.qp() <= 51, "QP should not exceed max");
        assert!(rc.qp() >= 1, "QP should not go below min");
    }

    #[test]
    fn initial_qp_mapping() {
        // 8Mbps at 1080p24 = ~0.16 bpp → QP 34
        assert_eq!(initial_qp_for_bitrate(8_000_000, 1920, 1080), 34);
        // 2Mbps at 1080p24 = ~0.04 bpp → QP 42
        assert_eq!(initial_qp_for_bitrate(2_000_000, 1920, 1080), 42);
        // 500kbps at 1080p24 = ~0.01 bpp → QP 42
        assert_eq!(initial_qp_for_bitrate(500_000, 1920, 1080), 42);
        // 50Mbps at 1080p24 = ~1.0 bpp → QP 22
        assert_eq!(initial_qp_for_bitrate(50_000_000, 1920, 1080), 22);
    }

    #[test]
    fn estimated_bitrate() {
        let mut rc = RateController::new(2_000_000, 24.0, 26);
        let target = rc.target_bits_per_frame();
        for _ in 0..5 {
            rc.update(target as u32);
        }
        let estimated = rc.estimated_bitrate();
        assert!(
            (estimated - 2_000_000.0).abs() < 100.0,
            "estimated should be close to target"
        );
    }
}
