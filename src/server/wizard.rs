//! Setup wizard API handlers.

use super::auth::build_session_cookie;
use super::{
    AppState, api_error_response, canonicalize_directory_path, config_write_blocked_response,
    hardware_error_response, refresh_file_watcher, replace_runtime_hardware,
    save_config_or_response,
};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString},
};
use axum::{
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
};
use chrono::Utc;
use rand::Rng;
use rand::TryRngCore;
use rand::rngs::OsRng;
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tracing::{error, info};

fn default_setup_min_bpp() -> f64 {
    0.1
}

fn default_setup_true() -> bool {
    true
}

fn default_setup_telemetry() -> bool {
    false
}

#[derive(Deserialize)]
pub(crate) struct SetupConfig {
    username: String,
    password: String,
    #[serde(default)]
    settings: Option<serde_json::Value>,
    #[serde(default)]
    size_reduction_threshold: f64,
    #[serde(default = "default_setup_min_bpp")]
    min_bpp_threshold: f64,
    #[serde(default)]
    min_file_size_mb: u64,
    #[serde(default)]
    concurrent_jobs: usize,
    #[serde(default)]
    directories: Vec<String>,
    #[serde(default = "default_setup_true")]
    allow_cpu_encoding: bool,
    #[serde(default = "default_setup_telemetry")]
    enable_telemetry: bool,
    #[serde(default)]
    output_codec: crate::config::OutputCodec,
    #[serde(default)]
    quality_profile: crate::config::QualityProfile,
}

pub(crate) fn normalize_setup_directories(
    directories: &[String],
) -> std::result::Result<Vec<String>, String> {
    let mut normalized = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for value in directories {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }

        let canonical = canonicalize_directory_path(trimmed, "directories")?;
        let canonical = canonical.to_string_lossy().to_string();
        if seen.insert(canonical.clone()) {
            normalized.push(canonical);
        }
    }

    Ok(normalized)
}

pub(crate) async fn setup_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(serde_json::json!({
        "setup_required": state.setup_required.load(Ordering::Relaxed),
        "enable_telemetry": config.system.enable_telemetry,
        "config_mutable": state.config_mutable
    }))
}

pub(crate) async fn setup_complete_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SetupConfig>,
) -> impl IntoResponse {
    if !state.setup_required.load(Ordering::Relaxed) {
        return api_error_response(
            StatusCode::FORBIDDEN,
            "SETUP_ALREADY_COMPLETE",
            "Setup already completed",
        );
    }

    let username = payload.username.trim();
    if username.len() < 3 {
        return api_error_response(
            StatusCode::BAD_REQUEST,
            "SETUP_USERNAME_INVALID",
            "username must be at least 3 characters",
        );
    }
    if payload.password.len() < 8 {
        return api_error_response(
            StatusCode::BAD_REQUEST,
            "SETUP_PASSWORD_INVALID",
            "password must be at least 8 characters",
        );
    }
    if payload.settings.is_none() && payload.concurrent_jobs == 0 {
        return api_error_response(
            StatusCode::BAD_REQUEST,
            "SETUP_CONCURRENT_JOBS_INVALID",
            "concurrent_jobs must be > 0",
        );
    }
    if payload.settings.is_none() && !(0.0..=1.0).contains(&payload.size_reduction_threshold) {
        return api_error_response(
            StatusCode::BAD_REQUEST,
            "SETUP_THRESHOLD_INVALID",
            "size_reduction_threshold must be 0.0-1.0",
        );
    }
    if payload.settings.is_none() && payload.min_bpp_threshold < 0.0 {
        return api_error_response(
            StatusCode::BAD_REQUEST,
            "SETUP_BPP_INVALID",
            "min_bpp_threshold must be >= 0.0",
        );
    }

    if !state.config_mutable {
        return config_write_blocked_response(state.config_path.as_path());
    }

    let mut next_config = match payload.settings {
        Some(raw_settings) => {
            // Deserialize the frontend SetupSettings into Config,
            // tolerating unknown fields and missing optional fields.
            let mut settings: crate::config::Config = match serde_json::from_value(raw_settings) {
                Ok(c) => c,
                Err(err) => {
                    return api_error_response(
                        StatusCode::BAD_REQUEST,
                        "SETUP_CONFIG_INVALID",
                        format!(
                            "Setup configuration is invalid: {}. \
                                 Please go back and check your settings.",
                            err
                        ),
                    );
                }
            };
            settings.scanner.directories =
                match normalize_setup_directories(&settings.scanner.directories) {
                    Ok(paths) => paths,
                    Err(msg) => {
                        return api_error_response(
                            StatusCode::BAD_REQUEST,
                            "SETUP_DIRECTORIES_INVALID",
                            msg,
                        );
                    }
                };
            settings
        }
        None => {
            let setup_directories = match normalize_setup_directories(&payload.directories) {
                Ok(paths) => paths,
                Err(msg) => {
                    return api_error_response(
                        StatusCode::BAD_REQUEST,
                        "SETUP_DIRECTORIES_INVALID",
                        msg,
                    );
                }
            };
            let mut config = state.config.read().await.clone();
            config.transcode.concurrent_jobs = payload.concurrent_jobs;
            config.transcode.size_reduction_threshold = payload.size_reduction_threshold;
            config.transcode.min_bpp_threshold = payload.min_bpp_threshold;
            config.transcode.min_file_size_mb = payload.min_file_size_mb;
            config.transcode.output_codec = payload.output_codec;
            config.transcode.quality_profile = payload.quality_profile;
            config.hardware.allow_cpu_encoding = payload.allow_cpu_encoding;
            config.scanner.directories = setup_directories;
            config.system.enable_telemetry = payload.enable_telemetry;
            config
        }
    };
    next_config.scanner.watch_enabled = true;

    if next_config.scanner.directories.is_empty() {
        return api_error_response(
            StatusCode::BAD_REQUEST,
            "SETUP_DIRECTORIES_REQUIRED",
            "At least one library directory must be configured.",
        );
    }

    if next_config.transcode.concurrent_jobs == 0 {
        return api_error_response(
            StatusCode::BAD_REQUEST,
            "SETUP_CONCURRENT_JOBS_REQUIRED",
            "Concurrent jobs must be at least 1.",
        );
    }

    if let Err(e) = next_config.validate() {
        return api_error_response(
            StatusCode::BAD_REQUEST,
            "SETUP_VALIDATION_FAILED",
            e.to_string(),
        );
    }

    let runtime_concurrent_jobs = next_config.transcode.concurrent_jobs;
    let runtime_engine_mode = next_config.system.engine_mode;

    let (hardware_info, probe_log) =
        match crate::system::hardware::detect_hardware_with_log(&next_config).await {
            Ok(result) => result,
            Err(err) => return hardware_error_response(&err),
        };

    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config_lock = state.config.write().await;
        *config_lock = next_config;
    }

    // Create User and Initial Session after config persistence succeeds.
    let mut salt_bytes = [0u8; 16];
    if let Err(e) = OsRng.try_fill_bytes(&mut salt_bytes) {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SETUP_SALT_GEN_FAILED",
            format!("Failed to generate salt: {}", e),
        );
    }
    let salt = match SaltString::encode_b64(&salt_bytes) {
        Ok(salt) => salt,
        Err(e) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SETUP_SALT_ENCODE_FAILED",
                format!("Failed to encode salt: {}", e),
            );
        }
    };
    let argon2 = Argon2::default();
    let password_hash = match argon2.hash_password(payload.password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(e) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SETUP_HASHING_FAILED",
                format!("Hashing failed: {}", e),
            );
        }
    };

    let user_id = match state.db.create_user(username, &password_hash).await {
        Ok(id) => id,
        Err(e) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SETUP_USER_CREATE_FAILED",
                format!("Failed to create user: {}", e),
            );
        }
    };

    let token: String = rand::rng()
        .sample_iter(rand::distr::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    let expires_at = Utc::now() + chrono::Duration::days(30);

    if let Err(e) = state.db.create_session(user_id, &token, expires_at).await {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SETUP_SESSION_CREATE_FAILED",
            format!("Failed to create session: {}", e),
        );
    }

    // Update Setup State (Hot Reload)
    state.agent.set_manual_override(true);
    *state.agent.engine_mode.write().await = runtime_engine_mode;
    state
        .agent
        .set_concurrent_jobs(runtime_concurrent_jobs)
        .await;
    replace_runtime_hardware(state.as_ref(), hardware_info, probe_log).await;
    refresh_file_watcher(&state).await;

    // Mark setup as complete
    state
        .setup_required
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // Start Scan (optional, but good for UX)
    // Use library_scanner so the UI can track progress via /api/scan/status
    let scanner = state.library_scanner.clone();
    let agent_for_analysis = state.agent.clone();
    tokio::spawn(async move {
        if let Err(e) = scanner.start_scan().await {
            error!("Background initial scan failed: {}", e);
            return;
        }
        loop {
            let status = scanner.get_status().await;
            if !status.is_running {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        agent_for_analysis.analyze_pending_jobs().await;
    });

    info!("Configuration saved via web setup. Auth info created.");

    let cookie = build_session_cookie(&token);
    (
        [(header::SET_COOKIE, cookie)],
        axum::Json(serde_json::json!({
            "status": "saved",
            "message": "Setup completed successfully.",
            "concurrent_jobs": runtime_concurrent_jobs
        })),
    )
        .into_response()
}
