use std::fmt;
use std::str::FromStr;

use crate::chunking::ChunkingMode;
use crate::config::{BackendPolicy, WhyThoConfig};
use crate::error::WhyThoError;
use crate::file_ops::FileOperationMode;
use crate::media::{AudioCodec, ContainerFormat, VideoCodec};
use crate::verification::VerificationMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Preset {
    Av1Balanced,
    Av1StorageSaver,
    HevcCompatible,
    H264Safe,
    RemuxClean,
    Benchmark,
    StrictVerify,
}

impl Preset {
    pub const ALL: [Self; 7] = [
        Self::Av1Balanced,
        Self::Av1StorageSaver,
        Self::HevcCompatible,
        Self::H264Safe,
        Self::RemuxClean,
        Self::Benchmark,
        Self::StrictVerify,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Av1Balanced => "av1-balanced",
            Self::Av1StorageSaver => "av1-storage-saver",
            Self::HevcCompatible => "hevc-compatible",
            Self::H264Safe => "h264-safe",
            Self::RemuxClean => "remux-clean",
            Self::Benchmark => "benchmark",
            Self::StrictVerify => "strict-verify",
        }
    }

    pub fn resolve(self) -> WhyThoConfig {
        match self {
            Self::Av1Balanced => WhyThoConfig {
                preset: self,
                container: ContainerFormat::Mkv,
                default_video_codec: VideoCodec::Av1,
                default_audio_codec: AudioCodec::Opus,
                chunking: ChunkingMode::KeyframeAware,
                verification: VerificationMode::Sample,
                ..WhyThoConfig::default()
            },
            Self::Av1StorageSaver => WhyThoConfig {
                preset: self,
                container: ContainerFormat::Mkv,
                default_video_codec: VideoCodec::Av1,
                default_audio_codec: AudioCodec::Opus,
                chunking: ChunkingMode::KeyframeAware,
                verification: VerificationMode::Sample,
                backend_policy: BackendPolicy::CpuOnly,
                ..WhyThoConfig::default()
            },
            Self::HevcCompatible => WhyThoConfig {
                preset: self,
                container: ContainerFormat::Mp4,
                default_video_codec: VideoCodec::Hevc,
                default_audio_codec: AudioCodec::Aac,
                chunking: ChunkingMode::KeyframeAware,
                verification: VerificationMode::Sample,
                ..WhyThoConfig::default()
            },
            Self::H264Safe => WhyThoConfig {
                preset: self,
                container: ContainerFormat::Mp4,
                default_video_codec: VideoCodec::H264,
                default_audio_codec: AudioCodec::Aac,
                chunking: ChunkingMode::Enabled,
                verification: VerificationMode::Sample,
                ..WhyThoConfig::default()
            },
            Self::RemuxClean => WhyThoConfig {
                preset: self,
                container: ContainerFormat::Mkv,
                default_video_codec: VideoCodec::H264,
                default_audio_codec: AudioCodec::Passthrough,
                chunking: ChunkingMode::Disabled,
                verification: VerificationMode::Sample,
                file_operation: FileOperationMode::ReplaceOriginal,
                ..WhyThoConfig::default()
            },
            Self::Benchmark => WhyThoConfig {
                preset: self,
                container: ContainerFormat::Mkv,
                default_video_codec: VideoCodec::Av1,
                default_audio_codec: AudioCodec::Opus,
                chunking: ChunkingMode::KeyframeAware,
                verification: VerificationMode::Benchmark,
                ..WhyThoConfig::default()
            },
            Self::StrictVerify => WhyThoConfig {
                preset: self,
                container: ContainerFormat::Mkv,
                default_video_codec: VideoCodec::Av1,
                default_audio_codec: AudioCodec::Opus,
                chunking: ChunkingMode::KeyframeAware,
                verification: VerificationMode::Strict,
                ..WhyThoConfig::default()
            },
        }
    }
}

impl Default for Preset {
    fn default() -> Self {
        Self::Av1Balanced
    }
}

impl fmt::Display for Preset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Preset {
    type Err = WhyThoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "av1-balanced" => Ok(Self::Av1Balanced),
            "av1-storage-saver" => Ok(Self::Av1StorageSaver),
            "hevc-compatible" => Ok(Self::HevcCompatible),
            "h264-safe" => Ok(Self::H264Safe),
            "remux-clean" => Ok(Self::RemuxClean),
            "benchmark" => Ok(Self::Benchmark),
            "strict-verify" => Ok(Self::StrictVerify),
            _ => Err(WhyThoError::InvalidPreset { name: s.into() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_resolve() {
        for preset in Preset::ALL {
            let config = preset.resolve();
            assert_eq!(config.preset, preset);
        }
    }

    #[test]
    fn av1_balanced_defaults() {
        let config = Preset::Av1Balanced.resolve();
        assert_eq!(config.default_video_codec, VideoCodec::Av1);
        assert_eq!(config.default_audio_codec, AudioCodec::Opus);
        assert_eq!(config.container, ContainerFormat::Mkv);
        assert_eq!(config.verification, VerificationMode::Sample);
    }

    #[test]
    fn h264_safe_defaults() {
        let config = Preset::H264Safe.resolve();
        assert_eq!(config.default_video_codec, VideoCodec::H264);
        assert_eq!(config.container, ContainerFormat::Mp4);
    }

    #[test]
    fn from_str_roundtrip() {
        for preset in Preset::ALL {
            let s = preset.as_str();
            let parsed: Preset = s.parse().unwrap();
            assert_eq!(parsed, preset);
        }
    }
}
