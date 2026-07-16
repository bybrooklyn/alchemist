use std::path::PathBuf;
use std::time::Duration;

use crate::chunking::ChunkPlan;
use crate::config::WhyThoConfig;
use crate::media::MediaInput;
use crate::probe::ProbeResult;
use crate::scheduler::JobPriority;
use crate::verification::{VerificationMode, VerificationResult};

/// Raw YUV frame data for quality comparison.
/// This is a lightweight type that holds decoded pixel data without
/// codec-specific metadata.
#[derive(Debug, Clone)]
pub struct DecodedYuv {
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

impl DecodedYuv {
    pub fn width(&self) -> u32 {
        // Assuming 4:2:0: Y width = 2 * U width
        let uv_w = (self.u.len() as f64).sqrt() as u32;
        uv_w * 2
    }

    pub fn height(&self) -> u32 {
        let uv_h = (self.u.len() as f64 / (self.width() / 2) as f64) as u32;
        uv_h * 2
    }
}

#[derive(Debug, Clone)]
pub struct TranscodeJob {
    pub id: u64,
    pub input: MediaInput,
    pub output: PathBuf,
    pub config: WhyThoConfig,
    pub probe: ProbeResult,
    pub priority: JobPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscodeStatus {
    Pending,
    Planning,
    Encoding,
    Verifying,
    Finalizing,
    Complete,
    Failed,
    Cancelled,
}

impl TranscodeStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Failed | Self::Cancelled)
    }
}

impl std::fmt::Display for TranscodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Planning => write!(f, "planning"),
            Self::Encoding => write!(f, "encoding"),
            Self::Verifying => write!(f, "verifying"),
            Self::Finalizing => write!(f, "finalizing"),
            Self::Complete => write!(f, "complete"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TranscodeProgress {
    pub status: TranscodeStatus,
    pub frames_encoded: u64,
    pub total_frames: Option<u64>,
    pub elapsed: Duration,
    pub current_fps: f64,
    pub bytes_written: u64,
}

impl TranscodeProgress {
    pub fn percent(&self) -> Option<f64> {
        self.total_frames.map(|total| {
            if total > 0 {
                self.frames_encoded as f64 / total as f64 * 100.0
            } else {
                0.0
            }
        })
    }

    pub fn eta(&self) -> Option<Duration> {
        let total = self.total_frames?;
        if self.frames_encoded >= total || self.current_fps <= 0.0 {
            return None;
        }
        let remaining = total - self.frames_encoded;
        Some(Duration::from_secs_f64(remaining as f64 / self.current_fps))
    }
}

#[derive(Debug, Clone)]
pub struct TranscodeReport {
    pub input: PathBuf,
    pub output: PathBuf,
    pub frames_encoded: u64,
    pub elapsed: Duration,
    pub avg_fps: f64,
    pub output_size: u64,
    pub input_size: u64,
    pub status: TranscodeStatus,
    pub verification: Option<VerificationResult>,
    pub chunk_plan: Option<ChunkPlan>,
}

impl TranscodeReport {
    pub fn compression_ratio(&self) -> f64 {
        if self.input_size > 0 {
            self.output_size as f64 / self.input_size as f64
        } else {
            0.0
        }
    }
}

impl std::fmt::Display for TranscodeReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Transcode: {}", self.input.display())?;
        writeln!(f, "  Output: {}", self.output.display())?;
        writeln!(f, "  Status: {}", self.status)?;
        writeln!(f, "  Frames: {}", self.frames_encoded)?;
        writeln!(
            f,
            "  Time: {:.1}s ({:.1} fps)",
            self.elapsed.as_secs_f64(),
            self.avg_fps
        )?;
        writeln!(
            f,
            "  Size: {} -> {} ({:.2}x)",
            human_size(self.input_size),
            human_size(self.output_size),
            self.compression_ratio()
        )?;
        if let Some(ref chunk) = self.chunk_plan {
            writeln!(f, "  Chunks: {}", chunk.chunk_count())?;
        }
        if let Some(ref verify) = self.verification {
            write!(f, "{verify}")?;
        }
        Ok(())
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

/// Callback trait for transcoding progress reporting.
pub trait TranscodeObserver {
    fn on_status_change(&self, _status: TranscodeStatus) {}
    fn on_progress(&self, _progress: &TranscodeProgress) {}
    fn on_frame_encoded(&self, _frame_index: u64) {}
}

/// No-op observer for when progress reporting is not needed.
pub struct NullObserver;
impl TranscodeObserver for NullObserver {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcode_status_is_terminal() {
        assert!(TranscodeStatus::Complete.is_terminal());
        assert!(TranscodeStatus::Failed.is_terminal());
        assert!(TranscodeStatus::Cancelled.is_terminal());
        assert!(!TranscodeStatus::Pending.is_terminal());
        assert!(!TranscodeStatus::Encoding.is_terminal());
    }

    #[test]
    fn progress_percent() {
        let p = TranscodeProgress {
            status: TranscodeStatus::Encoding,
            frames_encoded: 50,
            total_frames: Some(100),
            elapsed: Duration::from_secs(10),
            current_fps: 5.0,
            bytes_written: 1000,
        };
        assert!((p.percent().unwrap() - 50.0).abs() < 0.01);
    }

    #[test]
    fn progress_eta() {
        let p = TranscodeProgress {
            status: TranscodeStatus::Encoding,
            frames_encoded: 50,
            total_frames: Some(100),
            elapsed: Duration::from_secs(10),
            current_fps: 10.0,
            bytes_written: 1000,
        };
        let eta = p.eta().unwrap();
        assert!((eta.as_secs_f64() - 5.0).abs() < 0.1);
    }

    #[test]
    fn progress_eta_none_when_no_total() {
        let p = TranscodeProgress {
            status: TranscodeStatus::Encoding,
            frames_encoded: 50,
            total_frames: None,
            elapsed: Duration::from_secs(10),
            current_fps: 5.0,
            bytes_written: 1000,
        };
        assert!(p.eta().is_none());
    }
}
