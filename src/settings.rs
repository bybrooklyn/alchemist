use crate::config::{Config, NotificationTargetConfig};
use crate::db::Db;
use crate::error::{AlchemistError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
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
    // Single transaction across every projection table (RG-21): callers
    // report one error and the database still matches either the old or
    // the new config, never a mix.
    db.replace_config_projection(config).await
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
    mask_config_notification_targets(&mut settings);
    SettingsBundleResponse {
        settings,
        source_of_truth: "toml".to_string(),
        projection_status: "synced".to_string(),
    }
}

/// Sentinel returned on reads in place of a stored outbound secret
/// (webhook URLs, tokens, passwords). Write paths treat this value as
/// "unchanged" and restore the stored secret instead of persisting the
/// mask (SEC-4).
pub const MASKED_SECRET: &str = "********";

/// `config_json` keys whose values are outbound secrets and must never be
/// echoed back on a read. `webhook_url` is included deliberately: a Discord
/// webhook URL embeds the token, so the URL itself is the secret.
const SECRET_KEYS: &[&str] = &[
    "app_token",
    "auth_token",
    "bot_token",
    "password",
    "smtp_password",
    "webhook_url",
];

/// Replace every secret-bearing value in a notification target's
/// `config_json` (plus the legacy top-level credential fields) with
/// [`MASKED_SECRET`]. Only non-empty strings are masked so absent or empty
/// values keep their shape.
pub fn mask_notification_target_secrets(target: &mut NotificationTargetConfig) {
    mask_notification_config_json(&mut target.config_json);
    if target
        .auth_token
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        target.auth_token = Some(MASKED_SECRET.into());
    }
    if target
        .endpoint_url
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        target.endpoint_url = Some(MASKED_SECRET.into());
    }
}

/// Mask secret-bearing keys inside a raw notification `config_json` value.
/// Used by response paths that echo the stored JSON blob directly.
pub fn mask_notification_config_json(config_json: &mut JsonValue) {
    if let Some(config_map) = config_json.as_object_mut() {
        for key in SECRET_KEYS {
            let masked = config_map
                .get(*key)
                .and_then(JsonValue::as_str)
                .is_some_and(|value| !value.is_empty());
            if masked {
                config_map.insert((*key).to_string(), JsonValue::String(MASKED_SECRET.into()));
            }
        }
    }
}

/// Restore secrets masked by [`mask_notification_target_secrets`] from the
/// previously stored target. Any field still holding [`MASKED_SECRET`] takes
/// back the stored value; every other field is left untouched, so a client
/// can rotate a secret by sending a new value. Targets with no stored
/// counterpart (fresh adds) are returned unchanged.
pub fn restore_masked_notification_target(
    target: &mut NotificationTargetConfig,
    stored: &NotificationTargetConfig,
) {
    if target.auth_token.as_deref() == Some(MASKED_SECRET) {
        target.auth_token.clone_from(&stored.auth_token);
    }
    if target.endpoint_url.as_deref() == Some(MASKED_SECRET) {
        target.endpoint_url.clone_from(&stored.endpoint_url);
    }
    if let Some(config_map) = target.config_json.as_object_mut() {
        let stored_map = stored.config_json.as_object();
        for key in SECRET_KEYS {
            if config_map.get(*key).and_then(JsonValue::as_str) != Some(MASKED_SECRET) {
                continue;
            }
            match stored_map.and_then(|map| map.get(*key)) {
                Some(prior) => {
                    config_map.insert((*key).to_string(), prior.clone());
                }
                None => {
                    config_map.remove(*key);
                }
            }
        }
    }
}

/// Mask secrets on every notification target in a config (read paths and
/// SSE broadcast payloads).
pub fn mask_config_notification_targets(config: &mut Config) {
    for target in &mut config.notifications.targets {
        mask_notification_target_secrets(target);
    }
}

/// Restore masked secrets on every notification target in `payload` from
/// the matching stored target (same name and type). Used by write paths
/// that accept a full config after the client read a masked one, so a
/// GET → PUT round-trip cannot persist the mask over the real secret.
pub fn restore_masked_config_notification_targets(payload: &mut Config, stored: &Config) {
    for target in &mut payload.notifications.targets {
        if let Some(prior) = stored.notifications.targets.iter().find(|candidate| {
            candidate.name == target.name && candidate.target_type == target.target_type
        }) {
            restore_masked_notification_target(target, prior);
        }
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
