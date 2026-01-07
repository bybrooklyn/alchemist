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
    #[serde(default = "default_quality_profile")]
    pub quality_profile: String, // "quality", "balanced", "speed"
}

fn default_quality_profile() -> String {
    "balanced".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HardwareConfig {
    pub preferred_vendor: Option<String>,
    pub device_path: Option<String>,
    pub allow_cpu_fallback: bool,
    #[serde(default = "default_cpu_preset")]
    pub cpu_preset: String, // "slow", "medium", "fast", "faster" for libsvtav1
    #[serde(default = "default_allow_cpu_encoding")]
    pub allow_cpu_encoding: bool,
}

fn default_cpu_preset() -> String {
    "medium".to_string()
}

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

impl Default for Config {
    fn default() -> Self {
        Self {
            transcode: TranscodeConfig {
                size_reduction_threshold: 0.3,
                min_bpp_threshold: 0.1,
                min_file_size_mb: 50,
                concurrent_jobs: 1,
                quality_profile: "balanced".to_string(),
            },
            hardware: HardwareConfig {
                preferred_vendor: None,
                device_path: None,
                allow_cpu_fallback: true,
                cpu_preset: "medium".to_string(),
                allow_cpu_encoding: true,
            },
            scanner: ScannerConfig {
                directories: Vec::new(),
                watch_enabled: false,
            },
            notifications: NotificationsConfig::default(),
            quality: QualityConfig::default(),
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
        // Validate CPU preset
        let valid_presets = ["slow", "medium", "fast", "faster"];
        if !valid_presets.contains(&self.hardware.cpu_preset.as_str()) {
            anyhow::bail!(
                "Invalid cpu_preset '{}'. Valid values: {:?}",
                self.hardware.cpu_preset,
                valid_presets
            );
        }

        // Validate quality profile
        let valid_profiles = ["quality", "balanced", "speed"];
        if !valid_profiles.contains(&self.transcode.quality_profile.as_str()) {
            anyhow::bail!(
                "Invalid quality_profile '{}'. Valid values: {:?}",
                self.transcode.quality_profile,
                valid_profiles
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
