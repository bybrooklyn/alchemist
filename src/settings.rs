use crate::config::Config;
use crate::db::Db;
use crate::error::{AlchemistError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsBundleResponse {
    pub settings: Config,
    pub source_of_truth: String,
    pub projection_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsConfigResponse {
    pub raw_toml: String,
    pub normalized: Config,
    pub source_of_truth: String,
    pub projection_status: String,
}

pub async fn project_config_to_db(db: &Db, config: &Config) -> Result<()> {
    db.replace_watch_dirs(&config.scanner.extra_watch_dirs)
        .await?;
    db.replace_notification_targets(&config.notifications.targets)
        .await?;
    db.replace_schedule_windows(&config.schedule.windows)
        .await?;
    db.replace_file_settings_projection(&config.files).await?;

    if let Some(theme_id) = config.appearance.active_theme_id.as_deref() {
        db.set_preference("active_theme_id", theme_id).await?;
    } else {
        db.delete_preference("active_theme_id").await?;
    }

    Ok(())
}

pub async fn save_config_and_project(db: &Db, config_path: &Path, config: &Config) -> Result<()> {
    config
        .save(config_path)
        .map_err(|err| AlchemistError::Config(err.to_string()))?;
    project_config_to_db(db, config).await
}

pub async fn load_and_project(db: &Db, config_path: &Path) -> Result<Config> {
    let config =
        Config::load(config_path).map_err(|err| AlchemistError::Config(err.to_string()))?;
    project_config_to_db(db, &config).await?;
    Ok(config)
}

pub fn load_raw_config(config_path: &Path) -> Result<String> {
    if !config_path.exists() {
        let default = Config::default();
        return toml::to_string_pretty(&default)
            .map_err(|err| AlchemistError::Config(err.to_string()));
    }

    std::fs::read_to_string(config_path).map_err(AlchemistError::Io)
}

pub fn parse_raw_config(raw_toml: &str) -> Result<Config> {
    let mut config: Config =
        toml::from_str(raw_toml).map_err(|err| AlchemistError::Config(err.to_string()))?;
    config.migrate_legacy_notifications();
    config.apply_env_overrides();
    config
        .validate()
        .map_err(|err| AlchemistError::Config(err.to_string()))?;
    Ok(config)
}

pub async fn apply_raw_config(db: &Db, config_path: &Path, raw_toml: &str) -> Result<Config> {
    let config = parse_raw_config(raw_toml)?;
    save_config_and_project(db, config_path, &config).await?;
    Ok(config)
}

pub fn bundle_response(config: Config) -> SettingsBundleResponse {
    let mut settings = config;
    settings.canonicalize_for_save();
    SettingsBundleResponse {
        settings,
        source_of_truth: "toml".to_string(),
        projection_status: "synced".to_string(),
    }
}

pub fn config_response(raw_toml: String, normalized: Config) -> SettingsConfigResponse {
    let mut normalized = normalized;
    normalized.canonicalize_for_save();
    SettingsConfigResponse {
        raw_toml,
        normalized,
        source_of_truth: "toml".to_string(),
        projection_status: "synced".to_string(),
    }
}
