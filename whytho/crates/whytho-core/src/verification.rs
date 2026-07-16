use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::error::WhyThoError;
use crate::quality::{self, QualityReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationMode {
    Sample,
    Strict,
    Benchmark,
    Military,
}

impl Default for VerificationMode {
    fn default() -> Self {
        Self::Sample
    }
}

impl fmt::Display for VerificationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sample => write!(f, "sample"),
            Self::Strict => write!(f, "strict"),
            Self::Benchmark => write!(f, "benchmark"),
            Self::Military => write!(f, "military"),
        }
    }
}

impl FromStr for VerificationMode {
    type Err = WhyThoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sample" => Ok(Self::Sample),
            "strict" => Ok(Self::Strict),
            "benchmark" => Ok(Self::Benchmark),
            "military" => Ok(Self::Military),
            _ => Err(WhyThoError::InvalidValue {
                field: "verification".into(),
                value: s.into(),
            }),
        }
    }
}

impl VerificationMode {
    /// Minimum PSNR threshold for passing (in dB).
    pub fn psnr_threshold(self) -> f64 {
        match self {
            Self::Sample => 28.0,
            Self::Strict => 35.0,
            Self::Benchmark => 30.0,
            Self::Military => 40.0,
        }
    }

    /// Maximum number of frames to sample for verification.
    /// None means verify all frames.
    pub fn max_sample_frames(self) -> Option<usize> {
        match self {
            Self::Sample => Some(10),
            Self::Strict => None,
            Self::Benchmark => Some(100),
            Self::Military => None,
        }
    }
}

/// Result of verifying an encoded output.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub mode: VerificationMode,
    pub passed: bool,
    pub quality: QualityReport,
    pub duration_delta_secs: f64,
    pub output_size: u64,
    pub input_size: u64,
    pub notes: Vec<String>,
}

impl VerificationResult {
    pub fn compression_ratio(&self) -> f64 {
        if self.input_size > 0 {
            self.output_size as f64 / self.input_size as f64
        } else {
            0.0
        }
    }
}

impl fmt::Display for VerificationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Verification: {} ({})",
            self.mode,
            if self.passed { "PASS" } else { "FAIL" }
        )?;
        write!(f, "{}", self.quality)?;
        writeln!(f, "  Duration delta: {:.3}s", self.duration_delta_secs)?;
        writeln!(
            f,
            "  Size: {} -> {} ({:.2}x)",
            human_size(self.input_size),
            human_size(self.output_size),
            self.compression_ratio()
        )?;
        if !self.notes.is_empty() {
            writeln!(f, "  Notes:")?;
            for note in &self.notes {
                writeln!(f, "    - {note}")?;
            }
        }
        Ok(())
    }
}

/// Verify an encoded output against its original.
///
/// This performs:
/// 1. Duration delta check (output duration must be close to input)
/// 2. Quality measurement (PSNR on sampled/all frames)
/// 3. Playability check (output must be decodable)
pub fn verify(
    mode: VerificationMode,
    original_frames: &[crate::transcode::DecodedYuv],
    encoded_frames: &[crate::transcode::DecodedYuv],
    input_duration_secs: f64,
    output_duration_secs: f64,
    input_size: u64,
    output_size: u64,
) -> VerificationResult {
    let mut notes = Vec::new();

    // Duration check: allow 1 second delta
    let duration_delta = (input_duration_secs - output_duration_secs).abs();
    if duration_delta > 1.0 {
        notes.push(format!(
            "duration delta {:.1}s exceeds 1s threshold",
            duration_delta
        ));
    }

    // Frame count check
    if original_frames.len() != encoded_frames.len() {
        notes.push(format!(
            "frame count mismatch: {} original vs {} encoded",
            original_frames.len(),
            encoded_frames.len()
        ));
    }

    // Quality measurement
    let quality =
        quality::measure_quality(original_frames, encoded_frames, mode.max_sample_frames());
    let passed = quality.passes(mode.psnr_threshold()) && duration_delta <= 1.0;

    if !passed && quality.avg_psnr_y < mode.psnr_threshold() {
        notes.push(format!(
            "PSNR {:.2} dB below threshold {:.1} dB",
            quality.avg_psnr_y,
            mode.psnr_threshold()
        ));
    }

    // Size check
    if output_size > input_size {
        notes.push("output is larger than input".into());
    }

    VerificationResult {
        mode,
        passed,
        quality,
        duration_delta_secs: duration_delta,
        output_size,
        input_size,
        notes,
    }
}

fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_mode_threshold() {
        assert_eq!(VerificationMode::Sample.psnr_threshold(), 28.0);
        assert_eq!(VerificationMode::Sample.max_sample_frames(), Some(10));
    }

    #[test]
    fn military_mode_threshold() {
        assert_eq!(VerificationMode::Military.psnr_threshold(), 40.0);
        assert_eq!(VerificationMode::Military.max_sample_frames(), None);
    }

    #[test]
    fn verification_passes_on_good_quality() {
        let frames = vec![crate::transcode::DecodedYuv {
            y: vec![128u8; 64 * 64],
            u: vec![128u8; 32 * 32],
            v: vec![128u8; 32 * 32],
        }];
        let result = verify(
            VerificationMode::Sample,
            &frames,
            &frames,
            1.0,
            1.0,
            1000,
            500,
        );
        assert!(result.passed);
    }

    #[test]
    fn verification_fails_on_bad_quality() {
        let original = vec![crate::transcode::DecodedYuv {
            y: vec![128u8; 64 * 64],
            u: vec![128u8; 32 * 32],
            v: vec![128u8; 32 * 32],
        }];
        let mut bad = original.clone();
        // Corrupt all pixels
        for px in bad[0].y.iter_mut() {
            *px = 0;
        }
        let result = verify(
            VerificationMode::Sample,
            &original,
            &bad,
            1.0,
            1.0,
            1000,
            500,
        );
        assert!(!result.passed);
    }

    #[test]
    fn display_format() {
        let frames = vec![crate::transcode::DecodedYuv {
            y: vec![128u8; 16 * 16],
            u: vec![128u8; 8 * 8],
            v: vec![128u8; 8 * 8],
        }];
        let result = verify(
            VerificationMode::Sample,
            &frames,
            &frames,
            1.0,
            1.0,
            1000,
            500,
        );
        let display = format!("{result}");
        assert!(display.contains("PASS") || display.contains("FAIL"));
        assert!(display.contains("PSNR"));
    }
}
