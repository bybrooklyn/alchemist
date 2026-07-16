use std::fmt;
use std::path::Path;
use std::str::FromStr;

use crate::chunking::ChunkingMode;
use crate::error::WhyThoError;
use crate::file_ops::FileOperationMode;
use crate::media::{AudioCodec, ContainerFormat, VideoCodec};
use crate::presets::Preset;
use crate::verification::VerificationMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    Cpu,
    Qsv,
    Nvenc,
    VideoToolbox,
    VaApi,
    Amf,
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu => write!(f, "cpu"),
            Self::Qsv => write!(f, "qsv"),
            Self::Nvenc => write!(f, "nvenc"),
            Self::VideoToolbox => write!(f, "videotoolbox"),
            Self::VaApi => write!(f, "va-api"),
            Self::Amf => write!(f, "amf"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendPolicy {
    PreferHardware,
    RequireHardware,
    CpuOnly,
}

impl Default for BackendPolicy {
    fn default() -> Self {
        Self::PreferHardware
    }
}

impl fmt::Display for BackendPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreferHardware => write!(f, "prefer-hardware"),
            Self::RequireHardware => write!(f, "require-hardware"),
            Self::CpuOnly => write!(f, "cpu-only"),
        }
    }
}

impl FromStr for BackendPolicy {
    type Err = WhyThoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "prefer-hardware" => Ok(Self::PreferHardware),
            "require-hardware" => Ok(Self::RequireHardware),
            "cpu-only" => Ok(Self::CpuOnly),
            _ => Err(WhyThoError::InvalidValue {
                field: "backend-policy".into(),
                value: s.into(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerCount {
    Auto,
    Fixed(usize),
}

impl Default for WorkerCount {
    fn default() -> Self {
        Self::Auto
    }
}

impl fmt::Display for WorkerCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Fixed(n) => write!(f, "{n}"),
        }
    }
}

impl FromStr for WorkerCount {
    type Err = WhyThoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "auto" {
            return Ok(Self::Auto);
        }
        match s.parse::<usize>() {
            Ok(n) => Ok(Self::Fixed(n)),
            Err(_) => Err(WhyThoError::InvalidValue {
                field: "cpu-workers".into(),
                value: s.into(),
            }),
        }
    }
}

impl<'de> serde::Deserialize<'de> for WorkerCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = WorkerCount;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(r#""auto" or an integer"#)
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<WorkerCount, E> {
                if v == "auto" {
                    Ok(WorkerCount::Auto)
                } else {
                    Err(E::custom(format!("invalid worker count: {v}")))
                }
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<WorkerCount, E> {
                Ok(WorkerCount::Fixed(v as usize))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<WorkerCount, E> {
                if v >= 0 {
                    Ok(WorkerCount::Fixed(v as usize))
                } else {
                    Err(E::custom(format!("negative worker count: {v}")))
                }
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhyThoConfig {
    pub preset: Preset,
    pub container: ContainerFormat,
    pub backend_policy: BackendPolicy,
    pub chunking: ChunkingMode,
    pub max_jobs: usize,
    pub cpu_workers: WorkerCount,
    pub default_video_codec: VideoCodec,
    pub default_audio_codec: AudioCodec,
    pub verification: VerificationMode,
    pub file_operation: FileOperationMode,
}

impl Default for WhyThoConfig {
    fn default() -> Self {
        Self {
            preset: Preset::default(),
            container: ContainerFormat::default(),
            backend_policy: BackendPolicy::default(),
            chunking: ChunkingMode::default(),
            max_jobs: 3,
            cpu_workers: WorkerCount::default(),
            default_video_codec: VideoCodec::Av1,
            default_audio_codec: AudioCodec::Opus,
            verification: VerificationMode::default(),
            file_operation: FileOperationMode::default(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct TomlConfig {
    pub defaults: TomlDefaults,
    pub concurrency: TomlConcurrency,
    pub video: TomlVideo,
    pub audio: TomlAudio,
    pub verification: TomlVerification,
    pub file_ops: TomlFileOps,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct TomlDefaults {
    pub preset: Option<String>,
    pub container: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct TomlConcurrency {
    pub max_jobs: Option<usize>,
    pub chunking: Option<bool>,
    pub cpu_workers: Option<WorkerCount>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct TomlVideo {
    pub default_codec: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct TomlAudio {
    pub default_codec: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct TomlVerification {
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct TomlFileOps {
    pub replace_original: Option<bool>,
    pub purge_partial_on_cancel: Option<bool>,
}

impl TomlConfig {
    pub fn from_file(path: &Path) -> Result<Self, WhyThoError> {
        let content = std::fs::read_to_string(path)?;
        let config: TomlConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

impl WhyThoConfig {
    pub fn merge_toml(&mut self, toml: &TomlConfig) {
        if let Some(ref p) = toml.defaults.preset {
            if let Ok(preset) = Preset::from_str(p) {
                self.preset = preset;
            }
        }
        if let Some(ref c) = toml.defaults.container {
            if let Ok(c) = ContainerFormat::from_str(c) {
                self.container = c;
            }
        }
        if let Some(max_jobs) = toml.concurrency.max_jobs {
            self.max_jobs = max_jobs;
        }
        if let Some(chunking) = toml.concurrency.chunking {
            self.chunking = if chunking {
                ChunkingMode::Enabled
            } else {
                ChunkingMode::Disabled
            };
        }
        if let Some(w) = toml.concurrency.cpu_workers {
            self.cpu_workers = w;
        }
        if let Some(ref v) = toml.video.default_codec {
            if let Ok(v) = VideoCodec::from_str(v) {
                self.default_video_codec = v;
            }
        }
        if let Some(ref a) = toml.audio.default_codec {
            if let Ok(a) = AudioCodec::from_str(a) {
                self.default_audio_codec = a;
            }
        }
        if let Some(ref m) = toml.verification.mode {
            if let Ok(m) = VerificationMode::from_str(m) {
                self.verification = m;
            }
        }
        if let Some(replace) = toml.file_ops.replace_original {
            self.file_operation = if replace {
                FileOperationMode::ReplaceOriginal
            } else {
                FileOperationMode::PreserveOriginal
            };
        }
    }

    pub fn default_video_codec_bitrate(&self) -> u32 {
        match self.default_video_codec {
            VideoCodec::Av1 => 2_000_000,
            VideoCodec::Hevc => 3_000_000,
            VideoCodec::H264 => 4_000_000,
            VideoCodec::Av2 => 1_500_000,
        }
    }

    pub fn load(
        config_path: Option<&Path>,
        preset_override: Option<Preset>,
    ) -> Result<Self, WhyThoError> {
        let toml = match config_path {
            Some(p) => Some(TomlConfig::from_file(p)?),
            None => Self::try_default_config_file()?,
        };

        let preset = preset_override
            .or_else(|| {
                toml.as_ref()
                    .and_then(|t| t.defaults.preset.as_deref()?.parse().ok())
            })
            .unwrap_or_default();

        let mut config = preset.resolve();

        if let Some(ref toml) = toml {
            config.merge_toml(toml);
        }

        Ok(config)
    }

    fn try_default_config_file() -> Result<Option<TomlConfig>, WhyThoError> {
        if let Ok(cwd) = std::env::current_dir() {
            let local = cwd.join("whytho.toml");
            if local.exists() {
                return Ok(Some(TomlConfig::from_file(&local)?));
            }
        }

        if let Some(home) = std::env::var_os("HOME") {
            let xdg = Path::new(&home)
                .join(".config")
                .join("whytho")
                .join("config.toml");
            if xdg.exists() {
                return Ok(Some(TomlConfig::from_file(&xdg)?));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_toml() {
        let toml_str = r#"
[defaults]
preset = "av1-balanced"
container = "mkv"

[concurrency]
max_jobs = 4
chunking = true

[video]
default_codec = "av1"

[audio]
default_codec = "opus"
"#;
        let config: TomlConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.defaults.preset.as_deref(), Some("av1-balanced"));
        assert_eq!(config.concurrency.max_jobs, Some(4));
        assert_eq!(config.concurrency.chunking, Some(true));
    }

    #[test]
    fn merge_toml_overrides_defaults() {
        let mut config = WhyThoConfig::default();
        let toml = TomlConfig {
            defaults: TomlDefaults {
                preset: None,
                container: Some("mp4".into()),
            },
            concurrency: TomlConcurrency {
                max_jobs: Some(8),
                chunking: None,
                cpu_workers: None,
            },
            ..Default::default()
        };
        config.merge_toml(&toml);
        assert_eq!(config.container, ContainerFormat::Mp4);
        assert_eq!(config.max_jobs, 8);
    }

    #[test]
    fn worker_count_serde_roundtrip() {
        #[derive(serde::Deserialize)]
        struct W {
            workers: WorkerCount,
        }

        let auto: W = toml::from_str("workers = \"auto\"").unwrap();
        assert_eq!(auto.workers, WorkerCount::Auto);

        let fixed: W = toml::from_str("workers = 4").unwrap();
        assert_eq!(fixed.workers, WorkerCount::Fixed(4));
    }

    #[test]
    fn load_with_no_config_file_returns_defaults() {
        let config = WhyThoConfig::load(None, None).unwrap();
        assert_eq!(config.preset, Preset::Av1Balanced);
        assert_eq!(config.max_jobs, 3);
    }

    #[test]
    fn load_with_preset_override() {
        let config = WhyThoConfig::load(None, Some(Preset::H264Safe)).unwrap();
        assert_eq!(config.preset, Preset::H264Safe);
        assert_eq!(config.default_video_codec, VideoCodec::H264);
    }
}
