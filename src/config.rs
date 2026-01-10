use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub transcode: TranscodeConfig,
    pub hardware: HardwareConfig,
    pub scanner: ScannerConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub quality: QualityConfig,
    #[serde(default)]
    pub system: SystemConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum QualityProfile {
    Quality,
    #[default]
    Balanced,
    Speed,
}

impl QualityProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Quality => "quality",
            Self::Balanced => "balanced",
            Self::Speed => "speed",
        }
    }
}

impl std::fmt::Display for QualityProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
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

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum CpuPreset {
    Slow,
    #[default]
    Medium,
    Fast,
    Faster,
}

impl CpuPreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Slow => "slow",
            Self::Medium => "medium",
            Self::Fast => "fast",
            Self::Faster => "faster",
        }
    }

    pub fn params(&self) -> (&'static str, &'static str) {
        match self {
            Self::Slow => ("4", "28"),
            Self::Medium => ("8", "32"),
            Self::Fast => ("12", "35"),
            Self::Faster => ("13", "38"),
        }
    }
}

impl std::fmt::Display for CpuPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Output codec selection
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputCodec {
    #[default]
    Av1,
    Hevc,
}

impl OutputCodec {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Av1 => "av1",
            Self::Hevc => "hevc",
        }
    }
}

/// Subtitle handling mode
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleMode {
    #[default]
    Copy,
    Burn,
    Extract,
    None,
}

impl SubtitleMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Burn => "burn",
            Self::Extract => "extract",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScannerConfig {
    pub directories: Vec<String>,
    #[serde(default)]
    pub watch_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscodeConfig {
    pub size_reduction_threshold: f64, // e.g., 0.3 for 30%
    pub min_bpp_threshold: f64,        // e.g., 0.1
    pub min_file_size_mb: u64,         // e.g., 50
    pub concurrent_jobs: usize,
    #[serde(default)]
    pub threads: usize, // 0 = auto
    #[serde(default)]
    pub quality_profile: QualityProfile,
    #[serde(default)]
    pub output_codec: OutputCodec,
    #[serde(default)]
    pub subtitle_mode: SubtitleMode,
}

// Removed default_quality_profile helper as Default trait on enum handles it now.

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HardwareConfig {
    pub preferred_vendor: Option<String>,
    pub device_path: Option<String>,
    pub allow_cpu_fallback: bool,
    #[serde(default)]
    pub cpu_preset: CpuPreset,
    #[serde(default = "default_allow_cpu_encoding")]
    pub allow_cpu_encoding: bool,
}

// Removed default_cpu_preset helper as Default trait on enum handles it now.

fn default_allow_cpu_encoding() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NotificationsConfig {
    pub enabled: bool,
    pub webhook_url: Option<String>,
    pub discord_webhook: Option<String>,
    pub notify_on_complete: bool,
    pub notify_on_failure: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QualityConfig {
    pub enable_vmaf: bool,
    pub min_vmaf_score: f64,
    pub revert_on_low_quality: bool,
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            enable_vmaf: false,
            min_vmaf_score: 90.0,
            revert_on_low_quality: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemConfig {
    #[serde(default = "default_poll_interval")]
    pub monitoring_poll_interval: f64,
    #[serde(default = "default_true")]
    pub enable_telemetry: bool,
}

fn default_true() -> bool {
    true
}

fn default_poll_interval() -> f64 {
    2.0
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            monitoring_poll_interval: default_poll_interval(),
            enable_telemetry: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            transcode: TranscodeConfig {
                size_reduction_threshold: 0.3,
                min_bpp_threshold: 0.1,
                min_file_size_mb: 50,
                concurrent_jobs: 1,
                threads: 0,
                quality_profile: QualityProfile::Balanced,
                output_codec: OutputCodec::Av1,
                subtitle_mode: SubtitleMode::Copy,
            },
            hardware: HardwareConfig {
                preferred_vendor: None,
                device_path: None,
                allow_cpu_fallback: true,
                cpu_preset: CpuPreset::Medium,
                allow_cpu_encoding: true,
            },
            scanner: ScannerConfig {
                directories: Vec::new(),
                watch_enabled: false,
            },
            notifications: NotificationsConfig::default(),
            quality: QualityConfig::default(),
            system: SystemConfig {
                monitoring_poll_interval: default_poll_interval(),
                enable_telemetry: true,
            },
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Enums automatically handle valid values via Serde,
        // so we don't need manual string checks for presets/profiles anymore.

        // Validate system monitoring poll interval
        if self.system.monitoring_poll_interval < 0.5 || self.system.monitoring_poll_interval > 10.0
        {
            anyhow::bail!(
                "monitoring_poll_interval must be between 0.5 and 10.0 seconds, got {}",
                self.system.monitoring_poll_interval
            );
        }

        // Validate thresholds
        if self.transcode.size_reduction_threshold < 0.0
            || self.transcode.size_reduction_threshold > 1.0
        {
            anyhow::bail!(
                "size_reduction_threshold must be between 0.0 and 1.0, got {}",
                self.transcode.size_reduction_threshold
            );
        }

        if self.transcode.min_bpp_threshold < 0.0 {
            anyhow::bail!(
                "min_bpp_threshold must be >= 0.0, got {}",
                self.transcode.min_bpp_threshold
            );
        }

        if self.transcode.concurrent_jobs == 0 {
            anyhow::bail!("concurrent_jobs must be >= 1");
        }

        // Validate VMAF threshold
        if self.quality.min_vmaf_score < 0.0 || self.quality.min_vmaf_score > 100.0 {
            anyhow::bail!(
                "min_vmaf_score must be between 0.0 and 100.0, got {}",
                self.quality.min_vmaf_score
            );
        }

        Ok(())
    }

    /// Save config to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
