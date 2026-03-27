use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub appearance: AppearanceConfig,
    pub transcode: TranscodeConfig,
    pub hardware: HardwareConfig,
    pub scanner: ScannerConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub files: FileSettingsConfig,
    #[serde(default)]
    pub schedule: ScheduleConfig,
    #[serde(default)]
    pub quality: QualityConfig,
    #[serde(default)]
    pub system: SystemConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppearanceConfig {
    #[serde(default)]
    pub active_theme_id: Option<String>,
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

    /// Get FFmpeg quality value for Apple VideoToolbox
    pub fn videotoolbox_quality(&self) -> &'static str {
        match self {
            Self::Quality => "55",
            Self::Balanced => "65",
            Self::Speed => "75",
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
    H264,
}

impl OutputCodec {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Av1 => "av1",
            Self::Hevc => "hevc",
            Self::H264 => "h264",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AudioMode {
    #[default]
    Copy,
    Aac,
    AacStereo,
}

impl AudioMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Aac => "aac",
            Self::AacStereo => "aac_stereo",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum HdrMode {
    #[default]
    Preserve,
    Tonemap,
}

impl HdrMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Preserve => "preserve",
            Self::Tonemap => "tonemap",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TonemapAlgorithm {
    #[default]
    Hable,
    Mobius,
    Reinhard,
    Clip,
}

impl TonemapAlgorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Hable => "hable",
            Self::Mobius => "mobius",
            Self::Reinhard => "reinhard",
            Self::Clip => "clip",
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

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum EngineMode {
    Background,
    #[default]
    Balanced,
    Throughput,
}

impl EngineMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Balanced => "balanced",
            Self::Throughput => "throughput",
        }
    }

    /// Compute the appropriate concurrent job count for this
    /// mode given the number of logical CPU cores available.
    /// Returns 0 to signal "use the stored manual override".
    #[allow(clippy::manual_clamp)]
    pub fn concurrent_jobs_for_cpu_count(&self, cpu_count: usize) -> usize {
        match self {
            // Background: always 1 job, minimal impact
            Self::Background => 1,
            // Balanced: half the cores, minimum 1, maximum 4
            Self::Balanced => (cpu_count / 2).max(1).min(4),
            // Throughput: half the cores uncapped, minimum 1
            Self::Throughput => (cpu_count / 2).max(1),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScannerConfig {
    pub directories: Vec<String>,
    #[serde(default)]
    pub watch_enabled: bool,
    #[serde(default)]
    pub extra_watch_dirs: Vec<WatchDirConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct WatchDirConfig {
    pub path: String,
    #[serde(default = "default_true")]
    pub is_recursive: bool,
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
    #[serde(default = "default_allow_fallback")]
    pub allow_fallback: bool,
    #[serde(default)]
    pub hdr_mode: HdrMode,
    #[serde(default)]
    pub tonemap_algorithm: TonemapAlgorithm,
    #[serde(default = "default_tonemap_peak")]
    pub tonemap_peak: f32,
    #[serde(default = "default_tonemap_desat")]
    pub tonemap_desat: f32,
    #[serde(default)]
    pub subtitle_mode: SubtitleMode,
    #[serde(default)]
    pub stream_rules: StreamRules,
    #[serde(default)]
    pub vmaf_min_score: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StreamRules {
    /// Strip audio tracks whose title contains any of these
    /// strings (case-insensitive). Common use: ["commentary",
    /// "director"].
    #[serde(default)]
    pub strip_audio_by_title: Vec<String>,

    /// If non-empty, keep ONLY audio tracks whose language tag
    /// matches one of these ISO 639-2 codes (e.g. ["eng", "jpn"]).
    /// Tracks with no language tag are always kept.
    /// If empty, all languages are kept (default).
    #[serde(default)]
    pub keep_audio_languages: Vec<String>,

    /// If true, strip all audio tracks except the one marked
    /// default by the source file. Overridden by
    /// keep_audio_languages if both are set.
    #[serde(default)]
    pub keep_only_default_audio: bool,
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

pub(crate) fn default_allow_fallback() -> bool {
    true
}

pub(crate) fn default_tonemap_peak() -> f32 {
    100.0
}

pub(crate) fn default_tonemap_desat() -> f32 {
    0.2
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NotificationsConfig {
    pub enabled: bool,
    #[serde(default)]
    pub targets: Vec<NotificationTargetConfig>,
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub discord_webhook: Option<String>,
    #[serde(default)]
    pub notify_on_complete: bool,
    #[serde(default)]
    pub notify_on_failure: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct NotificationTargetConfig {
    pub name: String,
    pub target_type: String,
    pub endpoint_url: String,
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileSettingsConfig {
    pub delete_source: bool,
    pub output_extension: String,
    pub output_suffix: String,
    pub replace_strategy: String,
    #[serde(default)]
    pub output_root: Option<String>,
}

impl Default for FileSettingsConfig {
    fn default() -> Self {
        Self {
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "-alchemist".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ScheduleConfig {
    #[serde(default)]
    pub windows: Vec<ScheduleWindowConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ScheduleWindowConfig {
    pub start_time: String,
    pub end_time: String,
    #[serde(default)]
    pub days_of_week: Vec<i32>,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    #[serde(default = "default_telemetry")]
    pub enable_telemetry: bool,
    #[serde(default = "default_log_retention_days")]
    pub log_retention_days: Option<u32>,
    #[serde(default)]
    pub engine_mode: EngineMode,
    /// Enable HSTS header (only enable if running behind HTTPS)
    #[serde(default)]
    pub https_only: bool,
}

fn default_true() -> bool {
    true
}

fn default_telemetry() -> bool {
    false
}

fn default_poll_interval() -> f64 {
    2.0
}

fn default_log_retention_days() -> Option<u32> {
    Some(30)
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            monitoring_poll_interval: default_poll_interval(),
            enable_telemetry: default_telemetry(),
            log_retention_days: default_log_retention_days(),
            engine_mode: EngineMode::default(),
            https_only: false,
        }
    }
}

#[derive(Debug, Serialize, Clone, Copy)]
pub struct BuiltInLibraryProfile {
    pub id: i64,
    pub name: &'static str,
    pub preset: &'static str,
    pub codec: OutputCodec,
    pub quality_profile: QualityProfile,
    pub hdr_mode: HdrMode,
    pub audio_mode: AudioMode,
    pub crf_override: Option<i32>,
    pub notes: Option<&'static str>,
}

pub const PRESET_SPACE_SAVER: BuiltInLibraryProfile = BuiltInLibraryProfile {
    id: 1,
    name: "Space Saver",
    preset: "space_saver",
    codec: OutputCodec::Av1,
    quality_profile: QualityProfile::Speed,
    hdr_mode: HdrMode::Tonemap,
    audio_mode: AudioMode::Aac,
    crf_override: None,
    notes: Some("Optimized for aggressive size reduction."),
};

pub const PRESET_QUALITY_FIRST: BuiltInLibraryProfile = BuiltInLibraryProfile {
    id: 2,
    name: "Quality First",
    preset: "quality_first",
    codec: OutputCodec::Hevc,
    quality_profile: QualityProfile::Quality,
    hdr_mode: HdrMode::Preserve,
    audio_mode: AudioMode::Copy,
    crf_override: None,
    notes: Some("Prioritizes fidelity over maximum compression."),
};

pub const PRESET_BALANCED: BuiltInLibraryProfile = BuiltInLibraryProfile {
    id: 3,
    name: "Balanced",
    preset: "balanced",
    codec: OutputCodec::Av1,
    quality_profile: QualityProfile::Balanced,
    hdr_mode: HdrMode::Preserve,
    audio_mode: AudioMode::Copy,
    crf_override: None,
    notes: Some("Balanced compression and playback quality."),
};

pub const PRESET_STREAMING: BuiltInLibraryProfile = BuiltInLibraryProfile {
    id: 4,
    name: "Streaming",
    preset: "streaming",
    codec: OutputCodec::H264,
    quality_profile: QualityProfile::Balanced,
    hdr_mode: HdrMode::Tonemap,
    audio_mode: AudioMode::AacStereo,
    crf_override: None,
    notes: Some("Maximizes compatibility for streaming clients."),
};

pub const BUILT_IN_LIBRARY_PROFILES: [BuiltInLibraryProfile; 4] = [
    PRESET_SPACE_SAVER,
    PRESET_QUALITY_FIRST,
    PRESET_BALANCED,
    PRESET_STREAMING,
];

impl Default for Config {
    fn default() -> Self {
        Self {
            appearance: AppearanceConfig::default(),
            transcode: TranscodeConfig {
                size_reduction_threshold: 0.3,
                min_bpp_threshold: 0.1,
                min_file_size_mb: 50,
                concurrent_jobs: 1,
                threads: 0,
                quality_profile: QualityProfile::Balanced,
                output_codec: OutputCodec::Av1,
                allow_fallback: default_allow_fallback(),
                hdr_mode: HdrMode::Preserve,
                tonemap_algorithm: TonemapAlgorithm::Hable,
                tonemap_peak: default_tonemap_peak(),
                tonemap_desat: default_tonemap_desat(),
                subtitle_mode: SubtitleMode::Copy,
                stream_rules: StreamRules::default(),
                vmaf_min_score: None,
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
                extra_watch_dirs: Vec::new(),
            },
            notifications: NotificationsConfig::default(),
            files: FileSettingsConfig::default(),
            schedule: ScheduleConfig::default(),
            quality: QualityConfig::default(),
            system: SystemConfig {
                monitoring_poll_interval: default_poll_interval(),
                enable_telemetry: default_telemetry(),
                log_retention_days: default_log_retention_days(),
                engine_mode: EngineMode::default(),
                https_only: false,
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
        let mut config: Config = toml::from_str(&content)?;
        config.migrate_legacy_notifications();
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

        if self.transcode.tonemap_peak < 50.0 || self.transcode.tonemap_peak > 1000.0 {
            anyhow::bail!(
                "tonemap_peak must be between 50 and 1000, got {}",
                self.transcode.tonemap_peak
            );
        }

        if !(0.0..=1.0).contains(&self.transcode.tonemap_desat) {
            anyhow::bail!(
                "tonemap_desat must be between 0.0 and 1.0, got {}",
                self.transcode.tonemap_desat
            );
        }

        if self
            .files
            .output_extension
            .chars()
            .any(|c| c == '/' || c == '\\')
        {
            anyhow::bail!("files.output_extension must not contain path separators");
        }

        if self
            .files
            .output_suffix
            .chars()
            .any(|c| c == '/' || c == '\\')
        {
            anyhow::bail!("files.output_suffix must not contain path separators");
        }

        for window in &self.schedule.windows {
            validate_schedule_time(&window.start_time)?;
            validate_schedule_time(&window.end_time)?;
            if window.days_of_week.is_empty()
                || window.days_of_week.iter().any(|day| !(0..=6).contains(day))
            {
                anyhow::bail!("schedule.windows days_of_week must contain values 0-6");
            }
        }

        // Validate VMAF threshold
        if self.quality.min_vmaf_score < 0.0 || self.quality.min_vmaf_score > 100.0 {
            anyhow::bail!(
                "min_vmaf_score must be between 0.0 and 100.0, got {}",
                self.quality.min_vmaf_score
            );
        }

        if let Some(vmaf_min_score) = self.transcode.vmaf_min_score {
            if !(0.0..=100.0).contains(&vmaf_min_score) {
                anyhow::bail!(
                    "vmaf_min_score must be between 0.0 and 100.0, got {}",
                    vmaf_min_score
                );
            }
        }

        Ok(())
    }

    /// Save config to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let mut config = self.clone();
        config.canonicalize_for_save();
        let content = toml::to_string_pretty(&config)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub(crate) fn migrate_legacy_notifications(&mut self) {
        if !self.notifications.targets.is_empty() {
            return;
        }

        let mut targets = Vec::new();
        let events = [
            self.notifications
                .notify_on_complete
                .then_some("completed".to_string()),
            self.notifications
                .notify_on_failure
                .then_some("failed".to_string()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        if let Some(discord_webhook) = self.notifications.discord_webhook.clone() {
            targets.push(NotificationTargetConfig {
                name: "Discord".to_string(),
                target_type: "discord".to_string(),
                endpoint_url: discord_webhook,
                auth_token: None,
                events: events.clone(),
                enabled: self.notifications.enabled,
            });
        }

        if let Some(webhook_url) = self.notifications.webhook_url.clone() {
            targets.push(NotificationTargetConfig {
                name: "Webhook".to_string(),
                target_type: "webhook".to_string(),
                endpoint_url: webhook_url,
                auth_token: None,
                events,
                enabled: self.notifications.enabled,
            });
        }

        self.notifications.targets = targets;
    }

    pub(crate) fn canonicalize_for_save(&mut self) {
        if !self.notifications.targets.is_empty() {
            self.notifications.webhook_url = None;
            self.notifications.discord_webhook = None;
            self.notifications.notify_on_complete = false;
            self.notifications.notify_on_failure = false;
        }
    }
}

fn validate_schedule_time(value: &str) -> Result<()> {
    let trimmed = value.trim();
    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() != 2 {
        anyhow::bail!("schedule time must be HH:MM");
    }
    let hour: u32 = parts[0].parse()?;
    let minute: u32 = parts[1].parse()?;
    if hour > 23 || minute > 59 {
        anyhow::bail!("schedule time must be HH:MM");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_notification_fields_migrate_into_targets() {
        let raw = r#"
            [transcode]
            size_reduction_threshold = 0.3
            min_bpp_threshold = 0.1
            min_file_size_mb = 50
            concurrent_jobs = 1

            [hardware]
            preferred_vendor = "cpu"
            allow_cpu_fallback = true

            [scanner]
            directories = []

            [notifications]
            enabled = true
            discord_webhook = "https://discord.com/api/webhooks/test"
            notify_on_complete = true
            notify_on_failure = true
        "#;

        let mut config: Config = toml::from_str(raw).expect("config");
        config.migrate_legacy_notifications();

        assert_eq!(config.notifications.targets.len(), 1);
        assert_eq!(config.notifications.targets[0].target_type, "discord");
        assert_eq!(
            config.notifications.targets[0].events,
            vec!["completed".to_string(), "failed".to_string()]
        );
    }

    #[test]
    fn save_canonicalizes_legacy_notification_fields() {
        let mut config = Config::default();
        config.notifications.targets = vec![NotificationTargetConfig {
            name: "Webhook".to_string(),
            target_type: "webhook".to_string(),
            endpoint_url: "https://example.com/webhook".to_string(),
            auth_token: None,
            events: vec!["completed".to_string()],
            enabled: true,
        }];
        config.notifications.webhook_url = Some("https://legacy.example.com".to_string());
        config.notifications.notify_on_complete = true;

        config.canonicalize_for_save();
        assert!(config.notifications.webhook_url.is_none());
        assert!(!config.notifications.notify_on_complete);
    }

    #[test]
    fn engine_mode_defaults_to_balanced() {
        assert_eq!(EngineMode::default(), EngineMode::Balanced);
        assert_eq!(EngineMode::Balanced.concurrent_jobs_for_cpu_count(8), 4);
    }
}
