pub mod h264;
pub mod matroska;

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use crate::error::WhyThoError;
use crate::media::ContainerFormat;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProbeResult {
    pub path: std::path::PathBuf,
    pub container: DetectedContainer,
    pub size_bytes: u64,
    pub streams: Vec<StreamInfo>,
    pub chapters: Vec<ChapterInfo>,
    pub attachments: Vec<AttachmentInfo>,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetectedContainer {
    pub format: ContainerFormat,
    pub matroska_version: Option<u32>,
    pub writing_app: Option<String>,
    pub muxing_app: Option<String>,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StreamInfo {
    pub index: u32,
    pub track_number: u64,
    pub kind: StreamKind,
    pub codec_id: String,
    pub codec: DetectedCodec,
    pub language: Option<String>,
    pub name: Option<String>,
    pub default: bool,
    pub forced: bool,
    pub enabled: bool,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum StreamKind {
    Video(VideoStreamInfo),
    Audio(AudioStreamInfo),
    Subtitle(SubtitleStreamInfo),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VideoStreamInfo {
    pub width: u32,
    pub height: u32,
    pub display_width: Option<u32>,
    pub display_height: Option<u32>,
    pub pixel_format: Option<String>,
    pub bit_depth: Option<u8>,
    pub frame_rate: Option<f64>,
    pub bitrate: Option<u64>,
    pub profile: Option<String>,
    pub level: Option<String>,
    pub color_space: Option<String>,
    pub color_range: Option<String>,
    pub hdr: Option<HdrInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioStreamInfo {
    pub sample_rate: Option<f64>,
    pub channels: Option<u64>,
    pub channel_layout: Option<String>,
    pub bit_depth: Option<u8>,
    pub bitrate: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubtitleStreamInfo {
    pub codec_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HdrInfo {
    pub format: Option<String>,
    pub color_primaries: Option<String>,
    pub transfer_characteristics: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChapterInfo {
    pub uid: u64,
    pub start: Duration,
    pub end: Duration,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AttachmentInfo {
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DetectedCodec {
    H264,
    Hevc,
    Av1,
    Av2,
    Vp8,
    Vp9,
    Aac,
    Opus,
    Mp3,
    Flac,
    Vorbis,
    Srt,
    Ass,
    Ssa,
    Pgs,
    UnknownVideo,
    UnknownAudio,
    UnknownSubtitle,
}

impl DetectedCodec {
    pub fn from_codec_id(codec_id: &str) -> Self {
        match codec_id {
            "V_MPEG4/ISO/AVC" => Self::H264,
            "V_MPEGH/ISO/HEVC" => Self::Hevc,
            "V_AV1" => Self::Av1,
            "V_VP8" => Self::Vp8,
            "V_VP9" => Self::Vp9,
            "A_AAC" | "A_AAC/MPEG4/LC" | "A_AAC/MPEG4/LC/SBR" | "A_AAC/MPEG4/MAIN"
            | "A_AAC/MPEG4/SSR" | "A_AAC/MPEG4/LTP" => Self::Aac,
            "A_OPUS" => Self::Opus,
            "A_MPEG/L3" => Self::Mp3,
            "A_FLAC" => Self::Flac,
            "A_VORBIS" => Self::Vorbis,
            "S_TEXT/UTF8" | "S_TEXT/ASCII" => Self::Srt,
            "S_TEXT/ASS" => Self::Ass,
            "S_TEXT/SSA" => Self::Ssa,
            "S_HDMV/PGS" => Self::Pgs,
            id if id.starts_with("V_") => Self::UnknownVideo,
            id if id.starts_with("A_") => Self::UnknownAudio,
            id if id.starts_with("S_") => Self::UnknownSubtitle,
            _ => Self::UnknownVideo,
        }
    }

    pub fn is_video(&self) -> bool {
        matches!(
            self,
            Self::H264
                | Self::Hevc
                | Self::Av1
                | Self::Av2
                | Self::Vp8
                | Self::Vp9
                | Self::UnknownVideo
        )
    }
}

impl fmt::Display for DetectedCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::H264 => write!(f, "H.264"),
            Self::Hevc => write!(f, "HEVC"),
            Self::Av1 => write!(f, "AV1"),
            Self::Av2 => write!(f, "AV2"),
            Self::Vp8 => write!(f, "VP8"),
            Self::Vp9 => write!(f, "VP9"),
            Self::Aac => write!(f, "AAC"),
            Self::Opus => write!(f, "Opus"),
            Self::Mp3 => write!(f, "MP3"),
            Self::Flac => write!(f, "FLAC"),
            Self::Vorbis => write!(f, "Vorbis"),
            Self::Srt => write!(f, "SRT"),
            Self::Ass => write!(f, "ASS"),
            Self::Ssa => write!(f, "SSA"),
            Self::Pgs => write!(f, "PGS"),
            Self::UnknownVideo => write!(f, "unknown video"),
            Self::UnknownAudio => write!(f, "unknown audio"),
            Self::UnknownSubtitle => write!(f, "unknown subtitle"),
        }
    }
}

pub fn probe(path: &Path) -> Result<ProbeResult, WhyThoError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "mkv" | "webm" | "mka" | "mks" => matroska::probe_mkv(path),
        "mp4" | "m4v" | "mov" => Err(WhyThoError::UnsupportedContainer {
            path: path.to_path_buf(),
            detected: "MP4/MOV (not yet supported)".into(),
        }),
        _ => Err(WhyThoError::UnsupportedContainer {
            path: path.to_path_buf(),
            detected: ext,
        }),
    }
}

impl fmt::Display for ProbeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size = human_size(self.size_bytes);
        let _dur = self
            .container
            .duration
            .map(human_duration)
            .unwrap_or_else(|| "unknown duration".into());

        writeln!(
            f,
            "Container: {} ({}){}, {}",
            self.container.format,
            format!("{:?}", self.container.format).to_lowercase(),
            self.container
                .writing_app
                .as_deref()
                .map(|w| format!(", {w}"))
                .unwrap_or_default(),
            size
        )?;
        if let Some(d) = self.container.duration {
            writeln!(f, "  Duration: {}", human_duration(d))?;
        }

        for stream in &self.streams {
            write!(f, "  Stream {}: {}", stream.index, stream.codec)?;
            match &stream.kind {
                StreamKind::Video(v) => {
                    write!(f, ", {}x{}", v.width, v.height)?;
                    if let Some(fps) = v.frame_rate {
                        write!(f, ", {fps:.3} fps")?;
                    }
                    if let Some(ref profile) = v.profile {
                        if let Some(ref level) = v.level {
                            write!(f, " {profile}@{level}")?;
                        } else {
                            write!(f, " {profile}")?;
                        }
                    }
                    if let Some(ref pix) = v.pixel_format {
                        write!(f, ", {pix}")?;
                    }
                    if let Some(depth) = v.bit_depth {
                        write!(f, ", {depth}-bit")?;
                    }
                }
                StreamKind::Audio(a) => {
                    if let Some(sr) = a.sample_rate {
                        write!(f, ", {} Hz", sr as u64)?;
                    }
                    if let Some(ch) = a.channels {
                        write!(f, ", {} channel{}", ch, if ch == 1 { "" } else { "s" })?;
                    }
                    if let Some(layout) = &a.channel_layout {
                        write!(f, " ({layout})")?;
                    }
                    if let Some(br) = a.bitrate {
                        write!(f, ", {} kbps", br / 1000)?;
                    }
                }
                StreamKind::Subtitle(_) => {}
            }
            if let Some(ref lang) = stream.language {
                write!(f, " [{lang}]")?;
            }
            if stream.default {
                write!(f, " [default]")?;
            }
            writeln!(f)?;
        }

        if !self.chapters.is_empty() {
            writeln!(f, "  Chapters: {}", self.chapters.len())?;
        }
        if !self.attachments.is_empty() {
            writeln!(f, "  Attachments: {}", self.attachments.len())?;
        }

        Ok(())
    }
}

fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn human_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    let millis = d.subsec_millis();

    if hours > 0 {
        format!("{hours}h {minutes:02}m {seconds:02}.{millis:03}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}.{millis:03}s")
    } else {
        format!("{seconds}.{millis:03}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detected_codec_from_codec_id() {
        assert_eq!(
            DetectedCodec::from_codec_id("V_MPEG4/ISO/AVC"),
            DetectedCodec::H264
        );
        assert_eq!(
            DetectedCodec::from_codec_id("V_MPEGH/ISO/HEVC"),
            DetectedCodec::Hevc
        );
        assert_eq!(DetectedCodec::from_codec_id("V_AV1"), DetectedCodec::Av1);
        assert_eq!(DetectedCodec::from_codec_id("A_OPUS"), DetectedCodec::Opus);
        assert_eq!(DetectedCodec::from_codec_id("A_AAC"), DetectedCodec::Aac);
        assert_eq!(
            DetectedCodec::from_codec_id("S_TEXT/UTF8"),
            DetectedCodec::Srt
        );
        assert_eq!(
            DetectedCodec::from_codec_id("V_MPEG4/ISO/SPARK"),
            DetectedCodec::UnknownVideo
        );
    }

    #[test]
    fn detected_codec_display() {
        assert_eq!(DetectedCodec::H264.to_string(), "H.264");
        assert_eq!(DetectedCodec::Opus.to_string(), "Opus");
    }

    #[test]
    fn human_size_formatting() {
        assert_eq!(human_size(500), "500 B");
        assert_eq!(human_size(1536), "1.5 KiB");
        assert_eq!(human_size(1048576), "1.0 MiB");
        assert_eq!(human_size(2147483648), "2.0 GiB");
    }

    #[test]
    fn human_duration_formatting() {
        assert_eq!(human_duration(Duration::from_millis(500)), "0.500s");
        assert_eq!(human_duration(Duration::from_secs(65)), "1m 05.000s");
        assert_eq!(human_duration(Duration::from_secs(3723)), "1h 02m 03.000s");
    }
}
