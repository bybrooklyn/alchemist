//! HTTP server module: routes, state, middleware, and API handlers.

pub mod auth;
pub mod conversion;
pub mod jobs;
pub mod middleware;
pub mod scan;
pub mod settings;
pub mod sse;
pub mod stats;
pub mod system;
pub mod wizard;

#[cfg(test)]
mod tests;

use crate::Agent;
use crate::Transcoder;
use crate::config::Config;
use crate::db::{Db, EventChannels};
use crate::error::{AlchemistError, Result};
use crate::system::hardware::{HardwareInfo, HardwareProbeLog, HardwareState};
use axum::{
    Router,
    extract::State,
    http::{StatusCode, Uri, header},
    middleware as axum_middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
#[cfg(feature = "embed-web")]
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::net::lookup_host;
use tokio::sync::{Mutex, RwLock};
use tokio::time::Duration;
#[cfg(not(feature = "embed-web"))]
use tracing::warn;
use tracing::{error, info};
use uuid::Uuid;

use middleware::RateLimitEntry;

#[cfg(feature = "embed-web")]
#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

fn load_static_asset(path: &str) -> Option<Vec<u8>> {
    sanitize_asset_path(path)?;

    #[cfg(feature = "embed-web")]
    if let Some(content) = Assets::get(path) {
        return Some(content.data.into_owned());
    }

    let full_path = PathBuf::from("web/dist").join(path);
    fs::read(full_path).ok()
}

pub(crate) fn api_error_response(
    status: StatusCode,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Response {
    (
        status,
        axum::Json(serde_json::json!({
            "error": {
                "code": code.into(),
                "message": message.into(),
            }
        })),
    )
        .into_response()
}

pub struct AppState {
    pub db: Arc<Db>,
    pub config: Arc<RwLock<Config>>,
    pub agent: Arc<Agent>,
    pub transcoder: Arc<Transcoder>,
    pub scheduler: crate::scheduler::SchedulerHandle,
    pub event_channels: Arc<EventChannels>,
    pub setup_required: Arc<AtomicBool>,
    pub start_time: Instant,
    pub telemetry_runtime_id: String,
    pub notification_manager: Arc<crate::notifications::NotificationManager>,
    pub sys: Mutex<sysinfo::System>,
    pub file_watcher: Arc<crate::system::watcher::FileWatcher>,
    pub library_scanner: Arc<crate::system::scanner::LibraryScanner>,
    pub config_path: PathBuf,
    pub config_mutable: bool,
    pub hardware_state: HardwareState,
    pub hardware_probe_log: Arc<tokio::sync::RwLock<HardwareProbeLog>>,
    pub resources_cache: Arc<tokio::sync::Mutex<Option<(serde_json::Value, std::time::Instant)>>>,
    pub library_intelligence_cache:
        Arc<tokio::sync::Mutex<Option<(serde_json::Value, std::time::Instant)>>>,
    pub library_health_scan_in_progress: Arc<AtomicBool>,
    pub(crate) login_rate_limiter: Mutex<HashMap<IpAddr, RateLimitEntry>>,
    pub(crate) global_rate_limiter: Mutex<HashMap<IpAddr, RateLimitEntry>>,
    pub(crate) sse_connections: Arc<std::sync::atomic::AtomicUsize>,
    /// IPs whose proxy headers are trusted. Empty = trust all private ranges.
    pub(crate) trusted_proxies: Vec<IpAddr>,
    /// If set, setup endpoints require `?token=<value>` query parameter.
    pub(crate) setup_token: Option<String>,
}

pub struct RunServerArgs {
    pub db: Arc<Db>,
    pub config: Arc<RwLock<Config>>,
    pub agent: Arc<Agent>,
    pub transcoder: Arc<Transcoder>,
    pub scheduler: crate::scheduler::SchedulerHandle,
    pub event_channels: Arc<EventChannels>,
    pub setup_required: bool,
    pub config_path: PathBuf,
    pub config_mutable: bool,
    pub hardware_state: HardwareState,
    pub hardware_probe_log: Arc<tokio::sync::RwLock<HardwareProbeLog>>,
    pub notification_manager: Arc<crate::notifications::NotificationManager>,
    pub file_watcher: Arc<crate::system::watcher::FileWatcher>,
    pub library_scanner: Arc<crate::system::scanner::LibraryScanner>,
    pub library_intelligence_cache:
        Arc<tokio::sync::Mutex<Option<(serde_json::Value, std::time::Instant)>>>,
    pub library_health_scan_in_progress: Arc<AtomicBool>,
}

pub async fn run_server(args: RunServerArgs) -> Result<()> {
    let RunServerArgs {
        db,
        config,
        agent,
        transcoder,
        scheduler,
        event_channels,
        setup_required,
        config_path,
        config_mutable,
        hardware_state,
        hardware_probe_log,
        notification_manager,
        file_watcher,
        library_scanner,
        library_intelligence_cache,
        library_health_scan_in_progress,
    } = args;
    #[cfg(not(feature = "embed-web"))]
    {
        let web_dist = PathBuf::from("web/dist");
        if !web_dist.exists() {
            let cwd = std::env::current_dir()
                .map(|p| format!("{}/", p.display()))
                .unwrap_or_default();
            warn!(
                "web/dist not found at {}web/dist — frontend will not be served. \
                 Build it first with `just web-build` or run from the repo root.",
                cwd
            );
        }
    }

    // Initialize sysinfo
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    // Read setup token from environment (opt-in security layer).
    let setup_token = std::env::var("ALCHEMIST_SETUP_TOKEN").ok();
    if setup_token.is_some() {
        info!("ALCHEMIST_SETUP_TOKEN is set — setup endpoints require token query param");
    }

    // Parse trusted proxy IPs from config. Unparseable entries are logged and skipped.
    let trusted_proxies: Vec<IpAddr> = {
        let cfg = config.read().await;
        cfg.system
            .trusted_proxies
            .iter()
            .filter_map(|s| {
                s.parse::<IpAddr>()
                    .map_err(|_| {
                        error!("Invalid trusted_proxy entry (not a valid IP address): {s}");
                    })
                    .ok()
            })
            .collect()
    };
    if !trusted_proxies.is_empty() {
        info!(
            "Trusted proxies configured ({}): only these IPs will be trusted for X-Forwarded-For headers",
            trusted_proxies.len()
        );
    }

    let state = Arc::new(AppState {
        db,
        config,
        agent,
        transcoder,
        scheduler,
        event_channels,
        setup_required: Arc::new(AtomicBool::new(setup_required)),
        start_time: std::time::Instant::now(),
        telemetry_runtime_id: Uuid::new_v4().to_string(),
        notification_manager,
        sys: Mutex::new(sys),
        file_watcher,
        library_scanner,
        config_path,
        config_mutable,
        hardware_state,
        hardware_probe_log,
        resources_cache: Arc::new(tokio::sync::Mutex::new(None)),
        library_intelligence_cache,
        library_health_scan_in_progress,
        login_rate_limiter: Mutex::new(HashMap::new()),
        global_rate_limiter: Mutex::new(HashMap::new()),
        sse_connections: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        trusted_proxies,
        setup_token,
    });

    // Clone agent for shutdown handler before moving state into router
    let shutdown_agent = state.agent.clone();

    let app = app_router(state.clone());

    let port = std::env::var("ALCHEMIST_SERVER_PORT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value.trim().parse::<u16>().map_err(|_| {
                AlchemistError::Config("ALCHEMIST_SERVER_PORT must be a valid u16".to_string())
            })
        })
        .transpose()?
        .unwrap_or(3000);
    let user_specified_port = std::env::var("ALCHEMIST_SERVER_PORT")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .is_some();
    let max_attempts: u16 = if user_specified_port { 1 } else { 10 };
    let mut listener = None;
    let mut bound_port = port;

    for attempt in 0..max_attempts {
        let try_port = port.saturating_add(attempt);
        let addr = format!("0.0.0.0:{try_port}");
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => {
                bound_port = try_port;
                listener = Some(l);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if user_specified_port {
                    return Err(AlchemistError::Config(format!(
                        "Port {try_port} is already in use. Set ALCHEMIST_SERVER_PORT to a different port."
                    )));
                }
                let next = try_port.saturating_add(1);
                if attempt + 1 < max_attempts {
                    tracing::warn!("Port {try_port} is in use, trying {next}");
                } else {
                    tracing::warn!("Port {try_port} is in use, no more ports to try");
                }
            }
            Err(e) => return Err(AlchemistError::Io(e)),
        }
    }

    let listener = listener.ok_or_else(|| {
        AlchemistError::Config(format!(
            "Could not bind to any port in range {port}–{}. Set ALCHEMIST_SERVER_PORT to use a specific port.",
            port.saturating_add(max_attempts - 1)
        ))
    })?;

    if bound_port != port {
        tracing::warn!(
            "Port {} was in use — Alchemist is listening on http://0.0.0.0:{bound_port} instead",
            port
        );
        info!("listening on http://0.0.0.0:{bound_port}");
    } else {
        info!("listening on http://0.0.0.0:{bound_port}");
    }

    // Run server with graceful shutdown on Ctrl+C
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        // Wait for shutdown signal
        let ctrl_c = async {
            if let Err(err) = tokio::signal::ctrl_c().await {
                error!("Failed to install Ctrl+C handler: {}", err);
                std::future::pending::<()>().await;
            }
        };

        #[cfg(unix)]
        let terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                }
                Err(err) => {
                    error!("Failed to install signal handler: {}", err);
                    std::future::pending::<()>().await;
                }
            }
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received Ctrl+C, initiating graceful shutdown...");
            }
            _ = terminate => {
                info!("Received SIGTERM, initiating graceful shutdown...");
            }
        }

        // Forceful immediate shutdown of active jobs
        shutdown_agent.graceful_shutdown().await;
        info!("Shutdown complete. Forcing process exit.");
        std::process::exit(0);
    })
    .await
    .map_err(|e| AlchemistError::Unknown(format!("Server error: {}", e)))?;

    Ok(())
}

fn app_router(state: Arc<AppState>) -> Router {
    use auth::*;
    use conversion::*;
    use jobs::*;
    use scan::*;
    use settings::*;
    use sse::*;
    use stats::*;
    use system::*;
    use wizard::*;

    Router::new()
        // API Routes
        .route("/api/scan/start", post(start_scan_handler))
        .route("/api/scan/status", get(get_scan_status_handler))
        .route("/api/scan", post(scan_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/stats/aggregated", get(aggregated_stats_handler))
        .route("/api/stats/daily", get(daily_stats_handler))
        .route("/api/stats/detailed", get(detailed_stats_handler))
        .route("/api/stats/savings", get(savings_summary_handler))
        .route("/api/stats/skip-reasons", get(skip_reasons_handler))
        // Canonical job list endpoint.
        .route("/api/jobs", get(jobs_table_handler))
        .route("/api/jobs/table", get(jobs_table_handler))
        .route("/api/jobs/enqueue", post(enqueue_job_handler))
        .route("/api/jobs/batch", post(batch_jobs_handler))
        .route("/api/logs/history", get(logs_history_handler))
        .route("/api/logs", delete(clear_logs_handler))
        .route("/api/jobs/restart-failed", post(restart_failed_handler))
        .route("/api/jobs/clear-completed", post(clear_completed_handler))
        .route("/api/jobs/clear-history", post(clear_history_handler))
        .route("/api/jobs/:id/cancel", post(cancel_job_handler))
        .route("/api/jobs/:id/priority", post(update_job_priority_handler))
        .route("/api/jobs/:id/restart", post(restart_job_handler))
        .route("/api/jobs/:id/delete", post(delete_job_handler))
        .route("/api/jobs/:id/details", get(get_job_detail_handler))
        .route("/api/conversion/uploads", post(upload_conversion_handler))
        .route("/api/conversion/preview", post(preview_conversion_handler))
        .route(
            "/api/conversion/jobs/:id/start",
            post(start_conversion_job_handler),
        )
        .route(
            "/api/conversion/jobs/:id",
            get(get_conversion_job_handler).delete(delete_conversion_job_handler),
        )
        .route(
            "/api/conversion/jobs/:id/download",
            get(download_conversion_job_handler),
        )
        .route("/api/events", get(sse_handler))
        .route("/api/engine/pause", post(pause_engine_handler))
        .route("/api/engine/resume", post(resume_engine_handler))
        .route("/api/engine/drain", post(drain_engine_handler))
        .route("/api/engine/stop-drain", post(stop_drain_handler))
        .route("/api/engine/restart", post(restart_engine_handler))
        .route(
            "/api/engine/mode",
            get(get_engine_mode_handler).post(set_engine_mode_handler),
        )
        .route("/api/engine/status", get(engine_status_handler))
        .route("/api/processor/status", get(processor_status_handler))
        .route(
            "/api/settings/transcode",
            get(get_transcode_settings_handler).post(update_transcode_settings_handler),
        )
        .route(
            "/api/settings/system",
            get(get_system_settings_handler).post(update_system_settings_handler),
        )
        .route(
            "/api/settings/bundle",
            get(get_settings_bundle_handler).put(update_settings_bundle_handler),
        )
        .route(
            "/api/settings/preferences",
            post(set_setting_preference_handler),
        )
        .route(
            "/api/settings/preferences/:key",
            get(get_setting_preference_handler),
        )
        .route(
            "/api/settings/config",
            get(get_settings_config_handler).put(update_settings_config_handler),
        )
        .route(
            "/api/settings/watch-dirs",
            get(get_watch_dirs_handler).post(add_watch_dir_handler),
        )
        .route("/api/settings/folders", post(sync_watch_dirs_handler))
        .route(
            "/api/settings/watch-dirs/:id",
            delete(remove_watch_dir_handler),
        )
        .route(
            "/api/settings/watch-dirs/:id/reanalyze",
            post(reanalyze_watch_dir_handler),
        )
        .route(
            "/api/watch-dirs/:id/profile",
            axum::routing::patch(assign_watch_dir_profile_handler),
        )
        .route("/api/profiles/presets", get(get_profile_presets_handler))
        .route(
            "/api/profiles",
            get(list_profiles_handler).post(create_profile_handler),
        )
        .route(
            "/api/profiles/:id",
            axum::routing::put(update_profile_handler).delete(delete_profile_handler),
        )
        .route(
            "/api/settings/notifications",
            get(get_notifications_handler)
                .put(update_notifications_settings_handler)
                .post(add_notification_handler),
        )
        .route(
            "/api/settings/notifications/:id",
            delete(delete_notification_handler),
        )
        .route(
            "/api/settings/notifications/test",
            post(test_notification_handler),
        )
        .route(
            "/api/settings/api-tokens",
            get(list_api_tokens_handler).post(create_api_token_handler),
        )
        .route(
            "/api/settings/api-tokens/:id",
            delete(revoke_api_token_handler),
        )
        .route(
            "/api/settings/files",
            get(get_file_settings_handler).post(update_file_settings_handler),
        )
        .route(
            "/api/settings/schedule",
            get(get_schedule_handler).post(add_schedule_handler),
        )
        .route(
            "/api/settings/hardware",
            get(get_hardware_settings_handler).post(update_hardware_settings_handler),
        )
        .route(
            "/api/settings/schedule/:id",
            delete(delete_schedule_handler),
        )
        // Health Check Routes
        .route("/api/health", get(health_handler))
        .route("/api/ready", get(ready_handler))
        // System Routes
        .route("/api/system/resources", get(system_resources_handler))
        .route("/api/system/info", get(get_system_info_handler))
        .route("/api/system/update", get(get_system_update_handler))
        .route("/api/system/hardware", get(get_hardware_info_handler))
        .route("/api/system/backup", post(backup_database_handler))
        .route(
            "/api/system/hardware/probe-log",
            get(get_hardware_probe_log_handler),
        )
        .route(
            "/api/library/intelligence",
            get(library_intelligence_handler),
        )
        .route(
            "/api/library/reanalyze",
            post(reanalyze_library_root_handler),
        )
        .route("/api/library/health", get(library_health_handler))
        .route(
            "/api/library/health/scan",
            post(start_library_health_scan_handler),
        )
        .route(
            "/api/library/health/scan/:id",
            post(rescan_library_health_issue_handler),
        )
        .route(
            "/api/library/health/issues",
            get(get_library_health_issues_handler),
        )
        .route("/api/fs/browse", get(fs_browse_handler))
        .route("/api/fs/recommendations", get(fs_recommendations_handler))
        .route("/api/fs/preview", post(fs_preview_handler))
        .route("/api/telemetry/payload", get(telemetry_payload_handler))
        .route("/metrics", get(metrics_handler))
        // Setup Routes
        .route("/api/setup/status", get(setup_status_handler))
        .route("/api/setup/complete", post(setup_complete_handler))
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route(
            "/api/ui/preferences",
            get(get_preferences_handler).post(update_preferences_handler),
        )
        // Static Asset Routes
        .route("/", get(index_handler))
        .route("/*file", get(static_handler))
        .layer(axum_middleware::from_fn(
            middleware::security_headers_middleware,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::rate_limit_middleware,
        ))
        .with_state(state)
}

// Helper functions used by multiple modules

pub(crate) async fn refresh_file_watcher(state: &AppState) {
    let config = state.config.read().await.clone();
    if let Err(e) = crate::system::watcher::refresh_from_sources(
        state.file_watcher.as_ref(),
        state.db.as_ref(),
        &config,
        state.setup_required.load(Ordering::Relaxed),
    )
    .await
    {
        error!("Failed to update file watcher: {}", e);
    }
}

pub(crate) async fn replace_runtime_hardware(
    state: &AppState,
    hardware_info: HardwareInfo,
    probe_log: HardwareProbeLog,
) {
    state.hardware_state.replace(Some(hardware_info)).await;
    *state.hardware_probe_log.write().await = probe_log;
}

pub(crate) fn config_write_blocked_response(config_path: &FsPath) -> Response {
    (
        StatusCode::CONFLICT,
        format!(
            "Configuration updates are disabled (ALCHEMIST_CONFIG_MUTABLE=false). \
Set ALCHEMIST_CONFIG_MUTABLE=true and ensure {:?} is writable.",
            config_path
        ),
    )
        .into_response()
}

pub(crate) fn config_save_error_to_response(config_path: &FsPath, err: &anyhow::Error) -> Response {
    if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
        let read_only = io_err
            .to_string()
            .to_ascii_lowercase()
            .contains("read-only");
        if io_err.kind() == std::io::ErrorKind::PermissionDenied || read_only {
            return (
                StatusCode::CONFLICT,
                format!(
                    "Configuration file {:?} is not writable: {}",
                    config_path, io_err
                ),
            )
                .into_response();
        }
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Failed to save config at {:?}: {}", config_path, err),
    )
        .into_response()
}

pub(crate) async fn save_config_or_response(
    state: &AppState,
    config: &Config,
) -> std::result::Result<(), Box<Response>> {
    if !state.config_mutable {
        return Err(Box::new(config_write_blocked_response(&state.config_path)));
    }

    if let Some(parent) = state.config_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                return Err(config_save_error_to_response(
                    &state.config_path,
                    &anyhow::Error::new(err),
                )
                .into());
            }
        }
    }

    if let Err(err) = crate::settings::save_config_and_project(
        state.db.as_ref(),
        state.config_path.as_path(),
        config,
    )
    .await
    {
        return Err(config_save_error_to_response(
            &state.config_path,
            &anyhow::Error::msg(err.to_string()),
        )
        .into());
    }

    Ok(())
}

pub(crate) fn config_read_error_response(context: &str, err: &AlchemistError) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Failed to {context}: {err}"),
    )
        .into_response()
}

pub(crate) fn hardware_error_response(err: &AlchemistError) -> Response {
    let status = match err {
        AlchemistError::Config(_) | AlchemistError::Hardware(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, err.to_string()).into_response()
}

pub(crate) fn validate_transcode_payload(
    payload: &settings::TranscodeSettingsPayload,
) -> std::result::Result<(), &'static str> {
    if payload.concurrent_jobs == 0 {
        return Err("concurrent_jobs must be > 0");
    }
    if !(0.0..=1.0).contains(&payload.size_reduction_threshold) {
        return Err("size_reduction_threshold must be 0.0-1.0");
    }
    if payload.min_bpp_threshold < 0.0 {
        return Err("min_bpp_threshold must be >= 0.0");
    }
    if payload.threads > 512 {
        return Err("threads must be <= 512");
    }
    if !(50.0..=1000.0).contains(&payload.tonemap_peak) {
        return Err("tonemap_peak must be between 50 and 1000");
    }
    if !(0.0..=1.0).contains(&payload.tonemap_desat) {
        return Err("tonemap_desat must be between 0.0 and 1.0");
    }
    Ok(())
}

pub(crate) fn canonicalize_directory_path(
    value: &str,
    field_name: &str,
) -> std::result::Result<PathBuf, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field_name} must not be empty"));
    }
    if trimmed.contains('\0') {
        return Err(format!("{field_name} must not contain null bytes"));
    }

    let path = PathBuf::from(trimmed);
    if !path.is_dir() {
        return Err(format!("{field_name} must be an existing directory"));
    }

    fs::canonicalize(&path).map_err(|_| format!("{field_name} must be canonicalizable"))
}

pub(crate) fn normalize_optional_directory(
    value: Option<&str>,
    field_name: &str,
) -> std::result::Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    canonicalize_directory_path(trimmed, field_name)
        .map(|path| Some(path.to_string_lossy().to_string()))
}

pub(crate) fn normalize_optional_path(
    value: Option<&str>,
    field_name: &str,
) -> std::result::Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.contains('\0') {
        return Err(format!("{field_name} must not contain null bytes"));
    }

    if cfg!(target_os = "linux") {
        let path = PathBuf::from(trimmed);
        if !path.exists() {
            return Err(format!("{field_name} must exist"));
        }
        return fs::canonicalize(path)
            .map(|path| Some(path.to_string_lossy().to_string()))
            .map_err(|_| format!("{field_name} must be canonicalizable"));
    }

    Ok(Some(trimmed.to_string()))
}

pub(crate) fn is_row_not_found(err: &AlchemistError) -> bool {
    matches!(err, AlchemistError::Database(sqlx::Error::RowNotFound))
}

pub(crate) fn has_path_separator(value: &str) -> bool {
    value.chars().any(|c| c == '/' || c == '\\')
}

pub(crate) fn normalize_schedule_time(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(format!("{:02}:{:02}", hour, minute))
}

pub(crate) async fn validate_notification_url(
    raw: &str,
    allow_local: bool,
) -> std::result::Result<(), String> {
    let url =
        reqwest::Url::parse(raw).map_err(|_| "endpoint_url must be a valid URL".to_string())?;
    match url.scheme() {
        "http" | "https" => {}
        _ => return Err("endpoint_url must use http or https".to_string()),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("endpoint_url must not contain embedded credentials".to_string());
    }
    if url.fragment().is_some() {
        return Err("endpoint_url must not include a URL fragment".to_string());
    }

    let host = url
        .host_str()
        .ok_or_else(|| "endpoint_url must include a host".to_string())?;

    if !allow_local && host.eq_ignore_ascii_case("localhost") {
        return Err("endpoint_url host is not allowed".to_string());
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if !allow_local && is_private_ip(ip) {
            return Err("endpoint_url host is not allowed".to_string());
        }
    } else {
        let port = url
            .port_or_known_default()
            .ok_or_else(|| "endpoint_url must include a port".to_string())?;
        let host_port = format!("{}:{}", host, port);
        let mut resolved = false;
        let addrs = tokio::time::timeout(Duration::from_secs(3), lookup_host(host_port))
            .await
            .map_err(|_| "endpoint_url host resolution timed out".to_string())?
            .map_err(|_| "endpoint_url host could not be resolved".to_string())?;
        for addr in addrs {
            resolved = true;
            if !allow_local && is_private_ip(addr.ip()) {
                return Err("endpoint_url host is not allowed".to_string());
            }
        }
        if !resolved {
            return Err("endpoint_url host could not be resolved".to_string());
        }
    }

    Ok(())
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_broadcast()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.is_multicast()
                || v6.is_unspecified()
        }
    }
}

fn sanitize_asset_path(raw: &str) -> Option<String> {
    let normalized = raw.replace('\\', "/");
    let mut segments = Vec::new();

    for segment in normalized.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return None;
        }
        segments.push(segment);
    }

    if segments.is_empty() {
        Some("index.html".to_string())
    } else {
        Some(segments.join("/"))
    }
}

// Static asset handlers

async fn index_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    static_handler(State(state), Uri::from_static("/index.html")).await
}

async fn static_handler(State(_state): State<Arc<AppState>>, uri: Uri) -> impl IntoResponse {
    let raw_path = uri.path().trim_start_matches('/');
    let path = match sanitize_asset_path(raw_path) {
        Some(path) => path,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    if let Some(content) = load_static_asset(&path) {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.as_ref())], content).into_response();
    }

    // Attempt to serve index.html for directory paths (e.g. /jobs -> jobs/index.html)
    if !path.contains('.') {
        let index_path = format!("{}/index.html", path);
        if let Some(content) = load_static_asset(&index_path) {
            let mime = mime_guess::from_path("index.html").first_or_octet_stream();
            return ([(header::CONTENT_TYPE, mime.as_ref())], content).into_response();
        }
    }

    if path == "index.html" {
        const MISSING_WEB_BUILD_PAGE: &str = r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Alchemist UI Not Built</title></head>
<body>
<h1>Alchemist UI is not built</h1>
<p>The backend is running, but frontend assets are missing.</p>
<p>Run <code>cd web && bun install && bun run build</code>, then restart Alchemist.</p>
</body>
</html>"#;
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            MISSING_WEB_BUILD_PAGE,
        )
            .into_response();
    }

    if !path.contains('.') {
        if let Some(content) = load_static_asset("404.html") {
            let mime = mime_guess::from_path("404.html").first_or_octet_stream();
            return (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, mime.as_ref())],
                content,
            )
                .into_response();
        }
    }

    // Default fallback to 404 for missing files.
    StatusCode::NOT_FOUND.into_response()
}
