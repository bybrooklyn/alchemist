use crate::error::{AlchemistError, Result};
use crate::media::pipeline::{Analyzer as AnalyzerTrait, MediaMetadata};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct FfprobeMetadata {
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
    pub channel_layout: Option<String>,
    pub channels: Option<u32>,
    pub r_frame_rate: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Format {
    pub format_name: String,
    pub format_long_name: Option<String>,
    pub duration: String,
    pub size: String,
    pub bit_rate: String,
}

pub struct FfmpegAnalyzer;

#[async_trait]
impl AnalyzerTrait for FfmpegAnalyzer {
    async fn analyze(&self, path: &Path) -> Result<MediaMetadata> {
        let path = path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let output = Command::new("ffprobe")
                .args([
                    "-v",
                    "quiet",
                    "-print_format",
                    "json",
                    "-show_format",
                    "-show_streams",
                ])
                .arg(&path)
                .output()
                .map_err(|e| AlchemistError::Analyzer(format!("Failed to run ffprobe: {}", e)))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(AlchemistError::Analyzer(format!("ffprobe failed: {}", err)));
            }

            let metadata: FfprobeMetadata =
                serde_json::from_slice(&output.stdout).map_err(|e| {
                    AlchemistError::Analyzer(format!("Failed to parse ffprobe JSON: {}", e))
                })?;

            let video_stream = metadata
                .streams
                .iter()
                .find(|s| s.codec_type == "video")
                .ok_or_else(|| AlchemistError::Analyzer("No video stream found".to_string()))?;

            let audio_stream = metadata.streams.iter().find(|s| s.codec_type == "audio");

            Ok(MediaMetadata {
                path: path.clone(),
                duration_secs: metadata.format.duration.parse().unwrap_or(0.0),
                codec_name: video_stream.codec_name.clone(),
                width: video_stream.width.unwrap_or(0),
                height: video_stream.height.unwrap_or(0),
                bit_depth: video_stream
                    .bits_per_raw_sample
                    .as_deref()
                    .unwrap_or("8")
                    .parse()
                    .unwrap_or(8),
                size_bytes: metadata.format.size.parse().unwrap_or(0),
                bit_rate: video_stream
                    .bit_rate
                    .as_deref()
                    .or(Some(&metadata.format.bit_rate))
                    .and_then(|b| b.parse().ok())
                    .unwrap_or(0.0),
                fps: video_stream
                    .r_frame_rate
                    .as_deref()
                    .and_then(Analyzer::parse_fps)
                    .unwrap_or(24.0),
                container: metadata.format.format_name.clone(),
                audio_codec: audio_stream.map(|s| s.codec_name.clone()),
                audio_channels: audio_stream.and_then(|s| s.channels),
            })
        })
        .await
        .map_err(|e| AlchemistError::Analyzer(format!("spawn_blocking failed: {}", e)))?
    }
}

pub struct Analyzer;

impl Analyzer {
    // Keep legacy probe for now to avoid breaking too much at once, or use it internally
    pub fn probe(path: &Path) -> Result<FfprobeMetadata> {
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(path)
            .output()
            .map_err(|e| AlchemistError::Analyzer(format!("Failed to run ffprobe: {}", e)))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(AlchemistError::Analyzer(format!("ffprobe failed: {}", err)));
        }

        let metadata: FfprobeMetadata = serde_json::from_slice(&output.stdout).map_err(|e| {
            AlchemistError::Analyzer(format!("Failed to parse ffprobe JSON: {}", e))
        })?;

        Ok(metadata)
    }

    /// Async version of probe that doesn't block the runtime
    pub async fn probe_async(path: &Path) -> Result<FfprobeMetadata> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || Self::probe(&path))
            .await
            .map_err(|e| AlchemistError::Analyzer(format!("spawn_blocking failed: {}", e)))?
    }

    // ... should_transcode adapted below ...

    pub fn should_transcode(
        _path: &Path,
        metadata: &MediaMetadata,
        config: &crate::config::Config,
    ) -> (bool, String) {
        // 1. Codec Check (skip if already AV1 + 10-bit)
        if metadata.codec_name == "av1" && metadata.bit_depth == 10 {
            return (false, "Already AV1 10-bit".to_string());
        }

        // 2. Efficiency Rules (BPP)
        let bitrate = metadata.bit_rate;
        let width = metadata.width as f64;
        let height = metadata.height as f64;
        let fps = metadata.fps;

        if width == 0.0 || height == 0.0 || bitrate == 0.0 {
            return (
                false,
                "Incomplete metadata (bitrate/resolution)".to_string(),
            );
        }

        let bpp = bitrate / (width * height * fps);

        // Normalize BPP based on resolution (4K needs less BPP than 1080p for same quality)
        let res_correction = if width >= 3840.0 {
            0.6 // 4K
        } else if width >= 1920.0 {
            0.8 // 1080p
        } else {
            1.0 // 720p and below
        };
        let normalized_bpp = bpp * res_correction;

        // Heuristic: If BPP is already very low, don't murder it further.
        if normalized_bpp < config.transcode.min_bpp_threshold {
            return (
                false,
                format!(
                    "BPP too low ({:.4} normalized < {:.2}), avoiding quality murder",
                    normalized_bpp, config.transcode.min_bpp_threshold
                ),
            );
        }

        // 4. Projected Size Logic
        let size_bytes = metadata.size_bytes;
        let min_size_bytes = config.transcode.min_file_size_mb * 1024 * 1024;
        if size_bytes < min_size_bytes {
            return (
                false,
                format!(
                    "File too small ({}MB < {}MB) to justify transcode overhead",
                    size_bytes / 1024 / 1024,
                    config.transcode.min_file_size_mb
                ),
            );
        }

        (
            true,
            format!(
                "Ready for AV1 transcode (Current codec: {}, BPP: {:.4})",
                metadata.codec_name, bpp
            ),
        )
    }

    fn parse_fps(s: &str) -> Option<f64> {
        if s.contains('/') {
            let parts: Vec<&str> = s.split('/').collect();
            if parts.len() == 2 {
                let num: f64 = parts[0].parse().ok()?;
                let den: f64 = parts[1].parse().ok()?;
                if den == 0.0 {
                    return None;
                }
                return Some(num / den);
            }
        }
        s.parse().ok()
    }

    pub fn should_transcode_audio(stream: &Stream) -> bool {
        if stream.codec_type != "audio" {
            return false;
        }

        // Transcode if it's a "heavy" codec or very high bitrate
        let heavy_codecs = ["truehd", "dts-hd", "flac", "pcm_s24le", "pcm_s16le"];
        if heavy_codecs.contains(&stream.codec_name.to_lowercase().as_str()) {
            return true;
        }

        let bitrate = stream
            .bit_rate
            .as_ref()
            .and_then(|b| b.parse::<u64>().ok())
            .unwrap_or(0);

        // If bitrate > 640kbps (standard AC3 max), maybe transcode?
        if bitrate > 640000 {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fps() {
        assert_eq!(Analyzer::parse_fps("24/1"), Some(24.0));
        assert_eq!(Analyzer::parse_fps("23.976"), Some(23.976));
        assert_eq!(Analyzer::parse_fps("60000/1001"), Some(60000.0 / 1001.0));
        assert_eq!(Analyzer::parse_fps("invalid"), None);
        assert_eq!(Analyzer::parse_fps("24/0"), None);
    }

    #[test]
    fn test_should_transcode_audio() {
        let heavy = Stream {
            codec_name: "truehd".into(),
            codec_type: "audio".into(),
            width: None,
            height: None,
            bit_rate: None,
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            r_frame_rate: None,
        };
        assert!(Analyzer::should_transcode_audio(&heavy));

        let standard = Stream {
            codec_name: "ac3".into(),
            codec_type: "audio".into(),
            width: None,
            height: None,
            bit_rate: Some("384000".into()),
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            r_frame_rate: None,
        };
        assert!(!Analyzer::should_transcode_audio(&standard));

        let high_bitrate_ac3 = Stream {
            codec_name: "ac3".into(),
            codec_type: "audio".into(),
            width: None,
            height: None,
            bit_rate: Some("1000000".into()),
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            r_frame_rate: None,
        };
        assert!(Analyzer::should_transcode_audio(&high_bitrate_ac3));
    }
}
