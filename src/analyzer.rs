use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
// use tracing::info; // Removed unused import

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub streams: Vec<Stream>,
    pub format: Format,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Stream {
    pub codec_name: String,
    pub codec_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bit_rate: Option<String>,
    pub bits_per_raw_sample: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Format {
    pub duration: String,
    pub size: String,
    pub bit_rate: String,
}

pub struct Analyzer;

impl Analyzer {
    pub fn probe(path: &Path) -> Result<MediaMetadata> {
        let output = Command::new("ffprobe")
            .args(&[
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(path)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("ffprobe failed: {}", err));
        }

        let metadata: MediaMetadata = serde_json::from_slice(&output.stdout)?;
        Ok(metadata)
    }

    pub fn should_transcode(_path: &Path, metadata: &MediaMetadata) -> (bool, String) {
        // 1. Codec Check (skip if already AV1 + 10-bit)
        let video_stream = metadata.streams.iter().find(|s| s.codec_type == "video");
        
        let video_stream = match video_stream {
            Some(v) => v,
            None => return (false, "No video stream found".to_string()),
        };

        let bit_depth = video_stream.bits_per_raw_sample.as_deref().unwrap_or("8");
        if video_stream.codec_name == "av1" && bit_depth == "10" {
            return (false, "Already AV1 10-bit".to_string());
        }

        // 2. Resolution logic (don't upscale)
        // For Phase 1, we target AV1 10-bit as the gold standard.
        
        // 3. Efficiency Rules (BPP)
        // BPP = Bitrate / (Width * Height * Framerate)
        // We'll simplify for now as framerate is tricky from ffprobe without more flags.
        // Let's use bits per pixel: Bitrate / (Width * Height)
        let bitrate = video_stream.bit_rate.as_ref()
            .and_then(|b| b.parse::<f64>().ok())
            .or_else(|| metadata.format.bit_rate.parse::<f64>().ok())
            .unwrap_or(0.0);

        let width = video_stream.width.unwrap_or(0) as f64;
        let height = video_stream.height.unwrap_or(0) as f64;

        if width == 0.0 || height == 0.0 || bitrate == 0.0 {
            return (false, "Incomplete metadata (bitrate/resolution)".to_string());
        }

        let bpp = bitrate / (width * height);
        
        // Heuristic: If BPP is already very low, don't murder it further.
        // threshold 0.1 is very low for h264, maybe 0.05 for av1.
        if bpp < 0.1 {
            return (false, format!("BPP too low ({:.4}), avoiding quality murder", bpp));
        }

        // 4. Projected Size Logic
        // Target AV1 is roughly 30-50% smaller than H264 for same quality.
        // If it's already a small file (e.g. under 50MB), maybe skip?
        let size_bytes = metadata.format.size.parse::<u64>().unwrap_or(0);
        if size_bytes < 50 * 1024 * 1024 {
            return (false, "File too small to justify transcode overhead".to_string());
        }

        (true, format!("Ready for AV1 transcode (Current codec: {}, BPP: {:.4})", video_stream.codec_name, bpp))
    }
}
