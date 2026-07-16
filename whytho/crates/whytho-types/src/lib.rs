#![forbid(unsafe_code)]

//! The codec contract for `whytho.`.
//!
//! This crate is the tiny, dependency-light seam that every codec crate implements and that
//! the facade, core, and consumers depend on: the media-type enums, the raw frame/packet
//! types, and the encoder/decoder traits. It deliberately holds no app policy (planning,
//! scheduling, reporting) so codecs never have to depend on the orchestrator.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Media-type enums (moved from `whytho-core::media`)
// ---------------------------------------------------------------------------

/// Error returned when parsing a codec or container enum from a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseEnumError {
    pub field: &'static str,
    pub value: String,
}

impl fmt::Display for ParseEnumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {}: {}", self.field, self.value)
    }
}

impl std::error::Error for ParseEnumError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerFormat {
    Mkv,
    Mp4,
    WebM,
}

impl Default for ContainerFormat {
    fn default() -> Self {
        Self::Mkv
    }
}

impl fmt::Display for ContainerFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mkv => write!(f, "mkv"),
            Self::Mp4 => write!(f, "mp4"),
            Self::WebM => write!(f, "webm"),
        }
    }
}

impl FromStr for ContainerFormat {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mkv" => Ok(Self::Mkv),
            "mp4" => Ok(Self::Mp4),
            "webm" => Ok(Self::WebM),
            _ => Err(ParseEnumError {
                field: "container",
                value: s.into(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
    Av2,
}

impl fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::H264 => write!(f, "h264"),
            Self::Hevc => write!(f, "hevc"),
            Self::Av1 => write!(f, "av1"),
            Self::Av2 => write!(f, "av2"),
        }
    }
}

impl FromStr for VideoCodec {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "h264" => Ok(Self::H264),
            "hevc" => Ok(Self::Hevc),
            "av1" => Ok(Self::Av1),
            "av2" => Ok(Self::Av2),
            _ => Err(ParseEnumError {
                field: "video-codec",
                value: s.into(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioCodec {
    Aac,
    Opus,
    Passthrough,
}

impl fmt::Display for AudioCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aac => write!(f, "aac"),
            Self::Opus => write!(f, "opus"),
            Self::Passthrough => write!(f, "passthrough"),
        }
    }
}

impl FromStr for AudioCodec {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "aac" => Ok(Self::Aac),
            "opus" => Ok(Self::Opus),
            "passthrough" => Ok(Self::Passthrough),
            _ => Err(ParseEnumError {
                field: "audio-codec",
                value: s.into(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Codec contract (moved from `whytho-codecs`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecDirection {
    Decode,
    Encode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoCodecCapability {
    pub name: String,
    pub codec: VideoCodec,
    pub direction: CodecDirection,
}

impl VideoCodecCapability {
    pub fn new(name: impl Into<String>, codec: VideoCodec, direction: CodecDirection) -> Self {
        Self {
            name: name.into(),
            codec,
            direction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioCodecCapability {
    pub name: String,
    pub codec: AudioCodec,
    pub direction: CodecDirection,
}

impl AudioCodecCapability {
    pub fn new(name: impl Into<String>, codec: AudioCodec, direction: CodecDirection) -> Self {
        Self {
            name: name.into(),
            codec,
            direction,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Yuv420,
    Yuv422,
    Yuv444,
}

#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
    pub pixel_format: PixelFormat,
    pub pts: Duration,
}

#[derive(Debug, Clone)]
pub struct EncodedPacket {
    pub data: Vec<u8>,
    pub pts: Duration,
    pub is_keyframe: bool,
}

#[derive(Debug, Clone)]
pub struct VideoEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub bitrate: u32,
    pub keyframe_interval: u64,
    pub speed_preset: u8,
}

impl Default for VideoEncoderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 24.0,
            bitrate: 2_000_000,
            keyframe_interval: 240,
            speed_preset: 6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioEncoderConfig {
    pub sample_rate: u32,
    pub channels: usize,
    pub bitrate: u32,
}

impl Default for AudioEncoderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bitrate: 128_000,
        }
    }
}

pub trait VideoEncoder {
    fn name(&self) -> &str;
    fn codec(&self) -> VideoCodec;
    fn configure(&mut self, config: &VideoEncoderConfig) -> Result<(), String>;
    fn encode(&mut self, frame: &DecodedFrame) -> Result<Vec<EncodedPacket>, String>;
    fn flush(&mut self) -> Result<Vec<EncodedPacket>, String>;
}

pub trait VideoDecoder {
    fn name(&self) -> &str;
    fn codec(&self) -> VideoCodec;
    fn decode_nal(&mut self, data: &[u8]) -> Result<Vec<DecodedFrame>, String>;
    fn flush(&mut self) -> Vec<DecodedFrame>;
}

pub trait AudioEncoder {
    fn name(&self) -> &str;
    fn codec(&self) -> AudioCodec;
    fn configure(&mut self, config: &AudioEncoderConfig) -> Result<(), String>;
    fn encode(&mut self, samples: &[f32]) -> Result<Vec<Vec<u8>>, String>;
    fn flush(&mut self) -> Result<Vec<Vec<u8>>, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_capability_records_codec_direction() {
        let capability =
            VideoCodecCapability::new("rav1e", VideoCodec::Av1, CodecDirection::Encode);

        assert_eq!(capability.name, "rav1e");
        assert_eq!(capability.codec, VideoCodec::Av1);
        assert_eq!(capability.direction, CodecDirection::Encode);
    }

    #[test]
    fn default_video_encoder_config() {
        let config = VideoEncoderConfig::default();
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert_eq!(config.bitrate, 2_000_000);
    }

    #[test]
    fn default_audio_encoder_config() {
        let config = AudioEncoderConfig::default();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
    }

    #[test]
    fn codec_enums_roundtrip_via_string() {
        assert_eq!("av2".parse::<VideoCodec>(), Ok(VideoCodec::Av2));
        assert_eq!("opus".parse::<AudioCodec>(), Ok(AudioCodec::Opus));
        assert_eq!("mkv".parse::<ContainerFormat>(), Ok(ContainerFormat::Mkv));
        assert!("nope".parse::<VideoCodec>().is_err());
    }
}
