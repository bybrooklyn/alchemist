//! FFmpeg wrapper module for Alchemist
//! Provides typed FFmpeg commands, encoder detection, and structured progress parsing.

use crate::error::{AlchemistError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use tracing::{debug, info, warn};

/// Available hardware accelerators detected from FFmpeg
#[derive(Debug, Clone, Default)]
pub struct HardwareAccelerators {
    pub available: HashSet<String>,
}

impl HardwareAccelerators {
    /// Detect available hardware accelerators via `ffmpeg -hwaccels`
    pub fn detect() -> Result<Self> {
        let output = Command::new("ffmpeg")
            .args(["-hide_banner", "-hwaccels"])
            .output()
            .map_err(|e| AlchemistError::FFmpeg(format!("Failed to run ffmpeg -hwaccels: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut available = HashSet::new();

        for line in stdout.lines().skip(1) {
            let accel = line.trim();
            if !accel.is_empty() {
                available.insert(accel.to_string());
            }
        }

        info!("Detected hardware accelerators: {:?}", available);
        Ok(Self { available })
    }

    pub fn has(&self, accel: &str) -> bool {
        self.available.contains(accel)
    }

    pub fn has_qsv(&self) -> bool {
        self.has("qsv")
    }

    pub fn has_cuda(&self) -> bool {
        self.has("cuda")
    }

    pub fn has_vaapi(&self) -> bool {
        self.has("vaapi")
    }

    pub fn has_videotoolbox(&self) -> bool {
        self.has("videotoolbox")
    }
}

/// Available encoders detected from FFmpeg
#[derive(Debug, Clone, Default)]
pub struct EncoderCapabilities {
    pub video_encoders: HashSet<String>,
    pub audio_encoders: HashSet<String>,
}

impl EncoderCapabilities {
    /// Detect available encoders via `ffmpeg -encoders`
    pub fn detect() -> Result<Self> {
        let output = Command::new("ffmpeg")
            .args(["-hide_banner", "-encoders"])
            .output()
            .map_err(|e| AlchemistError::FFmpeg(format!("Failed to run ffmpeg -encoders: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut video_encoders = HashSet::new();
        let mut audio_encoders = HashSet::new();

        for line in stdout.lines() {
            let line = line.trim();
            if line.len() < 8 {
                continue;
            }

            // Format: " V..... encoder_name  Description"
            let flags = &line[..6];
            let rest = line[6..].trim();
            let encoder_name = rest.split_whitespace().next().unwrap_or("");

            if flags.starts_with(" V") {
                video_encoders.insert(encoder_name.to_string());
            } else if flags.starts_with(" A") {
                audio_encoders.insert(encoder_name.to_string());
            }
        }

        debug!(
            "Detected {} video encoders, {} audio encoders",
            video_encoders.len(),
            audio_encoders.len()
        );

        Ok(Self {
            video_encoders,
            audio_encoders,
        })
    }

    pub fn has_video_encoder(&self, name: &str) -> bool {
        self.video_encoders.contains(name)
    }

    pub fn has_av1_qsv(&self) -> bool {
        self.has_video_encoder("av1_qsv")
    }

    pub fn has_av1_nvenc(&self) -> bool {
        self.has_video_encoder("av1_nvenc")
    }

    pub fn has_av1_vaapi(&self) -> bool {
        self.has_video_encoder("av1_vaapi")
    }

    pub fn has_av1_videotoolbox(&self) -> bool {
        self.has_video_encoder("av1_videotoolbox")
    }

    pub fn has_libsvtav1(&self) -> bool {
        self.has_video_encoder("libsvtav1")
    }
}

/// Quality profile presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityProfile {
    Quality,   // Slower, better quality
    Balanced,  // Default balance
    Speed,     // Faster, acceptable quality
}

impl Default for QualityProfile {
    fn default() -> Self {
        Self::Balanced
    }
}

impl QualityProfile {
    /// Get FFmpeg preset/CRF values for CPU encoding (libsvtav1)
    pub fn cpu_params(&self) -> (&'static str, &'static str) {
        match self {
            Self::Quality => ("4", "24"),
            Self::Balanced => ("8", "28"),
            Self::Speed => ("12", "32"),
        }
    }

    /// Get FFmpeg quality value for Intel QSV
    pub fn qsv_quality(&self) -> &'static str {
        match self {
            Self::Quality => "20",
            Self::Balanced => "25",
            Self::Speed => "30",
        }
    }

    /// Get FFmpeg preset for NVIDIA NVENC
    pub fn nvenc_preset(&self) -> &'static str {
        match self {
            Self::Quality => "p7",
            Self::Balanced => "p4",
            Self::Speed => "p1",
        }
    }
}

/// Parsed FFmpeg progress from stderr
#[derive(Debug, Clone, Default)]
pub struct FFmpegProgress {
    pub frame: u64,
    pub fps: f64,
    pub bitrate: String,
    pub total_size: u64,
    pub time: String,
    pub time_seconds: f64,
    pub speed: String,
}

impl FFmpegProgress {
    /// Parse a line of FFmpeg stderr for progress info
    pub fn parse_line(line: &str) -> Option<Self> {
        if !line.contains("time=") {
            return None;
        }

        let mut progress = Self::default();

        for part in line.split_whitespace() {
            if let Some((key, value)) = part.split_once('=') {
                match key {
                    "frame" => progress.frame = value.parse().unwrap_or(0),
                    "fps" => progress.fps = value.parse().unwrap_or(0.0),
                    "bitrate" => progress.bitrate = value.to_string(),
                    "total_size" => progress.total_size = value.parse().unwrap_or(0),
                    "time" => {
                        progress.time = value.to_string();
                        progress.time_seconds = Self::parse_time(value);
                    }
                    "speed" => progress.speed = value.to_string(),
                    _ => {}
                }
            }
        }

        Some(progress)
    }

    /// Parse time string (HH:MM:SS.ms) to seconds
    fn parse_time(s: &str) -> f64 {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return 0.0;
        }

        let hours: f64 = parts[0].parse().unwrap_or(0.0);
        let minutes: f64 = parts[1].parse().unwrap_or(0.0);
        let seconds: f64 = parts[2].parse().unwrap_or(0.0);

        hours * 3600.0 + minutes * 60.0 + seconds
    }

    /// Calculate percentage complete given total duration
    pub fn percentage(&self, total_duration: f64) -> f64 {
        if total_duration <= 0.0 {
            return 0.0;
        }
        (self.time_seconds / total_duration * 100.0).min(100.0)
    }
}

/// VMAF quality score result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScore {
    pub vmaf: Option<f64>,
    pub psnr: Option<f64>,
    pub ssim: Option<f64>,
}

impl QualityScore {
    /// Run VMAF quality comparison between original and encoded file
    pub fn compute(original: &Path, encoded: &Path) -> Result<Self> {
        info!("Computing quality metrics for {:?}", encoded);

        // Use FFmpeg's libvmaf filter
        let output = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-i", encoded.to_str().unwrap_or(""),
                "-i", original.to_str().unwrap_or(""),
                "-lavfi", "libvmaf=log_fmt=json:log_path=-",
                "-f", "null",
                "-"
            ])
            .output()
            .map_err(|e| AlchemistError::FFmpeg(format!("Failed to run VMAF: {}", e)))?;

        // Parse VMAF score from output
        let stderr = String::from_utf8_lossy(&output.stderr);
        let vmaf = Self::extract_vmaf_score(&stderr);

        if vmaf.is_none() {
            warn!("Could not extract VMAF score from output");
        }

        Ok(Self {
            vmaf,
            psnr: None, // Could add PSNR filter too
            ssim: None, // Could add SSIM filter too
        })
    }

    fn extract_vmaf_score(output: &str) -> Option<f64> {
        // Look for "VMAF score:" in the output
        for line in output.lines() {
            if line.contains("VMAF score:") {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 2 {
                    return parts[1].trim().parse().ok();
                }
            }
        }
        None
    }

    /// Check if quality is acceptable (VMAF >= threshold)
    pub fn is_acceptable(&self, min_vmaf: f64) -> bool {
        self.vmaf.map(|v| v >= min_vmaf).unwrap_or(true)
    }
}

/// Encode statistics for a completed job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodeStats {
    pub input_size_bytes: u64,
    pub output_size_bytes: u64,
    pub compression_ratio: f64,
    pub encode_time_seconds: f64,
    pub encode_speed: f64, // x speed
    pub avg_bitrate_kbps: f64,
    pub quality_score: Option<QualityScore>,
}

impl EncodeStats {
    pub fn new(
        input_size_bytes: u64,
        output_size_bytes: u64,
        encode_time_seconds: f64,
        duration_seconds: f64,
    ) -> Self {
        let compression_ratio = if input_size_bytes > 0 {
            1.0 - (output_size_bytes as f64 / input_size_bytes as f64)
        } else {
            0.0
        };

        let encode_speed = if encode_time_seconds > 0.0 {
            duration_seconds / encode_time_seconds
        } else {
            0.0
        };

        let avg_bitrate_kbps = if duration_seconds > 0.0 {
            (output_size_bytes as f64 * 8.0) / (duration_seconds * 1000.0)
        } else {
            0.0
        };

        Self {
            input_size_bytes,
            output_size_bytes,
            compression_ratio,
            encode_time_seconds,
            encode_speed,
            avg_bitrate_kbps,
            quality_score: None,
        }
    }

    pub fn with_quality(mut self, score: QualityScore) -> Self {
        self.quality_score = Some(score);
        self
    }
}

/// Verify FFmpeg is available and return version info
pub fn verify_ffmpeg() -> Result<String> {
    let output = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map_err(|e| AlchemistError::FFmpeg(format!("FFmpeg not found: {}", e)))?;

    if !output.status.success() {
        return Err(AlchemistError::FFmpeg("FFmpeg returned error".into()));
    }

    let version = String::from_utf8_lossy(&output.stdout);
    let first_line = version.lines().next().unwrap_or("unknown");
    
    info!("FFmpeg version: {}", first_line);
    Ok(first_line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_parsing() {
        let line = "frame=  100 fps=25.0 bitrate=1500kbps total_size=1000000 time=00:00:04.00 speed=1.5x";
        let progress = FFmpegProgress::parse_line(line).unwrap();
        
        assert_eq!(progress.frame, 100);
        assert_eq!(progress.fps, 25.0);
        assert_eq!(progress.time, "00:00:04.00");
        assert!((progress.time_seconds - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_quality_profile_cpu_params() {
        let (preset, crf) = QualityProfile::Quality.cpu_params();
        assert_eq!(preset, "4");
        assert_eq!(crf, "24");
    }
}
