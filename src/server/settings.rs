//! Configuration get/set, validation handlers.

use super::{
    AppState, config_read_error_response, config_save_error_to_response,
    config_write_blocked_response, hardware_error_response, has_path_separator,
    normalize_optional_directory, normalize_optional_path, normalize_schedule_time,
    refresh_file_watcher, replace_runtime_hardware, save_config_or_response,
    validate_notification_url, validate_transcode_payload,
};
use crate::config::Config;
use crate::db::ApiTokenAccessLevel;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::sync::Arc;

// Transcode settings

#[derive(Deserialize, Serialize)]
pub(crate) struct TranscodeSettingsPayload {
    pub(crate) concurrent_jobs: usize,
    pub(crate) size_reduction_threshold: f64,
    pub(crate) min_bpp_threshold: f64,
    pub(crate) min_file_size_mb: u64,
    pub(crate) output_codec: crate::config::OutputCodec,
    pub(crate) quality_profile: crate::config::QualityProfile,
    #[serde(default)]
    pub(crate) threads: usize,
    #[serde(default = "crate::config::default_allow_fallback")]
    pub(crate) allow_fallback: bool,
    #[serde(default)]
    pub(crate) hdr_mode: crate::config::HdrMode,
    #[serde(default)]
    pub(crate) tonemap_algorithm: crate::config::TonemapAlgorithm,
    #[serde(default = "crate::config::default_tonemap_peak")]
    pub(crate) tonemap_peak: f32,
    #[serde(default = "crate::config::default_tonemap_desat")]
    pub(crate) tonemap_desat: f32,
    #[serde(default)]
    pub(crate) subtitle_mode: crate::config::SubtitleMode,
    #[serde(default)]
    pub(crate) stream_rules: crate::config::StreamRules,
}

pub(crate) async fn get_transcode_settings_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(TranscodeSettingsPayload {
        concurrent_jobs: config.transcode.concurrent_jobs,
        size_reduction_threshold: config.transcode.size_reduction_threshold,
        min_bpp_threshold: config.transcode.min_bpp_threshold,
        min_file_size_mb: config.transcode.min_file_size_mb,
        output_codec: config.transcode.output_codec,
        quality_profile: config.transcode.quality_profile,
        threads: config.transcode.threads,
        allow_fallback: config.transcode.allow_fallback,
        hdr_mode: config.transcode.hdr_mode,
        tonemap_algorithm: config.transcode.tonemap_algorithm,
        tonemap_peak: config.transcode.tonemap_peak,
        tonemap_desat: config.transcode.tonemap_desat,
        subtitle_mode: config.transcode.subtitle_mode,
        stream_rules: config.transcode.stream_rules.clone(),
    })
}

pub(crate) async fn update_transcode_settings_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<TranscodeSettingsPayload>,
) -> impl IntoResponse {
    if let Err(msg) = validate_transcode_payload(&payload) {
        return (StatusCode::BAD_REQUEST, msg).into_response();
    }

    let mut next_config = state.config.read().await.clone();
    next_config.transcode.concurrent_jobs = payload.concurrent_jobs;
    next_config.transcode.size_reduction_threshold = payload.size_reduction_threshold;
    next_config.transcode.min_bpp_threshold = payload.min_bpp_threshold;
    next_config.transcode.min_file_size_mb = payload.min_file_size_mb;
    next_config.transcode.output_codec = payload.output_codec;
    next_config.transcode.quality_profile = payload.quality_profile;
    next_config.transcode.threads = payload.threads;
    next_config.transcode.allow_fallback = payload.allow_fallback;
    next_config.transcode.hdr_mode = payload.hdr_mode;
    next_config.transcode.tonemap_algorithm = payload.tonemap_algorithm;
    next_config.transcode.tonemap_peak = payload.tonemap_peak;
    next_config.transcode.tonemap_desat = payload.tonemap_desat;
    next_config.transcode.subtitle_mode = payload.subtitle_mode;
    next_config.transcode.stream_rules = payload.stream_rules.clone();

    if let Err(e) = next_config.validate() {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }

    {
        let mut config = state.config.write().await;
        *config = next_config;
    }

    state.agent.set_manual_override(true);
    state
        .agent
        .set_concurrent_jobs(payload.concurrent_jobs)
        .await;

    StatusCode::OK.into_response()
}

// Hardware settings

#[derive(Serialize, Deserialize)]
pub(crate) struct HardwareSettingsPayload {
    allow_cpu_fallback: bool,
    allow_cpu_encoding: bool,
    cpu_preset: String,
    preferred_vendor: Option<String>,
    #[serde(default)]
    device_path: Option<String>,
}

pub(crate) async fn get_hardware_settings_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(HardwareSettingsPayload {
        allow_cpu_fallback: config.hardware.allow_cpu_fallback,
        allow_cpu_encoding: config.hardware.allow_cpu_encoding,
        cpu_preset: config.hardware.cpu_preset.to_string(),
        preferred_vendor: config.hardware.preferred_vendor.clone(),
        device_path: config.hardware.device_path.clone(),
    })
}

pub(crate) async fn update_hardware_settings_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<HardwareSettingsPayload>,
) -> impl IntoResponse {
    let mut next_config = state.config.read().await.clone();

    next_config.hardware.allow_cpu_fallback = payload.allow_cpu_fallback;
    next_config.hardware.allow_cpu_encoding = payload.allow_cpu_encoding;
    next_config.hardware.cpu_preset = match payload.cpu_preset.to_lowercase().as_str() {
        "slow" => crate::config::CpuPreset::Slow,
        "medium" => crate::config::CpuPreset::Medium,
        "fast" => crate::config::CpuPreset::Fast,
        "faster" => crate::config::CpuPreset::Faster,
        _ => crate::config::CpuPreset::Medium,
    };
    next_config.hardware.preferred_vendor = payload.preferred_vendor;
    next_config.hardware.device_path =
        match normalize_optional_path(payload.device_path.as_deref(), "device_path") {
            Ok(path) => path,
            Err(msg) => return (StatusCode::BAD_REQUEST, msg).into_response(),
        };

    if let Err(e) = next_config.validate() {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    let (hardware_info, probe_log) =
        match crate::system::hardware::detect_hardware_with_log(&next_config).await {
            Ok(result) => result,
            Err(err) => return hardware_error_response(&err),
        };

    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }

    {
        let mut config = state.config.write().await;
        *config = next_config;
    }
    replace_runtime_hardware(state.as_ref(), hardware_info, probe_log).await;

    StatusCode::OK.into_response()
}

// System settings

#[derive(Serialize, Deserialize)]
pub(crate) struct SystemSettingsPayload {
    monitoring_poll_interval: f64,
    conversion_upload_limit_gb: u32,
    conversion_download_retention_hours: u32,
    enable_telemetry: bool,
    #[serde(default)]
    watch_enabled: bool,
}

pub(crate) async fn get_system_settings_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(SystemSettingsPayload {
        monitoring_poll_interval: config.system.monitoring_poll_interval,
        conversion_upload_limit_gb: config.system.conversion_upload_limit_gb,
        conversion_download_retention_hours: config.system.conversion_download_retention_hours,
        enable_telemetry: config.system.enable_telemetry,
        watch_enabled: config.scanner.watch_enabled,
    })
}

pub(crate) async fn update_system_settings_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SystemSettingsPayload>,
) -> impl IntoResponse {
    if payload.monitoring_poll_interval < 0.5 || payload.monitoring_poll_interval > 10.0 {
        return (
            StatusCode::BAD_REQUEST,
            "monitoring_poll_interval must be between 0.5 and 10.0 seconds",
        )
            .into_response();
    }
    if payload.conversion_upload_limit_gb == 0 {
        return (
            StatusCode::BAD_REQUEST,
            "conversion_upload_limit_gb must be >= 1",
        )
            .into_response();
    }
    if !(1..=24).contains(&payload.conversion_download_retention_hours) {
        return (
            StatusCode::BAD_REQUEST,
            "conversion_download_retention_hours must be between 1 and 24",
        )
            .into_response();
    }

    let mut next_config = state.config.read().await.clone();
    next_config.system.monitoring_poll_interval = payload.monitoring_poll_interval;
    next_config.system.conversion_upload_limit_gb = payload.conversion_upload_limit_gb;
    next_config.system.conversion_download_retention_hours =
        payload.conversion_download_retention_hours;
    next_config.system.enable_telemetry = payload.enable_telemetry;
    next_config.scanner.watch_enabled = payload.watch_enabled;

    if let Err(e) = next_config.validate() {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }

    {
        let mut config = state.config.write().await;
        *config = next_config;
    }

    refresh_file_watcher(&state).await;

    (StatusCode::OK, "Settings updated").into_response()
}

// Settings bundle

pub(crate) async fn get_settings_bundle_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.read().await.clone();
    axum::Json(crate::settings::bundle_response(config)).into_response()
}

pub(crate) async fn update_settings_bundle_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<Config>,
) -> impl IntoResponse {
    if let Err(err) = payload.validate() {
        return (StatusCode::BAD_REQUEST, err.to_string()).into_response();
    }

    let (hardware_info, probe_log) =
        match crate::system::hardware::detect_hardware_with_log(&payload).await {
            Ok(result) => result,
            Err(err) => return hardware_error_response(&err),
        };

    if let Err(response) = save_config_or_response(&state, &payload).await {
        return *response;
    }

    {
        let mut config = state.config.write().await;
        *config = payload.clone();
    }

    state.agent.set_manual_override(true);
    *state.agent.engine_mode.write().await = payload.system.engine_mode;
    state
        .agent
        .set_concurrent_jobs(payload.transcode.concurrent_jobs)
        .await;
    replace_runtime_hardware(state.as_ref(), hardware_info, probe_log).await;
    refresh_file_watcher(&state).await;
    state.scheduler.trigger();

    axum::Json(crate::settings::bundle_response(payload)).into_response()
}

// Setting preferences

#[derive(Deserialize)]
pub(crate) struct SettingPreferencePayload {
    key: String,
    value: String,
}

#[derive(Serialize)]
struct SettingPreferenceResponse {
    key: String,
    value: String,
}

pub(crate) async fn set_setting_preference_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SettingPreferencePayload>,
) -> impl IntoResponse {
    let key = payload.key.trim();
    if key.is_empty() {
        return (StatusCode::BAD_REQUEST, "key must not be empty").into_response();
    }

    match state.db.set_preference(key, payload.value.as_str()).await {
        Ok(_) => axum::Json(SettingPreferenceResponse {
            key: key.to_string(),
            value: payload.value,
        })
        .into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub(crate) async fn get_setting_preference_handler(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match state.db.get_preference(key.as_str()).await {
        Ok(Some(value)) => axum::Json(SettingPreferenceResponse { key, value }).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

// Raw config

#[derive(Deserialize)]
pub(crate) struct RawConfigPayload {
    raw_toml: String,
}

pub(crate) async fn get_settings_config_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let raw_toml = match crate::settings::load_raw_config(state.config_path.as_path()) {
        Ok(raw_toml) => raw_toml,
        Err(err) => return config_read_error_response("load raw config", &err),
    };
    let normalized = state.config.read().await.clone();
    axum::Json(crate::settings::config_response(raw_toml, normalized)).into_response()
}

pub(crate) async fn update_settings_config_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<RawConfigPayload>,
) -> impl IntoResponse {
    let config = match crate::settings::parse_raw_config(&payload.raw_toml) {
        Ok(config) => config,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };

    if let Err(err) = config.validate() {
        return (StatusCode::BAD_REQUEST, err.to_string()).into_response();
    }

    let (hardware_info, probe_log) =
        match crate::system::hardware::detect_hardware_with_log(&config).await {
            Ok(result) => result,
            Err(err) => return hardware_error_response(&err),
        };

    if !state.config_mutable {
        return config_write_blocked_response(state.config_path.as_path());
    }

    if let Some(parent) = state.config_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                return config_save_error_to_response(&state.config_path, &anyhow::Error::new(err));
            }
        }
    }

    if let Err(err) = crate::settings::save_config_and_project(
        state.db.as_ref(),
        state.config_path.as_path(),
        &config,
    )
    .await
    {
        return config_save_error_to_response(
            &state.config_path,
            &anyhow::Error::msg(err.to_string()),
        );
    }

    {
        let mut config_lock = state.config.write().await;
        *config_lock = config.clone();
    }

    state.agent.set_manual_override(true);
    *state.agent.engine_mode.write().await = config.system.engine_mode;
    state
        .agent
        .set_concurrent_jobs(config.transcode.concurrent_jobs)
        .await;
    replace_runtime_hardware(state.as_ref(), hardware_info, probe_log).await;
    refresh_file_watcher(&state).await;
    state.scheduler.trigger();

    axum::Json(crate::settings::config_response(payload.raw_toml, config)).into_response()
}

// Notification settings

#[derive(Deserialize)]
pub(crate) struct AddNotificationTargetPayload {
    name: String,
    target_type: String,
    #[serde(default)]
    config_json: JsonValue,
    #[serde(default)]
    endpoint_url: Option<String>,
    #[serde(default)]
    auth_token: Option<String>,
    events: Vec<String>,
    enabled: bool,
}

#[derive(Serialize)]
pub(crate) struct NotificationTargetResponse {
    id: i64,
    name: String,
    target_type: String,
    config_json: JsonValue,
    events: Vec<String>,
    enabled: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub(crate) struct NotificationsSettingsResponse {
    daily_summary_time_local: String,
    targets: Vec<NotificationTargetResponse>,
}

#[derive(Deserialize)]
pub(crate) struct UpdateNotificationsSettingsPayload {
    daily_summary_time_local: String,
}

fn normalize_notification_payload(
    payload: &AddNotificationTargetPayload,
) -> crate::config::NotificationTargetConfig {
    let mut config_json = payload.config_json.clone();
    if !config_json.is_object() {
        config_json = JsonValue::Object(JsonMap::new());
    }

    let Some(config_map) = config_json.as_object_mut() else {
        unreachable!("notification config_json should always be an object here");
    };
    match payload.target_type.as_str() {
        "discord_webhook" | "discord" if !config_map.contains_key("webhook_url") => {
            if let Some(endpoint_url) = payload.endpoint_url.as_ref() {
                config_map.insert(
                    "webhook_url".to_string(),
                    JsonValue::String(endpoint_url.clone()),
                );
            }
        }
        "gotify" => {
            if let Some(endpoint_url) = payload.endpoint_url.as_ref() {
                config_map
                    .entry("server_url".to_string())
                    .or_insert_with(|| JsonValue::String(endpoint_url.clone()));
            }
            if let Some(auth_token) = payload.auth_token.as_ref() {
                config_map
                    .entry("app_token".to_string())
                    .or_insert_with(|| JsonValue::String(auth_token.clone()));
            }
        }
        "webhook" => {
            if let Some(endpoint_url) = payload.endpoint_url.as_ref() {
                config_map
                    .entry("url".to_string())
                    .or_insert_with(|| JsonValue::String(endpoint_url.clone()));
            }
            if let Some(auth_token) = payload.auth_token.as_ref() {
                config_map
                    .entry("auth_token".to_string())
                    .or_insert_with(|| JsonValue::String(auth_token.clone()));
            }
        }
        _ => {}
    }

    let mut target = crate::config::NotificationTargetConfig {
        name: payload.name.clone(),
        target_type: payload.target_type.clone(),
        config_json,
        endpoint_url: payload.endpoint_url.clone(),
        auth_token: payload.auth_token.clone(),
        events: payload.events.clone(),
        enabled: payload.enabled,
    };
    target.migrate_legacy_shape();
    target
}

fn notification_target_response(
    target: crate::db::NotificationTarget,
) -> NotificationTargetResponse {
    NotificationTargetResponse {
        id: target.id,
        name: target.name,
        target_type: target.target_type,
        config_json: serde_json::from_str(&target.config_json)
            .unwrap_or_else(|_| JsonValue::Object(JsonMap::new())),
        events: serde_json::from_str(&target.events).unwrap_or_default(),
        enabled: target.enabled,
        created_at: target.created_at,
    }
}

async fn validate_notification_target(
    state: &AppState,
    target: &crate::config::NotificationTargetConfig,
) -> std::result::Result<(), String> {
    target.validate().map_err(|err| err.to_string())?;

    let allow_local = state
        .config
        .read()
        .await
        .notifications
        .allow_local_notifications;
    let url = match target.target_type.as_str() {
        "discord_webhook" => target
            .config_json
            .get("webhook_url")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        "gotify" => target
            .config_json
            .get("server_url")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        "webhook" => target
            .config_json
            .get("url")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        _ => None,
    };

    if let Some(url) = url {
        validate_notification_url(&url, allow_local).await?;
    }

    Ok(())
}

pub(crate) async fn get_notifications_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_notification_targets().await {
        Ok(t) => {
            let daily_summary_time_local = state
                .config
                .read()
                .await
                .notifications
                .daily_summary_time_local
                .clone();
            axum::Json(NotificationsSettingsResponse {
                daily_summary_time_local,
                targets: t
                    .into_iter()
                    .map(notification_target_response)
                    .collect::<Vec<_>>(),
            })
            .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub(crate) async fn update_notifications_settings_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<UpdateNotificationsSettingsPayload>,
) -> impl IntoResponse {
    let mut next_config = state.config.read().await.clone();
    next_config.notifications.daily_summary_time_local = payload.daily_summary_time_local;
    if let Err(err) = next_config.validate() {
        return (StatusCode::BAD_REQUEST, err.to_string()).into_response();
    }
    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config = state.config.write().await;
        *config = next_config;
    }
    StatusCode::OK.into_response()
}

pub(crate) async fn add_notification_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<AddNotificationTargetPayload>,
) -> impl IntoResponse {
    let target = normalize_notification_payload(&payload);
    if let Err(msg) = validate_notification_target(&state, &target).await {
        return (StatusCode::BAD_REQUEST, msg).into_response();
    }

    let mut next_config = state.config.read().await.clone();
    next_config.notifications.targets.push(target);

    if let Err(e) = next_config.validate() {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }
    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config = state.config.write().await;
        *config = next_config;
    }

    match state.db.get_notification_targets().await {
        Ok(mut targets) => targets
            .pop()
            .map(|target| axum::Json(notification_target_response(target)).into_response())
            .unwrap_or_else(|| StatusCode::OK.into_response()),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub(crate) async fn delete_notification_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let target_index = match state.db.get_notification_targets().await {
        Ok(targets) => targets.iter().position(|target| target.id == id),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let Some(target_index) = target_index else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let mut next_config = state.config.read().await.clone();
    if target_index >= next_config.notifications.targets.len() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "notification settings projection is out of sync with config",
        )
            .into_response();
    }
    next_config.notifications.targets.remove(target_index);
    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config = state.config.write().await;
        *config = next_config;
    }
    StatusCode::OK.into_response()
}

pub(crate) async fn test_notification_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<AddNotificationTargetPayload>,
) -> impl IntoResponse {
    let target_config = normalize_notification_payload(&payload);
    if let Err(msg) = validate_notification_target(&state, &target_config).await {
        return (StatusCode::BAD_REQUEST, msg).into_response();
    }

    let target = crate::db::NotificationTarget {
        id: 0,
        name: target_config.name,
        target_type: target_config.target_type,
        config_json: target_config.config_json.to_string(),
        events: serde_json::to_string(&target_config.events).unwrap_or_else(|_| "[]".to_string()),
        enabled: target_config.enabled,
        created_at: chrono::Utc::now(),
    };

    match state.notification_manager.send_test(&target).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// API token settings

#[derive(Deserialize)]
pub(crate) struct CreateApiTokenPayload {
    name: String,
    access_level: ApiTokenAccessLevel,
}

#[derive(Serialize)]
pub(crate) struct CreatedApiTokenResponse {
    token: crate::db::ApiToken,
    plaintext_token: String,
}

pub(crate) async fn list_api_tokens_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.list_api_tokens().await {
        Ok(tokens) => axum::Json(tokens).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub(crate) async fn create_api_token_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<CreateApiTokenPayload>,
) -> impl IntoResponse {
    if payload.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "token name must not be empty").into_response();
    }

    let plaintext_token = format!(
        "alc_tok_{}",
        rand::rng()
            .sample_iter(rand::distr::Alphanumeric)
            .take(48)
            .map(char::from)
            .collect::<String>()
    );

    match state
        .db
        .create_api_token(payload.name.trim(), &plaintext_token, payload.access_level)
        .await
    {
        Ok(token) => axum::Json(CreatedApiTokenResponse {
            token,
            plaintext_token,
        })
        .into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub(crate) async fn revoke_api_token_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.revoke_api_token(id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) if super::is_row_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

// Schedule settings

pub(crate) async fn get_schedule_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_schedule_windows().await {
        Ok(w) => axum::Json(serde_json::json!(w)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct AddSchedulePayload {
    start_time: String,
    end_time: String,
    days_of_week: Vec<i32>,
    enabled: bool,
}

pub(crate) async fn add_schedule_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<AddSchedulePayload>,
) -> impl IntoResponse {
    if payload.days_of_week.is_empty()
        || payload.days_of_week.iter().any(|day| *day < 0 || *day > 6)
    {
        return (
            StatusCode::BAD_REQUEST,
            "days_of_week must include values 0-6",
        )
            .into_response();
    }

    let start_time = match normalize_schedule_time(&payload.start_time) {
        Some(value) => value,
        None => {
            return (StatusCode::BAD_REQUEST, "start_time must be HH:MM").into_response();
        }
    };
    let end_time = match normalize_schedule_time(&payload.end_time) {
        Some(value) => value,
        None => return (StatusCode::BAD_REQUEST, "end_time must be HH:MM").into_response(),
    };

    let mut next_config = state.config.read().await.clone();
    next_config
        .schedule
        .windows
        .push(crate::config::ScheduleWindowConfig {
            start_time: start_time.clone(),
            end_time: end_time.clone(),
            days_of_week: payload.days_of_week.clone(),
            enabled: payload.enabled,
        });

    if let Err(e) = next_config.validate() {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }
    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config = state.config.write().await;
        *config = next_config;
    }
    state.scheduler.trigger();

    match state.db.get_schedule_windows().await {
        Ok(mut windows) => windows
            .pop()
            .map(|window| axum::Json(serde_json::json!(window)).into_response())
            .unwrap_or_else(|| StatusCode::OK.into_response()),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub(crate) async fn delete_schedule_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let window_index = match state.db.get_schedule_windows().await {
        Ok(windows) => windows.iter().position(|window| window.id == id),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let Some(window_index) = window_index else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let mut next_config = state.config.read().await.clone();
    if window_index >= next_config.schedule.windows.len() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "schedule settings projection is out of sync with config",
        )
            .into_response();
    }
    next_config.schedule.windows.remove(window_index);
    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config = state.config.write().await;
        *config = next_config;
    }
    state.scheduler.trigger();
    StatusCode::OK.into_response()
}

// File settings

pub(crate) async fn get_file_settings_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(serde_json::json!({
        "id": 1,
        "delete_source": config.files.delete_source,
        "output_extension": config.files.output_extension,
        "output_suffix": config.files.output_suffix,
        "replace_strategy": config.files.replace_strategy,
        "output_root": config.files.output_root,
    }))
    .into_response()
}

#[derive(Deserialize)]
pub(crate) struct UpdateFileSettingsPayload {
    delete_source: bool,
    output_extension: String,
    output_suffix: String,
    replace_strategy: String,
    #[serde(default)]
    output_root: Option<String>,
}

pub(crate) async fn update_file_settings_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<UpdateFileSettingsPayload>,
) -> impl IntoResponse {
    if has_path_separator(&payload.output_extension) || has_path_separator(&payload.output_suffix) {
        return (
            StatusCode::BAD_REQUEST,
            "output_extension and output_suffix must not contain path separators",
        )
            .into_response();
    }

    let output_root =
        match normalize_optional_directory(payload.output_root.as_deref(), "output_root") {
            Ok(value) => value,
            Err(msg) => return (StatusCode::BAD_REQUEST, msg).into_response(),
        };

    let mut next_config = state.config.read().await.clone();
    next_config.files.delete_source = payload.delete_source;
    next_config.files.output_extension = payload.output_extension.clone();
    next_config.files.output_suffix = payload.output_suffix.clone();
    next_config.files.replace_strategy = payload.replace_strategy.clone();
    next_config.files.output_root = output_root.clone();

    if let Err(e) = next_config.validate() {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }
    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config = state.config.write().await;
        *config = next_config;
    }
    axum::Json(serde_json::json!({
        "id": 1,
        "delete_source": payload.delete_source,
        "output_extension": payload.output_extension,
        "output_suffix": payload.output_suffix,
        "replace_strategy": payload.replace_strategy,
        "output_root": output_root,
    }))
    .into_response()
}

// UI Preferences

#[derive(Deserialize, Serialize)]
pub(crate) struct UiPreferences {
    active_theme_id: Option<String>,
}

pub(crate) async fn get_preferences_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(UiPreferences {
        active_theme_id: config.appearance.active_theme_id.clone(),
    })
    .into_response()
}

pub(crate) async fn update_preferences_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<UiPreferences>,
) -> impl IntoResponse {
    let mut next_config = state.config.read().await.clone();
    next_config.appearance.active_theme_id = payload.active_theme_id;
    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config = state.config.write().await;
        *config = next_config;
    }
    StatusCode::OK.into_response()
}
