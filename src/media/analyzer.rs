use crate::error::{AlchemistError, Result};
use crate::media::pipeline::{
    AnalysisConfidence, AnalysisWarning, Analyzer as AnalyzerTrait, DynamicRange, MediaAnalysis,
    MediaMetadata, TranscodeDecision,
};
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
    pub pix_fmt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub coded_width: Option<u32>,
    pub coded_height: Option<u32>,
    pub bit_rate: Option<String>,
    pub bits_per_raw_sample: Option<String>,
    pub channel_layout: Option<String>,
    pub channels: Option<u32>,
    pub avg_frame_rate: Option<String>,
    pub r_frame_rate: Option<String>,
    pub nb_frames: Option<String>,
    pub duration: Option<String>,
    pub disposition: Option<Disposition>,
    pub color_primaries: Option<String>,
    pub color_transfer: Option<String>,
    pub color_space: Option<String>,
    pub color_range: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Disposition {
    pub default: Option<i32>,
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
    async fn analyze(&self, path: &Path) -> Result<MediaAnalysis> {
        let path = path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let output = Command::new("ffprobe")
                .args([
                    "-v",
                    "quiet",
                    "-analyzeduration",
                    "1M",
                    "-probesize",
                    "1M",
                    "-print_format",
                    "json",
                    "-show_entries",
                    "format=duration,size,bit_rate,format_name,format_long_name:stream=codec_type,codec_name,pix_fmt,width,height,coded_width,coded_height,bit_rate,bits_per_raw_sample,channel_layout,channels,avg_frame_rate,r_frame_rate,nb_frames,duration,disposition,color_primaries,color_transfer,color_space,color_range",
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

            let video_stream = select_video_stream(&metadata.streams)
                .ok_or_else(|| AlchemistError::Analyzer("No video stream found".to_string()))?;

            let audio_stream = metadata.streams.iter().find(|s| s.codec_type == "audio");

            let color_transfer = video_stream.color_transfer.clone();
            let color_primaries = video_stream.color_primaries.clone();
            let dynamic_range =
                detect_dynamic_range(color_transfer.as_deref(), color_primaries.as_deref());

            let mut warnings = Vec::new();

            let fps = Analyzer::parse_fps(
                video_stream
                    .avg_frame_rate
                    .as_deref()
                    .or(video_stream.r_frame_rate.as_deref())
                    .unwrap_or(""),
            )
            .or_else(|| {
                let stream_duration = video_stream
                    .duration
                    .as_deref()
                    .and_then(parse_f64);
                let format_duration = parse_f64(&metadata.format.duration);
                let duration = stream_duration.or(format_duration)?;
                let frames = video_stream.nb_frames.as_deref().and_then(parse_f64)?;
                if duration > 0.0 {
                    Some(frames / duration)
                } else {
                    None
                }
            })
            .unwrap_or(0.0);

            let duration_secs = parse_f64(&metadata.format.duration)
                .or_else(|| video_stream.duration.as_deref().and_then(parse_f64))
                .or_else(|| {
                    let frames = video_stream.nb_frames.as_deref().and_then(parse_f64)?;
                    if fps > 0.0 {
                        Some(frames / fps)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);

            if video_stream.bit_rate.is_none() {
                warnings.push(AnalysisWarning::MissingVideoBitrate);
            }
            if metadata.format.bit_rate.parse::<u64>().is_err() {
                warnings.push(AnalysisWarning::MissingContainerBitrate);
            }
            if fps <= 0.0 {
                warnings.push(AnalysisWarning::MissingFps);
            }
            if duration_secs <= 0.0 {
                warnings.push(AnalysisWarning::MissingDuration);
            }
            if infer_bit_depth(video_stream).is_none() {
                warnings.push(AnalysisWarning::MissingBitDepth);
            }
            if video_stream
                .pix_fmt
                .as_deref()
                .is_some_and(|pix_fmt| bit_depth_from_pix_fmt(pix_fmt).is_none())
            {
                warnings.push(AnalysisWarning::UnrecognizedPixelFormat);
            }

            let confidence = if warnings.is_empty() {
                AnalysisConfidence::High
            } else if warnings.len() >= 3 {
                AnalysisConfidence::Low
            } else {
                AnalysisConfidence::Medium
            };

            let metadata = MediaMetadata {
                path: path.clone(),
                duration_secs,
                codec_name: video_stream.codec_name.clone(),
                width: video_stream
                    .width
                    .or(video_stream.coded_width)
                    .unwrap_or(0),
                height: video_stream
                    .height
                    .or(video_stream.coded_height)
                    .unwrap_or(0),
                bit_depth: infer_bit_depth(video_stream),
                color_primaries,
                color_transfer,
                color_space: video_stream.color_space.clone(),
                color_range: video_stream.color_range.clone(),
                dynamic_range,
                size_bytes: metadata.format.size.parse().unwrap_or(0),
                video_bitrate_bps: video_stream.bit_rate.as_deref().and_then(parse_u64),
                container_bitrate_bps: parse_u64(&metadata.format.bit_rate),
                fps,
                container: metadata.format.format_name.clone(),
                audio_codec: audio_stream.map(|s| s.codec_name.clone()),
                audio_channels: audio_stream.and_then(|s| s.channels),
            };

            Ok(MediaAnalysis {
                metadata,
                warnings,
                confidence,
            })
        })
        .await
        .map_err(|e| AlchemistError::Analyzer(format!("spawn_blocking failed: {}", e)))?
    }
}

pub struct Analyzer;

impl Analyzer {
    /// Async version of probe that doesn't block the runtime
    pub async fn probe_async(path: &Path) -> Result<FfprobeMetadata> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let output = Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-analyzeduration",
                    "1M",
                    "-probesize",
                    "1M",
                    "-print_format",
                    "json",
                    "-show_entries",
                    "format=duration,size,bit_rate,format_name,format_long_name:stream=codec_type,codec_name,pix_fmt,width,height,coded_width,coded_height,bit_rate,bits_per_raw_sample,channel_layout,channels,avg_frame_rate,r_frame_rate,nb_frames,duration,disposition,color_primaries,color_transfer,color_space,color_range",
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

            Ok(metadata)
        })
        .await
        .map_err(|e| AlchemistError::Analyzer(format!("spawn_blocking failed: {}", e)))?
    }

    pub async fn probe_video_codec(path: &Path) -> Result<String> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let output = Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-select_streams",
                    "v:0",
                    "-show_entries",
                    "stream=codec_name",
                    "-of",
                    "default=nokey=1:noprint_wrappers=1",
                ])
                .arg(&path)
                .output()
                .map_err(|e| AlchemistError::Analyzer(format!("Failed to run ffprobe: {}", e)))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(AlchemistError::Analyzer(format!("ffprobe failed: {}", err)));
            }

            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        })
        .await
        .map_err(|e| AlchemistError::Analyzer(format!("spawn_blocking failed: {}", e)))?
    }

    // ... should_transcode adapted below ...

    pub fn should_transcode(
        _path: &Path,
        metadata: &MediaMetadata,
        config: &crate::config::Config,
    ) -> TranscodeDecision {
        // 1. Codec Check (skip if already AV1 + 10-bit)
        if metadata.codec_name == "av1" && metadata.bit_depth == Some(10) {
            return TranscodeDecision::Skip {
                reason: "Already AV1 10-bit".to_string(),
            };
        }

        // 2. Efficiency Rules (BPP)
        let bitrate = metadata.video_bitrate_bps;
        let width = metadata.width as f64;
        let height = metadata.height as f64;
        let fps = metadata.fps;

        let bitrate = match bitrate {
            Some(bitrate) if bitrate > 0 => bitrate as f64,
            _ => {
                return TranscodeDecision::Skip {
                    reason: "Incomplete metadata (bitrate/resolution)".to_string(),
                };
            }
        };

        if width == 0.0 || height == 0.0 || fps <= 0.0 {
            return TranscodeDecision::Skip {
                reason: "Incomplete metadata (bitrate/resolution)".to_string(),
            };
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
            return TranscodeDecision::Skip {
                reason: format!(
                    "BPP too low ({:.4} normalized < {:.2}), avoiding quality murder",
                    normalized_bpp, config.transcode.min_bpp_threshold
                ),
            };
        }

        // 4. Projected Size Logic
        let size_bytes = metadata.size_bytes;
        let min_size_bytes = config.transcode.min_file_size_mb * 1024 * 1024;
        if size_bytes < min_size_bytes {
            return TranscodeDecision::Skip {
                reason: format!(
                    "File too small ({}MB < {}MB) to justify transcode overhead",
                    size_bytes / 1024 / 1024,
                    config.transcode.min_file_size_mb
                ),
            };
        }

        TranscodeDecision::Transcode {
            reason: format!(
                "Ready for AV1 transcode (Current codec: {}, BPP: {:.4})",
                metadata.codec_name, bpp
            ),
        }
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

fn parse_f64(s: &str) -> Option<f64> {
    s.parse().ok()
}

fn parse_u64(s: &str) -> Option<u64> {
    s.parse().ok()
}

fn infer_bit_depth(stream: &Stream) -> Option<u8> {
    if let Some(ref pix_fmt) = stream.pix_fmt {
        if let Some(depth) = bit_depth_from_pix_fmt(pix_fmt) {
            return Some(depth);
        }
    }

    stream
        .bits_per_raw_sample
        .as_deref()
        .and_then(|s| s.parse().ok())
}

fn bit_depth_from_pix_fmt(pix_fmt: &str) -> Option<u8> {
    let fmt = pix_fmt.to_ascii_lowercase();
    let depth_candidates = [
        (16u8, ["p16", "p016", "16le", "16be"]),
        (14u8, ["p14", "p014", "14le", "14be"]),
        (12u8, ["p12", "p012", "12le", "12be"]),
        (10u8, ["p10", "p010", "10le", "10be"]),
        (9u8, ["p09", "p9", "9le", "9be"]),
        (8u8, ["p08", "p8", "8le", "8be"]),
    ];

    for (depth, patterns) in depth_candidates.iter() {
        if patterns.iter().any(|pattern| fmt.contains(pattern)) {
            return Some(*depth);
        }
    }

    None
}

fn detect_dynamic_range(
    color_transfer: Option<&str>,
    color_primaries: Option<&str>,
) -> DynamicRange {
    match color_transfer {
        Some("smpte2084") => DynamicRange::Hdr10,
        Some("arib-std-b67") => DynamicRange::Hlg,
        Some(_) => DynamicRange::Sdr,
        None => {
            if matches!(color_primaries, Some("bt2020")) {
                DynamicRange::Unknown
            } else {
                DynamicRange::Sdr
            }
        }
    }
}

fn select_video_stream(streams: &[Stream]) -> Option<&Stream> {
    let mut best: Option<&Stream> = None;
    let mut best_pixels = 0u64;
    let mut best_is_default = false;

    for stream in streams.iter().filter(|s| s.codec_type == "video") {
        let is_default = stream
            .disposition
            .as_ref()
            .and_then(|d| d.default)
            .unwrap_or(0)
            == 1;
        let width = stream.width.or(stream.coded_width).unwrap_or(0) as u64;
        let height = stream.height.or(stream.coded_height).unwrap_or(0) as u64;
        let pixels = width.saturating_mul(height);

        if best.is_none()
            || (is_default && !best_is_default)
            || (is_default == best_is_default && pixels > best_pixels)
        {
            best = Some(stream);
            best_pixels = pixels;
            best_is_default = is_default;
        }
    }

    best
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
            pix_fmt: None,
            width: None,
            height: None,
            coded_width: None,
            coded_height: None,
            bit_rate: None,
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            avg_frame_rate: None,
            r_frame_rate: None,
            nb_frames: None,
            duration: None,
            disposition: None,
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
        };
        assert!(Analyzer::should_transcode_audio(&heavy));

        let standard = Stream {
            codec_name: "ac3".into(),
            codec_type: "audio".into(),
            pix_fmt: None,
            width: None,
            height: None,
            coded_width: None,
            coded_height: None,
            bit_rate: Some("384000".into()),
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            avg_frame_rate: None,
            r_frame_rate: None,
            nb_frames: None,
            duration: None,
            disposition: None,
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
        };
        assert!(!Analyzer::should_transcode_audio(&standard));

        let high_bitrate_ac3 = Stream {
            codec_name: "ac3".into(),
            codec_type: "audio".into(),
            pix_fmt: None,
            width: None,
            height: None,
            coded_width: None,
            coded_height: None,
            bit_rate: Some("1000000".into()),
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            avg_frame_rate: None,
            r_frame_rate: None,
            nb_frames: None,
            duration: None,
            disposition: None,
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
        };
        assert!(Analyzer::should_transcode_audio(&high_bitrate_ac3));
    }
}
