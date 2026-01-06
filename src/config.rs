use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub transcode: TranscodeConfig,
    pub hardware: HardwareConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscodeConfig {
    pub size_reduction_threshold: f64, // e.g., 0.3 for 30%
    pub min_bpp_threshold: f64,        // e.g., 0.1
    pub min_file_size_mb: u64,         // e.g., 50
    pub concurrent_jobs: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HardwareConfig {
    pub preferred_vendor: Option<String>,
    pub device_path: Option<String>,
    pub allow_cpu_fallback: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            transcode: TranscodeConfig {
                size_reduction_threshold: 0.3,
                min_bpp_threshold: 0.1,
                min_file_size_mb: 50,
                concurrent_jobs: 1,
            },
            hardware: HardwareConfig {
                preferred_vendor: None,
                device_path: None,
                allow_cpu_fallback: false,
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
        Ok(config)
    }
}
