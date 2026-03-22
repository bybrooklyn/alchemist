use crate::config::Config;
use crate::db::{AlchemistEvent, Db, JobState};
use crate::error::{AlchemistError, Result};
use crate::system::hardware::{HardwareProbeLog, HardwareState};
use crate::Agent;
use crate::Transcoder;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{ConnectInfo, Path, Query, Request, State},
    http::{header, HeaderMap, StatusCode, Uri},
    middleware::{self, Next},
    response::{
        sse::{Event as AxumEvent, Sse},
        IntoResponse, Response,
    },
    routing::{delete, get, post},
    Router,
};
use chrono::Utc;
use futures::{
    stream::{self, Stream},
    FutureExt, StreamExt,
};
use rand::rngs::OsRng;
use rand::Rng;
use reqwest::Url;
#[cfg(feature = "embed-web")]
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path as FsPath, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::net::lookup_host;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

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

pub struct AppState {
    pub db: Arc<Db>,
    pub config: Arc<RwLock<Config>>,
    pub agent: Arc<Agent>,
    pub transcoder: Arc<Transcoder>,
    pub scheduler: crate::scheduler::SchedulerHandle,
    pub tx: broadcast::Sender<AlchemistEvent>,
    pub setup_required: Arc<AtomicBool>,
    pub start_time: Instant,
    pub telemetry_runtime_id: String,
    pub notification_manager: Arc<crate::notifications::NotificationManager>,
    pub sys: std::sync::Mutex<sysinfo::System>,
    pub file_watcher: Arc<crate::system::watcher::FileWatcher>,
    pub library_scanner: Arc<crate::system::scanner::LibraryScanner>,
    pub config_path: PathBuf,
    pub config_mutable: bool,
    pub hardware_state: HardwareState,
    pub hardware_probe_log: Arc<tokio::sync::RwLock<HardwareProbeLog>>,
    pub resources_cache: Arc<tokio::sync::Mutex<Option<(serde_json::Value, std::time::Instant)>>>,
    login_rate_limiter: Mutex<HashMap<IpAddr, RateLimitEntry>>,
    global_rate_limiter: Mutex<HashMap<IpAddr, RateLimitEntry>>,
}

pub struct RunServerArgs {
    pub db: Arc<Db>,
    pub config: Arc<RwLock<Config>>,
    pub agent: Arc<Agent>,
    pub transcoder: Arc<Transcoder>,
    pub scheduler: crate::scheduler::SchedulerHandle,
    pub tx: broadcast::Sender<AlchemistEvent>,
    pub setup_required: bool,
    pub config_path: PathBuf,
    pub config_mutable: bool,
    pub hardware_state: HardwareState,
    pub hardware_probe_log: Arc<tokio::sync::RwLock<HardwareProbeLog>>,
    pub notification_manager: Arc<crate::notifications::NotificationManager>,
    pub file_watcher: Arc<crate::system::watcher::FileWatcher>,
}

struct RateLimitEntry {
    tokens: f64,
    last_refill: Instant,
}

const LOGIN_RATE_LIMIT_CAPACITY: f64 = 10.0;
const LOGIN_RATE_LIMIT_REFILL_PER_SEC: f64 = 1.0;
const GLOBAL_RATE_LIMIT_CAPACITY: f64 = 120.0;
const GLOBAL_RATE_LIMIT_REFILL_PER_SEC: f64 = 60.0;

pub async fn run_server(args: RunServerArgs) -> Result<()> {
    let RunServerArgs {
        db,
        config,
        agent,
        transcoder,
        scheduler,
        tx,
        setup_required,
        config_path,
        config_mutable,
        hardware_state,
        hardware_probe_log,
        notification_manager,
        file_watcher,
    } = args;
    // Initialize sysinfo
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let library_scanner = Arc::new(crate::system::scanner::LibraryScanner::new(
        db.clone(),
        config.clone(),
    ));

    let state = Arc::new(AppState {
        db,
        config,
        agent,
        transcoder,
        scheduler,
        tx,
        setup_required: Arc::new(AtomicBool::new(setup_required)),
        start_time: std::time::Instant::now(),
        telemetry_runtime_id: Uuid::new_v4().to_string(),
        notification_manager,
        sys: std::sync::Mutex::new(sys),
        file_watcher,
        library_scanner,
        config_path,
        config_mutable,
        hardware_state,
        hardware_probe_log,
        resources_cache: Arc::new(tokio::sync::Mutex::new(None)),
        login_rate_limiter: Mutex::new(HashMap::new()),
        global_rate_limiter: Mutex::new(HashMap::new()),
    });

    let app = app_router(state);

    let addr = "0.0.0.0:3000";
    info!("listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(AlchemistError::Io)?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| AlchemistError::Unknown(format!("Server error: {}", e)))?;

    Ok(())
}

fn app_router(state: Arc<AppState>) -> Router {
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
        // Canonical job list endpoint.
        .route("/api/jobs", get(jobs_table_handler))
        .route("/api/jobs/table", get(jobs_table_handler))
        .route("/api/jobs/batch", post(batch_jobs_handler))
        .route("/api/logs/history", get(logs_history_handler))
        .route("/api/logs", delete(clear_logs_handler))
        .route("/api/jobs/restart-failed", post(restart_failed_handler))
        .route("/api/jobs/clear-completed", post(clear_completed_handler))
        .route("/api/jobs/:id/cancel", post(cancel_job_handler))
        .route("/api/jobs/:id/priority", post(update_job_priority_handler))
        .route("/api/jobs/:id/restart", post(restart_job_handler))
        .route("/api/jobs/:id/delete", post(delete_job_handler))
        .route("/api/jobs/:id/details", get(get_job_detail_handler))
        .route("/api/events", get(sse_handler))
        .route("/api/engine/pause", post(pause_engine_handler))
        .route("/api/engine/resume", post(resume_engine_handler))
        .route("/api/engine/drain", post(drain_engine_handler))
        .route("/api/engine/stop-drain", post(stop_drain_handler))
        .route(
            "/api/engine/mode",
            get(get_engine_mode_handler).post(set_engine_mode_handler),
        )
        .route("/api/engine/status", get(engine_status_handler))
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
        .route(
            "/api/settings/watch-dirs/:id",
            delete(remove_watch_dir_handler),
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
            get(get_notifications_handler).post(add_notification_handler),
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
        .route("/api/system/hardware", get(get_hardware_info_handler))
        .route(
            "/api/system/hardware/probe-log",
            get(get_hardware_probe_log_handler),
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .with_state(state)
}

async fn refresh_file_watcher(state: &AppState) {
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

async fn replace_runtime_hardware(
    state: &AppState,
    hardware_info: crate::system::hardware::HardwareInfo,
    probe_log: HardwareProbeLog,
) {
    state.hardware_state.replace(Some(hardware_info)).await;
    *state.hardware_probe_log.write().await = probe_log;
}

async fn setup_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(serde_json::json!({
        "setup_required": state.setup_required.load(Ordering::Relaxed),
        "enable_telemetry": config.system.enable_telemetry,
        "config_mutable": state.config_mutable
    }))
}

fn config_write_blocked_response(config_path: &FsPath) -> Response {
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

fn config_save_error_to_response(config_path: &FsPath, err: &anyhow::Error) -> Response {
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

async fn save_config_or_response(
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

fn config_read_error_response(context: &str, err: &AlchemistError) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Failed to {context}: {err}"),
    )
        .into_response()
}

fn hardware_error_response(err: &AlchemistError) -> Response {
    let status = match err {
        AlchemistError::Config(_) | AlchemistError::Hardware(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, err.to_string()).into_response()
}

fn validate_transcode_payload(
    payload: &TranscodeSettingsPayload,
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

fn canonicalize_directory_path(
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

fn normalize_setup_directories(directories: &[String]) -> std::result::Result<Vec<String>, String> {
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

fn normalize_optional_directory(
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

fn normalize_optional_path(
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

fn is_row_not_found(err: &AlchemistError) -> bool {
    matches!(err, AlchemistError::Database(sqlx::Error::RowNotFound))
}

#[derive(Serialize)]
struct BlockedJob {
    id: i64,
    status: JobState,
}

#[derive(Serialize)]
struct BlockedJobsResponse {
    message: String,
    blocked: Vec<BlockedJob>,
}

fn blocked_jobs_response(message: impl Into<String>, blocked: &[crate::db::Job]) -> Response {
    let payload = BlockedJobsResponse {
        message: message.into(),
        blocked: blocked
            .iter()
            .map(|job| BlockedJob {
                id: job.id,
                status: job.status,
            })
            .collect(),
    };
    (StatusCode::CONFLICT, axum::Json(payload)).into_response()
}

async fn request_job_cancel(state: &AppState, job: &crate::db::Job) -> Result<bool> {
    match job.status {
        JobState::Queued => {
            state
                .db
                .update_job_status(job.id, JobState::Cancelled)
                .await?;
            Ok(true)
        }
        JobState::Analyzing | JobState::Resuming => {
            if !state.transcoder.cancel_job(job.id) {
                return Ok(false);
            }
            state
                .db
                .update_job_status(job.id, JobState::Cancelled)
                .await?;
            Ok(true)
        }
        JobState::Encoding | JobState::Remuxing => Ok(state.transcoder.cancel_job(job.id)),
        _ => Ok(false),
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct TranscodeSettingsPayload {
    concurrent_jobs: usize,
    size_reduction_threshold: f64,
    min_bpp_threshold: f64,
    min_file_size_mb: u64,
    output_codec: crate::config::OutputCodec,
    quality_profile: crate::config::QualityProfile,
    #[serde(default)]
    threads: usize,
    #[serde(default = "crate::config::default_allow_fallback")]
    allow_fallback: bool,
    #[serde(default)]
    hdr_mode: crate::config::HdrMode,
    #[serde(default)]
    tonemap_algorithm: crate::config::TonemapAlgorithm,
    #[serde(default = "crate::config::default_tonemap_peak")]
    tonemap_peak: f32,
    #[serde(default = "crate::config::default_tonemap_desat")]
    tonemap_desat: f32,
    #[serde(default)]
    subtitle_mode: crate::config::SubtitleMode,
    #[serde(default)]
    stream_rules: crate::config::StreamRules,
}

async fn get_transcode_settings_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
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

async fn update_transcode_settings_handler(
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

#[derive(serde::Serialize, serde::Deserialize)]
struct HardwareSettingsPayload {
    allow_cpu_fallback: bool,
    allow_cpu_encoding: bool,
    cpu_preset: String,
    preferred_vendor: Option<String>,
    #[serde(default)]
    device_path: Option<String>,
}

async fn get_hardware_settings_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(HardwareSettingsPayload {
        allow_cpu_fallback: config.hardware.allow_cpu_fallback,
        allow_cpu_encoding: config.hardware.allow_cpu_encoding,
        cpu_preset: config.hardware.cpu_preset.to_string(),
        preferred_vendor: config.hardware.preferred_vendor.clone(),
        device_path: config.hardware.device_path.clone(),
    })
}

async fn update_hardware_settings_handler(
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

#[derive(serde::Serialize, serde::Deserialize)]
struct SystemSettingsPayload {
    monitoring_poll_interval: f64,
    enable_telemetry: bool,
    #[serde(default)]
    watch_enabled: bool,
}

async fn get_system_settings_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(SystemSettingsPayload {
        monitoring_poll_interval: config.system.monitoring_poll_interval,
        enable_telemetry: config.system.enable_telemetry,
        watch_enabled: config.scanner.watch_enabled,
    })
}

async fn update_system_settings_handler(
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

    let mut next_config = state.config.read().await.clone();
    next_config.system.monitoring_poll_interval = payload.monitoring_poll_interval;
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

async fn get_settings_bundle_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await.clone();
    axum::Json(crate::settings::bundle_response(config)).into_response()
}

#[derive(serde::Deserialize)]
struct SettingPreferencePayload {
    key: String,
    value: String,
}

#[derive(serde::Serialize)]
struct SettingPreferenceResponse {
    key: String,
    value: String,
}

async fn set_setting_preference_handler(
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

async fn get_setting_preference_handler(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match state.db.get_preference(key.as_str()).await {
        Ok(Some(value)) => axum::Json(SettingPreferenceResponse { key, value }).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn update_settings_bundle_handler(
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

#[derive(serde::Deserialize)]
struct RawConfigPayload {
    raw_toml: String,
}

async fn get_settings_config_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let raw_toml = match crate::settings::load_raw_config(state.config_path.as_path()) {
        Ok(raw_toml) => raw_toml,
        Err(err) => return config_read_error_response("load raw config", &err),
    };
    let normalized = state.config.read().await.clone();
    axum::Json(crate::settings::config_response(raw_toml, normalized)).into_response()
}

async fn update_settings_config_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<RawConfigPayload>,
) -> impl IntoResponse {
    let config = match crate::settings::parse_raw_config(&payload.raw_toml) {
        Ok(config) => config,
        Err(err) => return hardware_error_response(&err),
    };

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

#[derive(serde::Deserialize)]
struct SetupConfig {
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

fn default_setup_min_bpp() -> f64 {
    0.1
}

fn default_setup_true() -> bool {
    true
}

fn default_setup_telemetry() -> bool {
    false
}

async fn setup_complete_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SetupConfig>,
) -> impl IntoResponse {
    if !state.setup_required.load(Ordering::Relaxed) {
        return (StatusCode::FORBIDDEN, "Setup already completed").into_response();
    }

    let username = payload.username.trim();
    if username.len() < 3 {
        return (
            StatusCode::BAD_REQUEST,
            "username must be at least 3 characters",
        )
            .into_response();
    }
    if payload.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            "password must be at least 8 characters",
        )
            .into_response();
    }
    if payload.settings.is_none() && payload.concurrent_jobs == 0 {
        return (StatusCode::BAD_REQUEST, "concurrent_jobs must be > 0").into_response();
    }
    if payload.settings.is_none() && !(0.0..=1.0).contains(&payload.size_reduction_threshold) {
        return (
            StatusCode::BAD_REQUEST,
            "size_reduction_threshold must be 0.0-1.0",
        )
            .into_response();
    }
    if payload.settings.is_none() && payload.min_bpp_threshold < 0.0 {
        return (StatusCode::BAD_REQUEST, "min_bpp_threshold must be >= 0.0").into_response();
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
                    return (
                        StatusCode::BAD_REQUEST,
                        format!(
                            "Setup configuration is invalid: {}. \
                                 Please go back and check your settings.",
                            err
                        ),
                    )
                        .into_response();
                }
            };
            settings.scanner.directories =
                match normalize_setup_directories(&settings.scanner.directories) {
                    Ok(paths) => paths,
                    Err(msg) => return (StatusCode::BAD_REQUEST, msg).into_response(),
                };
            settings
        }
        None => {
            let setup_directories = match normalize_setup_directories(&payload.directories) {
                Ok(paths) => paths,
                Err(msg) => return (StatusCode::BAD_REQUEST, msg).into_response(),
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
        return (
            StatusCode::BAD_REQUEST,
            "At least one library directory must be configured.",
        )
            .into_response();
    }

    if next_config.transcode.concurrent_jobs == 0 {
        return (
            StatusCode::BAD_REQUEST,
            "Concurrent jobs must be at least 1.",
        )
            .into_response();
    }

    if let Err(e) = next_config.validate() {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
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
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = match argon2.hash_password(payload.password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Hashing failed: {}", e),
            )
                .into_response()
        }
    };

    let user_id = match state.db.create_user(username, &password_hash).await {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create user: {}", e),
            )
                .into_response()
        }
    };

    let token: String = OsRng
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    let expires_at = Utc::now() + chrono::Duration::days(30);

    if let Err(e) = state.db.create_session(user_id, &token, expires_at).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create session: {}", e),
        )
            .into_response();
    }

    // Update Setup State (Hot Reload)
    state.setup_required.store(false, Ordering::Relaxed);
    state.agent.set_manual_override(true);
    *state.agent.engine_mode.write().await = runtime_engine_mode;
    state
        .agent
        .set_concurrent_jobs(runtime_concurrent_jobs)
        .await;
    replace_runtime_hardware(state.as_ref(), hardware_info, probe_log).await;
    refresh_file_watcher(&state).await;

    // Start Scan (optional, but good for UX)
    // Start Scan (optional, but good for UX)
    // Use library_scanner so the UI can track progress via /api/scan/status
    let scanner = state.library_scanner.clone();
    tokio::spawn(async move {
        if let Err(e) = scanner.start_scan().await {
            error!("Background initial scan failed: {}", e);
        }
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

#[derive(serde::Deserialize, serde::Serialize)]
struct UiPreferences {
    active_theme_id: Option<String>,
}

async fn get_preferences_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(UiPreferences {
        active_theme_id: config.appearance.active_theme_id.clone(),
    })
    .into_response()
}

async fn update_preferences_handler(
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

async fn index_handler() -> impl IntoResponse {
    static_handler(Uri::from_static("/index.html")).await
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
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

struct StatsData {
    total: i64,
    completed: i64,
    active: i64,
    failed: i64,
    concurrent_limit: usize,
}

async fn get_stats_data(db: &Db, concurrent_limit: usize) -> Result<StatsData> {
    let s = db.get_stats().await?;
    let total = s
        .as_object()
        .map(|m| m.values().filter_map(|v| v.as_i64()).sum::<i64>())
        .unwrap_or(0);
    let completed = s.get("completed").and_then(|v| v.as_i64()).unwrap_or(0);
    let active = s
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| {
                    ["encoding", "analyzing", "remuxing", "resuming"].contains(&k.as_str())
                })
                .map(|(_, v)| v.as_i64().unwrap_or(0))
                .sum::<i64>()
        })
        .unwrap_or(0);
    let failed = s.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);

    Ok(StatsData {
        total,
        completed,
        active,
        failed,
        concurrent_limit,
    })
}

async fn stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match get_stats_data(&state.db, state.agent.concurrent_jobs_limit()).await {
        Ok(stats) => axum::Json(serde_json::json!({
            "total": stats.total,
            "completed": stats.completed,
            "active": stats.active,
            "failed": stats.failed,
            "concurrent_limit": stats.concurrent_limit
        }))
        .into_response(),
        Err(err) => config_read_error_response("load job stats", &err),
    }
}

async fn aggregated_stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_aggregated_stats().await {
        Ok(stats) => {
            let savings = stats.total_input_size - stats.total_output_size;
            axum::Json(serde_json::json!({
                "total_input_bytes": stats.total_input_size,
                "total_output_bytes": stats.total_output_size,
                "total_savings_bytes": savings,
                "total_time_seconds": stats.total_encode_time_seconds,
                "total_jobs": stats.completed_jobs,
                "avg_vmaf": stats.avg_vmaf.unwrap_or(0.0)
            }))
            .into_response()
        }
        Err(err) => config_read_error_response("load aggregated stats", &err),
    }
}

async fn daily_stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_daily_stats(30).await {
        Ok(stats) => axum::Json(serde_json::json!(stats)).into_response(),
        Err(err) => config_read_error_response("load daily stats", &err),
    }
}

async fn detailed_stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_detailed_encode_stats(50).await {
        Ok(stats) => axum::Json(serde_json::json!(stats)).into_response(),
        Err(err) => config_read_error_response("load detailed stats", &err),
    }
}

async fn savings_summary_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_savings_summary().await {
        Ok(summary) => axum::Json(summary).into_response(),
        Err(err) => config_read_error_response("load storage savings summary", &err),
    }
}

async fn scan_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    let mut dirs: Vec<std::path::PathBuf> = config
        .scanner
        .directories
        .iter()
        .map(std::path::PathBuf::from)
        .collect();
    drop(config);

    if let Ok(watch_dirs) = state.db.get_watch_dirs().await {
        for wd in watch_dirs {
            dirs.push(std::path::PathBuf::from(wd.path));
        }
    }

    let _ = state.agent.scan_and_enqueue(dirs).await;
    StatusCode::OK
}

async fn cancel_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.get_job_by_id(id).await {
        Ok(Some(job)) => match request_job_cancel(&state, &job).await {
            Ok(_) => StatusCode::OK.into_response(),
            Err(e) if is_row_not_found(&e) => StatusCode::NOT_FOUND.into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn restart_failed_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.restart_failed_jobs().await {
        Ok(count) => {
            let message = if count == 0 {
                "No failed or cancelled jobs were waiting to be retried.".to_string()
            } else if count == 1 {
                "Queued 1 failed or cancelled job for retry.".to_string()
            } else {
                format!("Queued {count} failed or cancelled jobs for retry.")
            };
            axum::Json(serde_json::json!({ "count": count, "message": message })).into_response()
        }
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn clear_completed_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.clear_completed_jobs().await {
        Ok(count) => {
            let message = if count == 0 {
                "No completed jobs were waiting to be cleared.".to_string()
            } else if count == 1 {
                "Cleared 1 completed job from the queue. Historical stats were preserved."
                    .to_string()
            } else {
                format!(
                    "Cleared {count} completed jobs from the queue. Historical stats were preserved."
                )
            };
            axum::Json(serde_json::json!({ "count": count, "message": message })).into_response()
        }
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn pause_engine_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.agent.pause();
    axum::Json(serde_json::json!({ "status": "paused" }))
}

async fn resume_engine_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.agent.resume();
    axum::Json(serde_json::json!({ "status": "running" }))
}

async fn drain_engine_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.agent.drain();
    axum::Json(serde_json::json!({ "status": "draining" }))
}

async fn stop_drain_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.agent.stop_drain();
    axum::Json(serde_json::json!({ "status": "running" }))
}

async fn engine_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": if state.agent.is_draining() {
            "draining"
        } else if state.agent.is_paused() {
            "paused"
        } else {
            "running"
        },
        "manual_paused": state.agent.is_manual_paused(),
        "scheduler_paused": state.agent.is_scheduler_paused(),
        "draining": state.agent.is_draining(),
        "mode": state.agent.current_mode().await.as_str(),
        "concurrent_limit": state.agent.concurrent_jobs_limit(),
        "is_manual_override": state.agent.is_manual_override(),
    }))
}

async fn get_engine_mode_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    let cpu_count = {
        let sys = state.sys.lock().unwrap_or_else(|e| e.into_inner());
        sys.cpus().len()
    };
    drop(config);
    axum::Json(serde_json::json!({
        "mode": state.agent.current_mode().await.as_str(),
        "is_manual_override": state.agent.is_manual_override(),
        "concurrent_limit": state.agent.concurrent_jobs_limit(),
        "cpu_count": cpu_count,
        "computed_limits": {
            "background": crate::config::EngineMode::Background
                .concurrent_jobs_for_cpu_count(cpu_count),
            "balanced": crate::config::EngineMode::Balanced
                .concurrent_jobs_for_cpu_count(cpu_count),
            "throughput": crate::config::EngineMode::Throughput
                .concurrent_jobs_for_cpu_count(cpu_count),
        }
    }))
}

#[derive(Deserialize)]
struct SetEngineModePayload {
    mode: crate::config::EngineMode,
    // Optional manual override of concurrent jobs.
    // If provided, bypasses mode auto-computation.
    concurrent_jobs_override: Option<usize>,
    // Optional manual thread override (0 = auto).
    threads_override: Option<usize>,
}

async fn set_engine_mode_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SetEngineModePayload>,
) -> impl IntoResponse {
    let cpu_count = {
        let sys = state.sys.lock().unwrap_or_else(|e| e.into_inner());
        sys.cpus().len()
    };

    if let Some(override_jobs) = payload.concurrent_jobs_override {
        if override_jobs == 0 {
            return (
                StatusCode::BAD_REQUEST,
                "concurrent_jobs_override must be > 0",
            )
                .into_response();
        }
        state.agent.set_manual_override(true);
        state.agent.set_concurrent_jobs(override_jobs).await;
        *state.agent.engine_mode.write().await = payload.mode;
    } else {
        state.agent.apply_mode(payload.mode, cpu_count).await;
    }

    // Apply thread override to config if provided
    if let Some(threads) = payload.threads_override {
        let mut config = state.config.write().await;
        config.transcode.threads = threads;
    }

    // Persist mode to config
    {
        let mut config = state.config.write().await;
        config.system.engine_mode = payload.mode;
    }
    let config = state.config.read().await;
    if let Err(e) = save_config_or_response(&state, &config).await {
        return *e;
    }

    axum::Json(serde_json::json!({
        "status": "ok",
        "mode": payload.mode.as_str(),
        "concurrent_limit": state.agent.concurrent_jobs_limit(),
        "is_manual_override": state.agent.is_manual_override(),
    }))
    .into_response()
}

#[derive(Deserialize)]
struct FsBrowseQuery {
    path: Option<String>,
}

async fn fs_browse_handler(Query(query): Query<FsBrowseQuery>) -> impl IntoResponse {
    match crate::system::fs_browser::browse(query.path.as_deref()).await {
        Ok(response) => axum::Json(response).into_response(),
        Err(err) => config_read_error_response("browse server filesystem", &err),
    }
}

async fn fs_recommendations_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await.clone();
    match crate::system::fs_browser::recommendations(&config, state.db.as_ref()).await {
        Ok(response) => axum::Json(response).into_response(),
        Err(err) => config_read_error_response("load folder recommendations", &err),
    }
}

async fn fs_preview_handler(
    axum::Json(payload): axum::Json<crate::system::fs_browser::FsPreviewRequest>,
) -> impl IntoResponse {
    match crate::system::fs_browser::preview(payload).await {
        Ok(response) => axum::Json(response).into_response(),
        Err(err) => config_read_error_response("preview selected server folders", &err),
    }
}

async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed();
    let hours = uptime.as_secs() / 3600;
    let minutes = (uptime.as_secs() % 3600) / 60;
    let seconds = uptime.as_secs() % 60;

    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": format!("{}h {}m {}s", hours, minutes, seconds),
        "uptime_seconds": uptime.as_secs()
    }))
}

async fn ready_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Check if database is accessible
    let db_ok = state.db.get_stats().await.is_ok();

    if db_ok {
        (
            StatusCode::OK,
            axum::Json(serde_json::json!({ "ready": true })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({ "ready": false, "reason": "database unavailable" })),
        )
    }
}

async fn library_health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_health_summary().await {
        Ok(summary) => axum::Json(summary).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn get_library_health_issues_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_jobs_with_health_issues().await {
        Ok(jobs) => {
            let issues = jobs
                .into_iter()
                .map(|row| {
                    let (job, raw_health_issue) = row.into_parts();
                    let report = serde_json::from_str::<crate::media::health::HealthIssueReport>(
                        &raw_health_issue,
                    )
                    .unwrap_or_else(|_| {
                        crate::media::health::categorize_health_output(&raw_health_issue)
                    });
                    LibraryHealthIssueResponse { job, report }
                })
                .collect::<Vec<_>>();
            axum::Json(issues).into_response()
        }
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

#[derive(Serialize)]
struct LibraryHealthIssueResponse {
    job: crate::db::Job,
    report: crate::media::health::HealthIssueReport,
}

async fn run_library_health_scan(db: Arc<Db>) {
    let result = std::panic::AssertUnwindSafe({
        let db = db.clone();
        async move {
            let created_run_id = match db.create_health_scan_run().await {
                Ok(id) => id,
                Err(err) => {
                    error!("Failed to create library health scan run: {}", err);
                    return;
                }
            };

            let jobs = match db.get_jobs_needing_health_check().await {
                Ok(jobs) => jobs,
                Err(err) => {
                    error!("Failed to load jobs for library health scan: {}", err);
                    let _ = db.complete_health_scan_run(created_run_id, 0, 0).await;
                    return;
                }
            };

            let counters = Arc::new(Mutex::new((0_i64, 0_i64)));
            let semaphore = Arc::new(tokio::sync::Semaphore::new(2));

            stream::iter(jobs)
                .for_each_concurrent(None, {
                    let db = db.clone();
                    let counters = counters.clone();
                    let semaphore = semaphore.clone();

                    move |job| {
                        let db = db.clone();
                        let counters = counters.clone();
                        let semaphore = semaphore.clone();
                        async move {
                            let Ok(permit) = semaphore.acquire_owned().await else {
                                error!("Library health scan semaphore closed unexpectedly");
                                return;
                            };
                            let _permit = permit;

                            match crate::media::health::HealthChecker::check_file(FsPath::new(
                                &job.output_path,
                            ))
                            .await
                            {
                                Ok(issues) => {
                                    if let Err(err) =
                                        db.record_health_check(job.id, issues.as_ref()).await
                                    {
                                        error!(
                                            "Failed to record library health result for job {}: {}",
                                            job.id, err
                                        );
                                        return;
                                    }

                                    let mut guard = counters.lock().await;
                                    guard.0 += 1;
                                    if issues.is_some() {
                                        guard.1 += 1;
                                    }
                                }
                                Err(err) => {
                                    error!(
                                        "Library health check was inconclusive for job {} ({}): {}",
                                        job.id, job.output_path, err
                                    );
                                }
                            }
                        }
                    }
                })
                .await;

            let (files_checked, issues_found) = *counters.lock().await;
            if let Err(err) = db
                .complete_health_scan_run(created_run_id, files_checked, issues_found)
                .await
            {
                error!(
                    "Failed to complete library health scan run {}: {}",
                    created_run_id, err
                );
            }
        }
    })
    .catch_unwind()
    .await;

    if result.is_err() {
        error!("Library health scan panicked");
    }
}

async fn start_library_health_scan_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.clone();
    tokio::spawn(async move {
        run_library_health_scan(db).await;
    });

    (
        StatusCode::ACCEPTED,
        axum::Json(serde_json::json!({ "status": "accepted" })),
    )
        .into_response()
}

async fn rescan_library_health_issue_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let job = match state.db.get_job_by_id(id).await {
        Ok(Some(job)) => job,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    match crate::media::health::HealthChecker::check_file(FsPath::new(&job.output_path)).await {
        Ok(issue) => {
            if let Err(err) = state.db.record_health_check(job.id, issue.as_ref()).await {
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
            }
            axum::Json(serde_json::json!({
                "job_id": job.id,
                "issue_found": issue.is_some(),
            }))
            .into_response()
        }
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct LoginPayload {
    username: String,
    password: String,
}

async fn login_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::Json(payload): axum::Json<LoginPayload>,
) -> impl IntoResponse {
    if !allow_login_attempt(&state, addr.ip()).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Too many requests").into_response();
    }

    let user = match state.db.get_user_by_username(&payload.username).await {
        Ok(Some(u)) => u,
        _ => return (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response(),
    };

    let parsed_hash = match PasswordHash::new(&user.password_hash) {
        Ok(h) => h,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Invalid hash format").into_response()
        }
    };

    if Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response();
    }

    // Create session
    let token: String = OsRng
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    let expires_at = Utc::now() + chrono::Duration::days(30);

    if let Err(e) = state.db.create_session(user.id, &token, expires_at).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create session: {}", e),
        )
            .into_response();
    }

    let cookie = build_session_cookie(&token);
    (
        [(header::SET_COOKIE, cookie)],
        axum::Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response()
}

async fn logout_handler(State(state): State<Arc<AppState>>, req: Request) -> impl IntoResponse {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|auth_str| auth_str.strip_prefix("Bearer ").map(str::to_string))
        .or_else(|| get_cookie_value(req.headers(), "alchemist_session"));

    if let Some(t) = token {
        let _ = state.db.delete_session(&t).await;
    }

    let cookie = build_clear_session_cookie();
    (
        [(header::SET_COOKIE, cookie)],
        axum::Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response()
}

async fn auth_middleware(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let path = req.uri().path();

    // 1. API Protection: Only lock down /api routes
    if path.starts_with("/api") {
        // Public API endpoints
        if path.starts_with("/api/setup")
            || path.starts_with("/api/auth/login")
            || path.starts_with("/api/auth/logout")
            || path == "/api/health"
            || path == "/api/ready"
        {
            return next.run(req).await;
        }

        if state.setup_required.load(Ordering::Relaxed) && path == "/api/system/hardware" {
            return next.run(req).await;
        }
        if state.setup_required.load(Ordering::Relaxed) && path.starts_with("/api/fs/") {
            return next.run(req).await;
        }
        if state.setup_required.load(Ordering::Relaxed) && path == "/api/settings/bundle" {
            return next.run(req).await;
        }

        // Protected API endpoints -> Require Token
        let mut token = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|auth_str| auth_str.strip_prefix("Bearer ").map(str::to_string));

        if token.is_none() {
            token = get_cookie_value(req.headers(), "alchemist_session");
        }

        if let Some(t) = token {
            if let Ok(Some(_session)) = state.db.get_session(&t).await {
                return next.run(req).await;
            }
        }

        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    // 2. Static Assets / Frontend Pages
    // Allow everything else. The frontend app (Layout.astro) handles client-side redirects
    // if the user isn't authenticated, and the backend API protects the actual data.
    next.run(req).await
}

async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    if !req.uri().path().starts_with("/api/") {
        return next.run(req).await;
    }

    let ip = request_ip(&req).unwrap_or(IpAddr::from([0, 0, 0, 0]));
    if !allow_global_request(&state, ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Too many requests").into_response();
    }
    next.run(req).await
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SseMessage {
    event_name: &'static str,
    data: String,
}

impl From<SseMessage> for AxumEvent {
    fn from(message: SseMessage) -> Self {
        AxumEvent::default()
            .event(message.event_name)
            .data(message.data)
    }
}

fn sse_message_for_event(event: &AlchemistEvent) -> SseMessage {
    match event {
        AlchemistEvent::Log {
            level,
            job_id,
            message,
        } => SseMessage {
            event_name: "log",
            data: serde_json::json!({
                "level": level,
                "job_id": job_id,
                "message": message
            })
            .to_string(),
        },
        AlchemistEvent::Progress {
            job_id,
            percentage,
            time,
        } => SseMessage {
            event_name: "progress",
            data: serde_json::json!({
                "job_id": job_id,
                "percentage": percentage,
                "time": time
            })
            .to_string(),
        },
        AlchemistEvent::JobStateChanged { job_id, status } => SseMessage {
            event_name: "status",
            data: serde_json::json!({
                "job_id": job_id,
                "status": status
            })
            .to_string(),
        },
        AlchemistEvent::Decision {
            job_id,
            action,
            reason,
        } => SseMessage {
            event_name: "decision",
            data: serde_json::json!({
                "job_id": job_id,
                "action": action,
                "reason": reason
            })
            .to_string(),
        },
    }
}

fn sse_lagged_message(skipped: u64) -> SseMessage {
    SseMessage {
        event_name: "lagged",
        data: serde_json::json!({ "skipped": skipped }).to_string(),
    }
}

fn sse_message_stream(
    rx: broadcast::Receiver<AlchemistEvent>,
) -> impl Stream<Item = std::result::Result<SseMessage, Infallible>> {
    stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(event) => Some((Ok(sse_message_for_event(&event)), rx)),
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!("SSE subscriber lagged; skipped {skipped} events");
                Some((Ok(sse_lagged_message(skipped)), rx))
            }
            Err(broadcast::error::RecvError::Closed) => None,
        }
    })
}

async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = std::result::Result<AxumEvent, Infallible>>> {
    let stream = sse_message_stream(state.tx.subscribe()).map(|message| match message {
        Ok(message) => Ok(message.into()),
        Err(never) => match never {},
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

// #[derive(serde::Serialize)]
// struct GpuInfo {
//     name: String,
//     utilization: f32,
//     memory_used_mb: u64,
// }

#[derive(serde::Serialize)]
struct SystemResources {
    cpu_percent: f32,
    memory_used_mb: u64,
    memory_total_mb: u64,
    memory_percent: f32,
    uptime_seconds: u64,
    active_jobs: i64,
    concurrent_limit: usize,
    cpu_count: usize,
    gpu_utilization: Option<f32>,
    gpu_memory_percent: Option<f32>,
}

async fn system_resources_handler(State(state): State<Arc<AppState>>) -> Response {
    let mut cache = state.resources_cache.lock().await;
    if let Some((value, cached_at)) = cache.as_ref() {
        if cached_at.elapsed() < Duration::from_millis(500) {
            return axum::Json(value.clone()).into_response();
        }
    }

    let (cpu_percent, memory_used_mb, memory_total_mb, memory_percent, cpu_count) = {
        let mut sys = match state.sys.lock() {
            Ok(sys) => sys,
            Err(e) => {
                error!("System monitor lock poisoned: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "System monitor unavailable",
                )
                    .into_response();
            }
        };
        sys.refresh_all();

        let cpu_percent =
            sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32;
        let cpu_count = sys.cpus().len();
        let memory_used_mb = (sys.used_memory() / 1024 / 1024) as u64;
        let memory_total_mb = (sys.total_memory() / 1024 / 1024) as u64;
        let memory_percent = if memory_total_mb > 0 {
            (memory_used_mb as f32 / memory_total_mb as f32) * 100.0
        } else {
            0.0
        };

        (
            cpu_percent,
            memory_used_mb,
            memory_total_mb,
            memory_percent,
            cpu_count,
        )
    };

    let uptime_seconds = state.start_time.elapsed().as_secs();
    let stats = match state.db.get_job_stats().await {
        Ok(stats) => stats,
        Err(err) => return config_read_error_response("load system resource stats", &err),
    };
    let (gpu_utilization, gpu_memory_percent) = tokio::task::spawn_blocking(query_gpu_utilization)
        .await
        .unwrap_or((None, None));

    let value = match serde_json::to_value(SystemResources {
        cpu_percent,
        memory_used_mb,
        memory_total_mb,
        memory_percent,
        uptime_seconds,
        active_jobs: stats.active,
        concurrent_limit: state.agent.concurrent_jobs_limit(),
        cpu_count,
        gpu_utilization,
        gpu_memory_percent,
    }) {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to serialize system resource payload: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to serialize system resource payload",
            )
                .into_response();
        }
    };

    *cache = Some((value.clone(), Instant::now()));
    axum::Json(value).into_response()
}

/// Query GPU utilization using nvidia-smi (NVIDIA) or other platform-specific tools
fn query_gpu_utilization() -> (Option<f32>, Option<f32>) {
    // Try nvidia-smi first
    if let Some(output) = run_command_with_timeout(
        "nvidia-smi",
        &[
            "--query-gpu=utilization.gpu,memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ],
        Duration::from_secs(2),
    ) {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Format: "45, 2048, 8192" (utilization %, memory used MB, memory total MB)
            let parts: Vec<&str> = stdout.trim().split(',').map(|s| s.trim()).collect();
            if parts.len() >= 3 {
                let util = parts[0].parse::<f32>().ok();
                let mem_used = parts[1].parse::<f32>().ok();
                let mem_total = parts[2].parse::<f32>().ok();
                let mem_percent = match (mem_used, mem_total) {
                    (Some(used), Some(total)) if total > 0.0 => Some((used / total) * 100.0),
                    _ => None,
                };
                return (util, mem_percent);
            }
        }
    }
    (None, None)
}

fn run_command_with_timeout(
    command: &str,
    args: &[&str],
    timeout: Duration,
) -> Option<std::process::Output> {
    let mut child = Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let start = Instant::now();

    loop {
        if let Ok(Some(_status)) = child.try_wait() {
            return child.wait_with_output().ok();
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

#[derive(serde::Deserialize)]
struct LogParams {
    page: Option<i64>,
    limit: Option<i64>,
}

async fn logs_history_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    match state.db.get_logs(limit, offset).await {
        Ok(logs) => axum::Json(logs).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn clear_logs_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.clear_logs().await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct JobTableParams {
    limit: Option<i64>,
    page: Option<i64>,
    status: Option<String>,
    search: Option<String>,
    sort: Option<String>,
    sort_by: Option<String>,
    sort_desc: Option<bool>,
    archived: Option<String>,
}

async fn jobs_table_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<JobTableParams>,
) -> impl IntoResponse {
    let JobTableParams {
        limit,
        page,
        status,
        search,
        sort,
        sort_by,
        sort_desc,
        archived,
    } = params;

    let limit = limit.unwrap_or(50).clamp(1, 200);
    let page = page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let statuses = if let Some(s) = status {
        let list: Vec<JobState> = s
            .split(',')
            .filter_map(|s| serde_json::from_value(serde_json::Value::String(s.to_string())).ok())
            .collect();
        if list.is_empty() {
            None
        } else {
            Some(list)
        }
    } else {
        None
    };

    let archived = match archived.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        Some(_) | None => Some(false),
    };

    match state
        .db
        .get_jobs_filtered(crate::db::JobFilterQuery {
            limit,
            offset,
            statuses,
            search,
            sort_by: sort_by.or(sort),
            sort_desc: sort_desc.unwrap_or(false),
            archived,
        })
        .await
    {
        Ok(jobs) => axum::Json(jobs).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct BatchActionPayload {
    action: String,
    ids: Vec<i64>,
}

async fn batch_jobs_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<BatchActionPayload>,
) -> impl IntoResponse {
    let jobs = match state.db.get_jobs_by_ids(&payload.ids).await {
        Ok(jobs) => jobs,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    match payload.action.as_str() {
        "cancel" => {
            let mut count = 0_u64;
            for job in &jobs {
                match request_job_cancel(&state, job).await {
                    Ok(true) => count += 1,
                    Ok(false) => {}
                    Err(e) if is_row_not_found(&e) => {}
                    Err(e) => {
                        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
                    }
                }
            }
            axum::Json(serde_json::json!({ "count": count })).into_response()
        }
        "delete" | "restart" => {
            let blocked: Vec<_> = jobs.iter().filter(|job| job.is_active()).cloned().collect();
            if !blocked.is_empty() {
                return blocked_jobs_response(
                    format!("{} is blocked while jobs are active", payload.action),
                    &blocked,
                );
            }

            let result = if payload.action == "delete" {
                state.db.batch_delete_jobs(&payload.ids).await
            } else {
                state.db.batch_restart_jobs(&payload.ids).await
            };

            match result {
                Ok(count) => axum::Json(serde_json::json!({ "count": count })).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        _ => (StatusCode::BAD_REQUEST, "Invalid action").into_response(),
    }
}

#[derive(Deserialize)]
struct AddNotificationTargetPayload {
    name: String,
    target_type: String,
    endpoint_url: String,
    auth_token: Option<String>,
    events: Vec<String>,
    enabled: bool,
}

// #[derive(Deserialize)]
// struct TestNotificationPayload {
//     target: AddNotificationTargetPayload,
// }

async fn get_notifications_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_notification_targets().await {
        Ok(t) => axum::Json(serde_json::json!(t)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn add_notification_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<AddNotificationTargetPayload>,
) -> impl IntoResponse {
    if let Err(msg) = validate_notification_url(&payload.endpoint_url).await {
        return (StatusCode::BAD_REQUEST, msg).into_response();
    }

    let mut next_config = state.config.read().await.clone();
    next_config
        .notifications
        .targets
        .push(crate::config::NotificationTargetConfig {
            name: payload.name.clone(),
            target_type: payload.target_type.clone(),
            endpoint_url: payload.endpoint_url.clone(),
            auth_token: payload.auth_token.clone(),
            events: payload.events.clone(),
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

    match state.db.get_notification_targets().await {
        Ok(targets) => targets
            .into_iter()
            .find(|target| {
                target.name == payload.name
                    && target.target_type == payload.target_type
                    && target.endpoint_url == payload.endpoint_url
            })
            .map(|target| axum::Json(serde_json::json!(target)).into_response())
            .unwrap_or_else(|| StatusCode::OK.into_response()),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_notification_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let target = match state.db.get_notification_targets().await {
        Ok(targets) => targets.into_iter().find(|target| target.id == id),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let Some(target) = target else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let mut next_config = state.config.read().await.clone();
    next_config.notifications.targets.retain(|candidate| {
        !(candidate.name == target.name
            && candidate.target_type == target.target_type
            && candidate.endpoint_url == target.endpoint_url)
    });
    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config = state.config.write().await;
        *config = next_config;
    }
    StatusCode::OK.into_response()
}

async fn test_notification_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<AddNotificationTargetPayload>,
) -> impl IntoResponse {
    if let Err(msg) = validate_notification_url(&payload.endpoint_url).await {
        return (StatusCode::BAD_REQUEST, msg).into_response();
    }

    // Construct a temporary target
    let events_json = serde_json::to_string(&payload.events).unwrap_or_default();
    let target = crate::db::NotificationTarget {
        id: 0,
        name: payload.name,
        target_type: payload.target_type,
        endpoint_url: payload.endpoint_url,
        auth_token: payload.auth_token,
        events: events_json,
        enabled: payload.enabled,
        created_at: Utc::now(),
    };

    match state.notification_manager.send_test(&target).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_schedule_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_schedule_windows().await {
        Ok(w) => axum::Json(serde_json::json!(w)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct AddSchedulePayload {
    start_time: String,
    end_time: String,
    days_of_week: Vec<i32>,
    enabled: bool,
}

fn normalize_schedule_time(value: &str) -> Option<String> {
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

async fn add_schedule_handler(
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
        Ok(windows) => windows
            .into_iter()
            .find(|window| {
                window.start_time == start_time
                    && window.end_time == end_time
                    && window.enabled == payload.enabled
            })
            .map(|window| axum::Json(serde_json::json!(window)).into_response())
            .unwrap_or_else(|| StatusCode::OK.into_response()),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_schedule_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let window = match state.db.get_schedule_windows().await {
        Ok(windows) => windows.into_iter().find(|window| window.id == id),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let Some(window) = window else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let days_of_week: Vec<i32> = serde_json::from_str(&window.days_of_week).unwrap_or_default();
    let mut next_config = state.config.read().await.clone();
    next_config.schedule.windows.retain(|candidate| {
        !(candidate.start_time == window.start_time
            && candidate.end_time == window.end_time
            && candidate.enabled == window.enabled
            && candidate.days_of_week == days_of_week)
    });
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

#[derive(serde::Deserialize)]
struct AddWatchDirPayload {
    path: String,
    is_recursive: Option<bool>,
}

#[derive(serde::Serialize)]
struct LibraryProfileResponse {
    id: i64,
    name: String,
    preset: String,
    codec: String,
    quality_profile: String,
    hdr_mode: String,
    audio_mode: String,
    crf_override: Option<i32>,
    notes: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    builtin: bool,
}

#[derive(serde::Deserialize)]
struct LibraryProfilePayload {
    name: String,
    preset: String,
    codec: String,
    quality_profile: String,
    hdr_mode: String,
    audio_mode: String,
    crf_override: Option<i32>,
    notes: Option<String>,
}

#[derive(serde::Deserialize)]
struct AssignWatchDirProfilePayload {
    profile_id: Option<i64>,
}

fn is_builtin_profile_id(id: i64) -> bool {
    crate::config::BUILT_IN_LIBRARY_PROFILES
        .iter()
        .any(|profile| profile.id == id)
}

fn library_profile_response(profile: crate::db::LibraryProfile) -> LibraryProfileResponse {
    LibraryProfileResponse {
        id: profile.id,
        name: profile.name,
        preset: profile.preset,
        codec: profile.codec,
        quality_profile: profile.quality_profile,
        hdr_mode: profile.hdr_mode,
        audio_mode: profile.audio_mode,
        crf_override: profile.crf_override,
        notes: profile.notes,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
        builtin: is_builtin_profile_id(profile.id),
    }
}

fn validate_library_profile_payload(
    payload: &LibraryProfilePayload,
) -> std::result::Result<(), &'static str> {
    if payload.name.trim().is_empty() {
        return Err("name must not be empty");
    }
    if payload.preset.trim().is_empty() {
        return Err("preset must not be empty");
    }
    if payload.codec.trim().is_empty() {
        return Err("codec must not be empty");
    }
    if payload.quality_profile.trim().is_empty() {
        return Err("quality_profile must not be empty");
    }
    if payload.hdr_mode.trim().is_empty() {
        return Err("hdr_mode must not be empty");
    }
    if payload.audio_mode.trim().is_empty() {
        return Err("audio_mode must not be empty");
    }
    Ok(())
}

fn to_new_library_profile(payload: LibraryProfilePayload) -> crate::db::NewLibraryProfile {
    crate::db::NewLibraryProfile {
        name: payload.name.trim().to_string(),
        preset: payload.preset.trim().to_string(),
        codec: payload.codec.trim().to_ascii_lowercase(),
        quality_profile: payload.quality_profile.trim().to_ascii_lowercase(),
        hdr_mode: payload.hdr_mode.trim().to_ascii_lowercase(),
        audio_mode: payload.audio_mode.trim().to_ascii_lowercase(),
        crf_override: payload.crf_override,
        notes: payload
            .notes
            .map(|notes| notes.trim().to_string())
            .filter(|notes| !notes.is_empty()),
    }
}

async fn get_watch_dirs_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_watch_dirs().await {
        Ok(dirs) => axum::Json(dirs).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn add_watch_dir_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<AddWatchDirPayload>,
) -> impl IntoResponse {
    let normalized_path = match canonicalize_directory_path(&payload.path, "path") {
        Ok(path) => path,
        Err(msg) => return (StatusCode::BAD_REQUEST, msg).into_response(),
    };

    let normalized_path = normalized_path.to_string_lossy().to_string();
    let mut next_config = state.config.read().await.clone();
    if next_config
        .scanner
        .extra_watch_dirs
        .iter()
        .any(|watch_dir| watch_dir.path == normalized_path)
    {
        return (StatusCode::CONFLICT, "watch folder already exists").into_response();
    }
    next_config
        .scanner
        .extra_watch_dirs
        .push(crate::config::WatchDirConfig {
            path: normalized_path.clone(),
            is_recursive: payload.is_recursive.unwrap_or(true),
        });
    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config = state.config.write().await;
        *config = next_config;
    }
    refresh_file_watcher(&state).await;

    match state.db.get_watch_dirs().await {
        Ok(dirs) => dirs
            .into_iter()
            .find(|dir| dir.path == normalized_path)
            .map(|dir| axum::Json(dir).into_response())
            .unwrap_or_else(|| StatusCode::OK.into_response()),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn remove_watch_dir_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let dir = match state.db.get_watch_dirs().await {
        Ok(dirs) => dirs.into_iter().find(|dir| dir.id == id),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let Some(dir) = dir else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let mut next_config = state.config.read().await.clone();
    next_config
        .scanner
        .extra_watch_dirs
        .retain(|watch_dir| watch_dir.path != dir.path);
    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }
    {
        let mut config = state.config.write().await;
        *config = next_config;
    }
    refresh_file_watcher(&state).await;
    StatusCode::OK.into_response()
}

async fn list_profiles_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_all_profiles().await {
        Ok(profiles) => axum::Json(
            profiles
                .into_iter()
                .map(library_profile_response)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn get_profile_presets_handler() -> impl IntoResponse {
    let presets = crate::config::BUILT_IN_LIBRARY_PROFILES
        .iter()
        .map(|preset| {
            serde_json::json!({
                "id": preset.id,
                "name": preset.name,
                "preset": preset.preset,
                "codec": preset.codec.as_str(),
                "quality_profile": preset.quality_profile.as_str(),
                "hdr_mode": preset.hdr_mode.as_str(),
                "audio_mode": preset.audio_mode.as_str(),
                "crf_override": preset.crf_override,
                "notes": preset.notes,
                "builtin": true
            })
        })
        .collect::<Vec<_>>();
    axum::Json(presets).into_response()
}

async fn create_profile_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<LibraryProfilePayload>,
) -> impl IntoResponse {
    if let Err(message) = validate_library_profile_payload(&payload) {
        return (StatusCode::BAD_REQUEST, message).into_response();
    }

    let new_profile = to_new_library_profile(payload);
    let id = match state.db.create_profile(new_profile).await {
        Ok(id) => id,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    match state.db.get_profile(id).await {
        Ok(Some(profile)) => (
            StatusCode::CREATED,
            axum::Json(library_profile_response(profile)),
        )
            .into_response(),
        Ok(None) => StatusCode::CREATED.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn update_profile_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    axum::Json(payload): axum::Json<LibraryProfilePayload>,
) -> impl IntoResponse {
    if is_builtin_profile_id(id) {
        return (StatusCode::CONFLICT, "Built-in presets are read-only").into_response();
    }
    if let Err(message) = validate_library_profile_payload(&payload) {
        return (StatusCode::BAD_REQUEST, message).into_response();
    }

    match state
        .db
        .update_profile(id, to_new_library_profile(payload))
        .await
    {
        Ok(_) => match state.db.get_profile(id).await {
            Ok(Some(profile)) => axum::Json(library_profile_response(profile)).into_response(),
            Ok(None) => StatusCode::NOT_FOUND.into_response(),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        },
        Err(err) if is_row_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn delete_profile_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if is_builtin_profile_id(id) {
        return (StatusCode::CONFLICT, "Built-in presets cannot be deleted").into_response();
    }

    match state.db.count_watch_dirs_using_profile(id).await {
        Ok(count) if count > 0 => (
            StatusCode::CONFLICT,
            "Profile is still assigned to one or more watch folders",
        )
            .into_response(),
        Ok(_) => match state.db.delete_profile(id).await {
            Ok(_) => StatusCode::OK.into_response(),
            Err(err) if is_row_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        },
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn assign_watch_dir_profile_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    axum::Json(payload): axum::Json<AssignWatchDirProfilePayload>,
) -> impl IntoResponse {
    if let Some(profile_id) = payload.profile_id {
        match state.db.get_profile(profile_id).await {
            Ok(Some(_)) => {}
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
            }
        }
    }

    match state
        .db
        .assign_profile_to_watch_dir(id, payload.profile_id)
        .await
    {
        Ok(_) => match state.db.get_watch_dirs().await {
            Ok(dirs) => dirs
                .into_iter()
                .find(|dir| dir.id == id)
                .map(|dir| axum::Json(dir).into_response())
                .unwrap_or_else(|| StatusCode::OK.into_response()),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        },
        Err(err) if is_row_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn restart_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.get_job_by_id(id).await {
        Ok(Some(job)) => {
            if job.is_active() {
                return blocked_jobs_response("restart is blocked while the job is active", &[job]);
            }
            if let Err(e) = state.db.batch_restart_jobs(&[job.id]).await {
                return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
            }
            StatusCode::OK.into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let job = match state.db.get_job_by_id(id).await {
        Ok(Some(job)) => job,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    if job.is_active() {
        return blocked_jobs_response("delete is blocked while the job is active", &[job]);
    }

    match state.db.delete_job(id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) if is_row_not_found(&e) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct UpdateJobPriorityPayload {
    priority: i32,
}

async fn update_job_priority_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    axum::Json(payload): axum::Json<UpdateJobPriorityPayload>,
) -> impl IntoResponse {
    match state.db.set_job_priority(id, payload.priority).await {
        Ok(_) => axum::Json(serde_json::json!({ "id": id, "priority": payload.priority }))
            .into_response(),
        Err(e) if is_row_not_found(&e) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Serialize)]
struct JobDetailResponse {
    job: crate::db::Job,
    metadata: Option<crate::media::pipeline::MediaMetadata>,
    encode_stats: Option<crate::db::DetailedEncodeStats>,
    job_logs: Vec<crate::db::LogEntry>,
    job_failure_summary: Option<String>,
}

async fn get_job_detail_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let job = match state.db.get_job_by_id(id).await {
        Ok(Some(j)) => j,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // Avoid long probes while the job is still active.
    let metadata = match job.status {
        crate::db::JobState::Queued
        | crate::db::JobState::Analyzing
        | crate::db::JobState::Encoding
        | crate::db::JobState::Remuxing => None,
        _ => {
            let analyzer = crate::media::analyzer::FfmpegAnalyzer;
            use crate::media::pipeline::Analyzer;
            analyzer
                .analyze(std::path::Path::new(&job.input_path))
                .await
                .ok()
                .map(|analysis| analysis.metadata)
        }
    };

    // Try to get encode stats (using the subquery result or a specific query)
    // For now we'll just query the encode_stats table if completed
    let encode_stats = if job.status == crate::db::JobState::Completed {
        state.db.get_encode_stats_by_job_id(id).await.ok()
    } else {
        None
    };

    let job_logs = match state.db.get_logs_for_job(id, 200).await {
        Ok(logs) => logs,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    let job_failure_summary = if job.status == crate::db::JobState::Failed {
        job_logs
            .iter()
            .rev()
            .find(|entry| entry.level.eq_ignore_ascii_case("error"))
            .map(|entry| entry.message.clone())
    } else {
        None
    };

    axum::Json(JobDetailResponse {
        job,
        metadata,
        encode_stats,
        job_logs,
        job_failure_summary,
    })
    .into_response()
}

async fn get_file_settings_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
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
struct UpdateFileSettingsPayload {
    delete_source: bool,
    output_extension: String,
    output_suffix: String,
    replace_strategy: String,
    #[serde(default)]
    output_root: Option<String>,
}

async fn update_file_settings_handler(
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

fn has_path_separator(value: &str) -> bool {
    value.chars().any(|c| c == '/' || c == '\\')
}

#[derive(Serialize)]
struct SystemInfo {
    version: String,
    os_version: String,
    is_docker: bool,
    telemetry_enabled: bool,
    ffmpeg_version: String,
}

async fn get_system_info_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    let version = env!("CARGO_PKG_VERSION").to_string();
    let os_version = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    let is_docker = std::path::Path::new("/.dockerenv").exists();

    // Attempt to verify ffmpeg version
    let ffmpeg_version =
        crate::media::ffmpeg::verify_ffmpeg().unwrap_or_else(|_| "Unknown".to_string());

    axum::Json(SystemInfo {
        version,
        os_version,
        is_docker,
        telemetry_enabled: config.system.enable_telemetry,
        ffmpeg_version,
    })
    .into_response()
}

#[derive(Serialize)]
struct TelemetryPayload {
    runtime_id: String,
    timestamp: String,
    version: String,
    os_version: String,
    is_docker: bool,
    uptime_seconds: u64,
    cpu_count: usize,
    memory_total_mb: u64,
    active_jobs: i64,
    concurrent_limit: usize,
}

async fn telemetry_payload_handler(State(state): State<Arc<AppState>>) -> Response {
    let config = state.config.read().await;
    if !config.system.enable_telemetry {
        return (StatusCode::FORBIDDEN, "Telemetry disabled").into_response();
    }

    let (cpu_count, memory_total_mb) = {
        let mut sys = match state.sys.lock() {
            Ok(sys) => sys,
            Err(e) => {
                error!("System monitor lock poisoned: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "System monitor unavailable",
                )
                    .into_response();
            }
        };
        sys.refresh_memory();
        (sys.cpus().len(), (sys.total_memory() / 1024 / 1024) as u64)
    };

    let version = env!("CARGO_PKG_VERSION").to_string();
    let os_version = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    let is_docker = std::path::Path::new("/.dockerenv").exists();
    let uptime_seconds = state.start_time.elapsed().as_secs();
    let stats = match state.db.get_job_stats().await {
        Ok(stats) => stats,
        Err(err) => return config_read_error_response("load telemetry stats", &err),
    };

    axum::Json(TelemetryPayload {
        runtime_id: state.telemetry_runtime_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        version,
        os_version,
        is_docker,
        uptime_seconds,
        cpu_count,
        memory_total_mb,
        active_jobs: stats.active,
        concurrent_limit: config.transcode.concurrent_jobs,
    })
    .into_response()
}

async fn get_hardware_info_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.hardware_state.snapshot().await {
        Some(info) => axum::Json(info).into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Hardware state unavailable",
        )
            .into_response(),
    }
}

async fn get_hardware_probe_log_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    axum::Json(state.hardware_probe_log.read().await.clone()).into_response()
}

async fn start_scan_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.library_scanner.start_scan().await {
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_scan_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    axum::Json::<crate::system::scanner::ScanStatus>(state.library_scanner.get_status().await)
        .into_response()
}

async fn allow_login_attempt(state: &AppState, ip: IpAddr) -> bool {
    let mut limiter = state.login_rate_limiter.lock().await;
    let now = Instant::now();
    let cleanup_after = Duration::from_secs(60 * 60);
    limiter.retain(|_, entry| now.duration_since(entry.last_refill) <= cleanup_after);

    let entry = limiter.entry(ip).or_insert(RateLimitEntry {
        tokens: LOGIN_RATE_LIMIT_CAPACITY,
        last_refill: now,
    });

    let elapsed = now.duration_since(entry.last_refill).as_secs_f64();
    if elapsed > 0.0 {
        let refill = elapsed * LOGIN_RATE_LIMIT_REFILL_PER_SEC;
        entry.tokens = (entry.tokens + refill).min(LOGIN_RATE_LIMIT_CAPACITY);
        entry.last_refill = now;
    }

    if entry.tokens >= 1.0 {
        entry.tokens -= 1.0;
        true
    } else {
        false
    }
}

async fn allow_global_request(state: &AppState, ip: IpAddr) -> bool {
    let mut limiter = state.global_rate_limiter.lock().await;
    let now = Instant::now();
    let cleanup_after = Duration::from_secs(60 * 60);
    limiter.retain(|_, entry| now.duration_since(entry.last_refill) <= cleanup_after);
    let entry = limiter.entry(ip).or_insert(RateLimitEntry {
        tokens: GLOBAL_RATE_LIMIT_CAPACITY,
        last_refill: now,
    });

    let elapsed = now.duration_since(entry.last_refill).as_secs_f64();
    if elapsed > 0.0 {
        let refill = elapsed * GLOBAL_RATE_LIMIT_REFILL_PER_SEC;
        entry.tokens = (entry.tokens + refill).min(GLOBAL_RATE_LIMIT_CAPACITY);
        entry.last_refill = now;
    }

    if entry.tokens >= 1.0 {
        entry.tokens -= 1.0;
        true
    } else {
        false
    }
}

fn build_session_cookie(token: &str) -> String {
    let mut cookie = format!(
        "alchemist_session={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000",
        token
    );
    if secure_cookie_enabled() {
        cookie.push_str("; Secure");
    }
    cookie
}

fn build_clear_session_cookie() -> String {
    let mut cookie = "alchemist_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0".to_string();
    if secure_cookie_enabled() {
        cookie.push_str("; Secure");
    }
    cookie
}

fn get_cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let mut iter = part.trim().splitn(2, '=');
        let key = iter.next()?.trim();
        let value = iter.next()?.trim();
        if key == name {
            return Some(value.to_string());
        }
    }
    None
}

fn request_ip(req: &Request) -> Option<IpAddr> {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
}

fn secure_cookie_enabled() -> bool {
    match std::env::var("ALCHEMIST_COOKIE_SECURE") {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => !cfg!(debug_assertions),
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

async fn validate_notification_url(raw: &str) -> std::result::Result<(), String> {
    let url = Url::parse(raw).map_err(|_| "endpoint_url must be a valid URL".to_string())?;
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

    if host.eq_ignore_ascii_case("localhost") {
        return Err("endpoint_url host is not allowed".to_string());
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) {
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
            if is_private_ip(addr.ip()) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Method, Request},
    };
    use futures::StreamExt;
    use http_body_util::BodyExt;
    use serde_json::json;
    use std::path::PathBuf;
    use tower::util::ServiceExt;

    fn temp_path(prefix: &str, extension: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("{prefix}_{}.{}", rand::random::<u64>(), extension));
        path
    }

    async fn build_test_app<F>(
        setup_required: bool,
        tx_capacity: usize,
        configure: F,
    ) -> std::result::Result<(Arc<AppState>, Router, PathBuf, PathBuf), Box<dyn std::error::Error>>
    where
        F: FnOnce(&mut crate::config::Config),
    {
        let db_path = temp_path("alchemist_server_test", "db");
        let config_path = temp_path("alchemist_server_test", "toml");

        let mut config_value = crate::config::Config::default();
        configure(&mut config_value);
        config_value.save(&config_path)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let config = Arc::new(RwLock::new(config_value));
        let hardware_state = HardwareState::new(Some(crate::system::hardware::HardwareInfo {
            vendor: crate::system::hardware::Vendor::Cpu,
            device_path: None,
            supported_codecs: vec!["av1".to_string(), "hevc".to_string(), "h264".to_string()],
            backends: Vec::new(),
            detection_notes: Vec::new(),
        }));
        let hardware_probe_log = Arc::new(RwLock::new(HardwareProbeLog::default()));
        let (tx, _rx) = broadcast::channel(tx_capacity);
        let transcoder = Arc::new(Transcoder::new());
        let agent = Arc::new(
            Agent::new(
                db.clone(),
                transcoder.clone(),
                config.clone(),
                hardware_state.clone(),
                tx.clone(),
                true,
            )
            .await,
        );
        let scheduler = crate::scheduler::Scheduler::new(db.clone(), agent.clone());
        let file_watcher = Arc::new(crate::system::watcher::FileWatcher::new(db.clone()));

        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let state = Arc::new(AppState {
            db: db.clone(),
            config: config.clone(),
            agent,
            transcoder,
            scheduler: scheduler.handle(),
            tx,
            setup_required: Arc::new(AtomicBool::new(setup_required)),
            start_time: Instant::now(),
            telemetry_runtime_id: "test-runtime".to_string(),
            notification_manager: Arc::new(crate::notifications::NotificationManager::new(
                db.as_ref().clone(),
            )),
            sys: std::sync::Mutex::new(sys),
            file_watcher,
            library_scanner: Arc::new(crate::system::scanner::LibraryScanner::new(db, config)),
            config_path: config_path.clone(),
            config_mutable: true,
            hardware_state,
            hardware_probe_log,
            resources_cache: Arc::new(tokio::sync::Mutex::new(None)),
            login_rate_limiter: Mutex::new(HashMap::new()),
            global_rate_limiter: Mutex::new(HashMap::new()),
        });

        Ok((state.clone(), app_router(state), config_path, db_path))
    }

    async fn create_session(db: &Db) -> std::result::Result<String, Box<dyn std::error::Error>> {
        let user_id = db.create_user("tester", "hash").await?;
        let token = format!("test-session-{}", rand::random::<u64>());
        db.create_session(user_id, &token, Utc::now() + chrono::Duration::days(1))
            .await?;
        Ok(token)
    }

    fn auth_request(method: Method, uri: &str, token: &str, body: Body) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, format!("alchemist_session={token}"))
            .body(body)
            .unwrap()
    }

    fn auth_json_request(
        method: Method,
        uri: &str,
        token: &str,
        body: serde_json::Value,
    ) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, format!("alchemist_session={token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn body_text(response: Response) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    async fn seed_job(
        db: &Db,
        status: JobState,
    ) -> std::result::Result<(crate::db::Job, PathBuf, PathBuf), Box<dyn std::error::Error>> {
        let input = temp_path("alchemist_job_input", "mkv");
        let output = temp_path("alchemist_job_output", "mkv");
        std::fs::write(&input, b"test")?;

        db.enqueue_job(&input, &output, std::time::SystemTime::UNIX_EPOCH)
            .await?;
        let job = db
            .get_job_by_input_path(input.to_string_lossy().as_ref())
            .await?
            .expect("job");
        if job.status != status {
            db.update_job_status(job.id, status).await?;
        }

        let job = db.get_job_by_id(job.id).await?.expect("job by id");
        Ok((job, input, output))
    }

    fn cleanup_paths(paths: &[PathBuf]) {
        for path in paths {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_dir_all(path);
        }
    }

    fn sample_transcode_payload() -> TranscodeSettingsPayload {
        TranscodeSettingsPayload {
            concurrent_jobs: 1,
            size_reduction_threshold: 0.3,
            min_bpp_threshold: 0.1,
            min_file_size_mb: 50,
            output_codec: crate::config::OutputCodec::Av1,
            quality_profile: crate::config::QualityProfile::Balanced,
            threads: 0,
            allow_fallback: true,
            hdr_mode: crate::config::HdrMode::Preserve,
            tonemap_algorithm: crate::config::TonemapAlgorithm::Hable,
            tonemap_peak: 100.0,
            tonemap_desat: 0.2,
            subtitle_mode: crate::config::SubtitleMode::Copy,
            stream_rules: crate::config::StreamRules::default(),
        }
    }

    #[test]
    fn validate_transcode_payload_rejects_invalid_values() {
        let mut payload = sample_transcode_payload();
        payload.concurrent_jobs = 0;
        assert!(validate_transcode_payload(&payload).is_err());

        let mut payload = sample_transcode_payload();
        payload.size_reduction_threshold = 1.5;
        assert!(validate_transcode_payload(&payload).is_err());

        let mut payload = sample_transcode_payload();
        payload.tonemap_peak = 10.0;
        assert!(validate_transcode_payload(&payload).is_err());

        let mut payload = sample_transcode_payload();
        payload.tonemap_desat = 2.0;
        assert!(validate_transcode_payload(&payload).is_err());
    }

    #[test]
    fn normalize_setup_directories_trims_and_filters() {
        let movies_dir = temp_path("alchemist_setup_movies", "dir");
        let tv_dir = temp_path("alchemist_setup_tv", "dir");
        std::fs::create_dir_all(&movies_dir).unwrap();
        std::fs::create_dir_all(&tv_dir).unwrap();

        let input = vec![
            format!(" {} ", movies_dir.to_string_lossy()),
            "".to_string(),
            "   ".to_string(),
            tv_dir.to_string_lossy().to_string(),
        ];

        let normalized = normalize_setup_directories(&input).expect("normalize");
        assert_eq!(
            normalized,
            vec![
                std::fs::canonicalize(&movies_dir)
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                std::fs::canonicalize(&tv_dir)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            ]
        );

        cleanup_paths(&[movies_dir, tv_dir]);
    }

    #[test]
    fn config_write_blocked_returns_409() {
        let response = config_write_blocked_response(FsPath::new("/tmp/config.toml"));
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn config_save_permission_error_maps_to_409() {
        let err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let response = config_save_error_to_response(
            FsPath::new("/tmp/config.toml"),
            &anyhow::Error::new(err),
        );
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn config_save_other_errors_map_to_500() {
        let err = anyhow::anyhow!("something failed");
        let response = config_save_error_to_response(FsPath::new("/tmp/config.toml"), &err);
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn sse_message_stream_emits_lagged_event_and_recovers() {
        let (tx, rx) = broadcast::channel(1);
        tx.send(AlchemistEvent::Log {
            level: "info".to_string(),
            job_id: None,
            message: "first".to_string(),
        })
        .unwrap();
        tx.send(AlchemistEvent::Log {
            level: "info".to_string(),
            job_id: None,
            message: "second".to_string(),
        })
        .unwrap();
        drop(tx);

        let mut stream = Box::pin(sse_message_stream(rx));
        let first = stream.next().await.unwrap().unwrap();
        assert_eq!(first.event_name, "lagged");
        assert!(first.data.contains("\"skipped\":1"));

        let second = stream.next().await.unwrap().unwrap();
        assert_eq!(second.event_name, "log");
        assert!(second.data.contains("\"second\""));
    }

    #[tokio::test]
    async fn hardware_settings_route_updates_runtime_state(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;

        let response = app
            .clone()
            .oneshot(auth_json_request(
                Method::POST,
                "/api/settings/hardware",
                &token,
                json!({
                    "allow_cpu_fallback": true,
                    "allow_cpu_encoding": true,
                    "cpu_preset": "medium",
                    "preferred_vendor": "cpu",
                    "device_path": null
                }),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let hardware = state.hardware_state.snapshot().await.unwrap();
        assert_eq!(hardware.vendor, crate::system::hardware::Vendor::Cpu);

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::GET,
                "/api/system/hardware",
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_text(response).await;
        assert!(body.contains("\"vendor\":\"cpu\""));

        let persisted = crate::config::Config::load(config_path.as_path())?;
        assert_eq!(persisted.hardware.preferred_vendor.as_deref(), Some("cpu"));
        assert_eq!(persisted.hardware.device_path, None);

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn engine_mode_endpoint_applies_manual_override_and_persists_mode(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;

        let response = app
            .clone()
            .oneshot(auth_json_request(
                Method::POST,
                "/api/engine/mode",
                &token,
                json!({
                    "mode": "throughput",
                    "concurrent_jobs_override": 2,
                    "threads_override": 3
                }),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let payload: serde_json::Value = serde_json::from_str(&body_text(response).await)?;
        assert_eq!(payload["mode"], "throughput");
        assert_eq!(payload["concurrent_limit"], 2);
        assert_eq!(payload["is_manual_override"], true);

        assert_eq!(
            state.agent.current_mode().await,
            crate::config::EngineMode::Throughput
        );
        assert_eq!(state.agent.concurrent_jobs_limit(), 2);
        assert!(state.agent.is_manual_override());

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::GET,
                "/api/engine/mode",
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let payload: serde_json::Value = serde_json::from_str(&body_text(response).await)?;
        assert_eq!(payload["mode"], "throughput");
        assert_eq!(payload["concurrent_limit"], 2);
        assert_eq!(payload["is_manual_override"], true);
        assert!(payload["cpu_count"].as_u64().unwrap_or(0) > 0);

        let persisted = crate::config::Config::load(config_path.as_path())?;
        assert_eq!(
            persisted.system.engine_mode,
            crate::config::EngineMode::Throughput
        );
        assert_eq!(persisted.transcode.threads, 3);

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn engine_status_endpoint_reports_draining_state(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;

        state.agent.pause();
        state.agent.set_scheduler_paused(true);
        state.agent.set_manual_override(true);
        state.agent.drain();

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::GET,
                "/api/engine/status",
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let payload: serde_json::Value = serde_json::from_str(&body_text(response).await)?;
        assert_eq!(payload["status"], "draining");
        assert_eq!(payload["manual_paused"], true);
        assert_eq!(payload["scheduler_paused"], true);
        assert_eq!(payload["draining"], true);
        assert_eq!(payload["mode"], "balanced");
        assert_eq!(payload["concurrent_limit"], 1);
        assert_eq!(payload["is_manual_override"], true);

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn hardware_probe_log_route_returns_runtime_log(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;

        *state.hardware_probe_log.write().await = HardwareProbeLog {
            entries: vec![crate::system::hardware::HardwareProbeEntry {
                encoder: "hevc_videotoolbox".to_string(),
                backend: "videotoolbox".to_string(),
                device_path: None,
                success: false,
                stderr: Some("Unknown encoder".to_string()),
            }],
        };

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::GET,
                "/api/system/hardware/probe-log",
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let body = body_text(response).await;
        assert!(body.contains("\"encoder\":\"hevc_videotoolbox\""));
        assert!(body.contains("\"stderr\":\"Unknown encoder\""));

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn setup_complete_updates_runtime_hardware_without_mirroring_watch_dirs(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let watch_dir = temp_path("alchemist_setup_watch", "dir");
        std::fs::create_dir_all(&watch_dir)?;

        let (state, app, config_path, db_path) = build_test_app(true, 8, |config| {
            config.hardware.preferred_vendor = Some("cpu".to_string());
        })
        .await?;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/setup/complete")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "username": "admin",
                            "password": "password123",
                            "size_reduction_threshold": 0.3,
                            "min_bpp_threshold": 0.1,
                            "min_file_size_mb": 50,
                            "concurrent_jobs": 1,
                            "directories": [watch_dir.to_string_lossy().to_string()],
                            "allow_cpu_encoding": true,
                            "enable_telemetry": false,
                            "output_codec": "av1",
                            "quality_profile": "balanced"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let set_cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();

        assert!(!state.setup_required.load(Ordering::Relaxed));
        assert_eq!(
            state.hardware_state.snapshot().await.unwrap().vendor,
            crate::system::hardware::Vendor::Cpu
        );

        let watch_dirs = state.db.get_watch_dirs().await?;
        assert!(watch_dirs.is_empty());

        let persisted = crate::config::Config::load(config_path.as_path())?;
        assert!(persisted.scanner.watch_enabled);
        assert_eq!(
            persisted.scanner.directories,
            vec![std::fs::canonicalize(&watch_dir)?
                .to_string_lossy()
                .to_string()]
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/system/hardware")
                    .header(header::COOKIE, set_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_text(response).await;
        assert!(body.contains("\"vendor\":\"cpu\""));

        cleanup_paths(&[watch_dir, config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn setup_complete_accepts_nested_settings_payload(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let watch_dir = temp_path("alchemist_setup_nested_watch", "dir");
        std::fs::create_dir_all(&watch_dir)?;

        let (state, app, config_path, db_path) = build_test_app(true, 8, |config| {
            config.hardware.preferred_vendor = Some("cpu".to_string());
        })
        .await?;

        let mut settings = crate::config::Config::default();
        settings.transcode.concurrent_jobs = 3;
        settings.scanner.directories = vec![watch_dir.to_string_lossy().to_string()];
        settings.appearance.active_theme_id = Some("midnight".to_string());
        settings.notifications.targets = vec![crate::config::NotificationTargetConfig {
            name: "Discord".to_string(),
            target_type: "discord".to_string(),
            endpoint_url: "https://discord.com/api/webhooks/test".to_string(),
            auth_token: None,
            events: vec!["completed".to_string()],
            enabled: true,
        }];
        settings.schedule.windows = vec![crate::config::ScheduleWindowConfig {
            start_time: "22:00".to_string(),
            end_time: "06:00".to_string(),
            days_of_week: vec![1, 2, 3],
            enabled: true,
        }];

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/setup/complete")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "username": "admin",
                            "password": "password123",
                            "settings": settings,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!state.setup_required.load(Ordering::Relaxed));

        let persisted = crate::config::Config::load(config_path.as_path())?;
        assert_eq!(
            persisted.appearance.active_theme_id.as_deref(),
            Some("midnight")
        );
        assert_eq!(persisted.notifications.targets.len(), 1);
        assert_eq!(persisted.schedule.windows.len(), 1);
        assert_eq!(persisted.transcode.concurrent_jobs, 3);
        assert_eq!(state.agent.concurrent_jobs_limit(), 3);

        cleanup_paths(&[watch_dir, config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn setup_complete_rejects_nested_settings_without_library_directories(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (_state, app, config_path, db_path) = build_test_app(true, 8, |_| {}).await?;

        let mut settings = crate::config::Config::default();
        settings.scanner.directories = Vec::new();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/setup/complete")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "username": "admin",
                            "password": "password123",
                            "settings": settings,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = body_text(response).await;
        assert!(body.contains("At least one library directory must be configured."));

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn fs_endpoints_are_available_during_setup(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let browse_root = temp_path("alchemist_fs_browse", "dir");
        std::fs::create_dir_all(&browse_root)?;
        let media_dir = browse_root.join("movies");
        std::fs::create_dir_all(&media_dir)?;
        std::fs::write(media_dir.join("movie.mkv"), b"video")?;

        let (_state, app, config_path, db_path) = build_test_app(true, 8, |_| {}).await?;

        let browse_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!(
                        "/api/fs/browse?path={}",
                        browse_root.to_string_lossy()
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await?;
        assert_eq!(browse_response.status(), StatusCode::OK);
        let browse_body = body_text(browse_response).await;
        assert!(browse_body.contains("movies"));

        let preview_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/fs/preview")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "directories": [browse_root.to_string_lossy().to_string()]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await?;
        assert_eq!(preview_response.status(), StatusCode::OK);
        let preview_body = body_text(preview_response).await;
        assert!(preview_body.contains("\"total_media_files\":1"));

        cleanup_paths(&[browse_root, config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn transcode_settings_round_trip_subtitle_mode(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::GET,
                "/api/settings/transcode",
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_text(response).await;
        assert!(body.contains("\"subtitle_mode\":\"copy\""));
        assert!(body.contains("\"stream_rules\""));

        let mut payload = sample_transcode_payload();
        payload.subtitle_mode = crate::config::SubtitleMode::None;
        payload.stream_rules = crate::config::StreamRules {
            strip_audio_by_title: vec!["commentary".to_string()],
            keep_audio_languages: vec!["eng".to_string()],
            keep_only_default_audio: false,
        };
        let response = app
            .clone()
            .oneshot(auth_json_request(
                Method::POST,
                "/api/settings/transcode",
                &token,
                serde_json::to_value(&payload)?,
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let persisted = crate::config::Config::load(config_path.as_path())?;
        assert_eq!(
            persisted.transcode.subtitle_mode,
            crate::config::SubtitleMode::None
        );
        assert_eq!(
            persisted.transcode.stream_rules.strip_audio_by_title,
            vec!["commentary".to_string()]
        );
        assert_eq!(
            persisted.transcode.stream_rules.keep_audio_languages,
            vec!["eng".to_string()]
        );

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn system_settings_round_trip_watch_enabled(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |config| {
            config.scanner.watch_enabled = true;
        })
        .await?;
        let token = create_session(state.db.as_ref()).await?;

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::GET,
                "/api/settings/system",
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_text(response).await;
        assert!(body.contains("\"watch_enabled\":true"));

        let response = app
            .clone()
            .oneshot(auth_json_request(
                Method::POST,
                "/api/settings/system",
                &token,
                json!({
                    "monitoring_poll_interval": 2.0,
                    "enable_telemetry": false,
                    "watch_enabled": false
                }),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let persisted = crate::config::Config::load(config_path.as_path())?;
        assert!(!persisted.scanner.watch_enabled);

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn settings_bundle_put_projects_extended_settings_to_db(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;

        let mut payload = crate::config::Config::default();
        payload.appearance.active_theme_id = Some("midnight".to_string());
        payload.scanner.extra_watch_dirs = vec![crate::config::WatchDirConfig {
            path: "/tmp/library".to_string(),
            is_recursive: true,
        }];
        payload.files.output_suffix = "-custom".to_string();
        payload.schedule.windows = vec![crate::config::ScheduleWindowConfig {
            start_time: "22:00".to_string(),
            end_time: "06:00".to_string(),
            days_of_week: vec![1, 2, 3],
            enabled: true,
        }];
        payload.notifications.enabled = true;
        payload.notifications.targets = vec![crate::config::NotificationTargetConfig {
            name: "Discord".to_string(),
            target_type: "discord".to_string(),
            endpoint_url: "https://discord.com/api/webhooks/test".to_string(),
            auth_token: None,
            events: vec!["completed".to_string()],
            enabled: true,
        }];

        let response = app
            .clone()
            .oneshot(auth_json_request(
                Method::PUT,
                "/api/settings/bundle",
                &token,
                serde_json::to_value(&payload)?,
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let watch_dirs = state.db.get_watch_dirs().await?;
        assert_eq!(watch_dirs.len(), 1);
        assert_eq!(watch_dirs[0].path, "/tmp/library");

        let file_settings = state.db.get_file_settings().await?;
        assert_eq!(file_settings.output_suffix, "-custom");

        let schedule = state.db.get_schedule_windows().await?;
        assert_eq!(schedule.len(), 1);

        let notifications = state.db.get_notification_targets().await?;
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].target_type, "discord");

        let theme = state.db.get_preference("active_theme_id").await?;
        assert_eq!(theme.as_deref(), Some("midnight"));

        let persisted = crate::config::Config::load(config_path.as_path())?;
        assert_eq!(
            persisted.appearance.active_theme_id.as_deref(),
            Some("midnight")
        );
        assert_eq!(persisted.files.output_suffix, "-custom");
        assert_eq!(persisted.scanner.extra_watch_dirs.len(), 1);

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn raw_config_put_overwrites_divergent_db_projection(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;

        state.db.add_watch_dir("/tmp/stale", true).await?;

        let mut payload = crate::config::Config::default();
        payload.appearance.active_theme_id = Some("ember".to_string());
        payload.files.output_extension = "mp4".to_string();
        let raw_toml = toml::to_string_pretty(&payload)?;

        let response = app
            .clone()
            .oneshot(auth_json_request(
                Method::PUT,
                "/api/settings/config",
                &token,
                json!({ "raw_toml": raw_toml }),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let watch_dirs = state.db.get_watch_dirs().await?;
        assert!(watch_dirs.is_empty());
        let file_settings = state.db.get_file_settings().await?;
        assert_eq!(file_settings.output_extension, "mp4");
        let theme = state.db.get_preference("active_theme_id").await?;
        assert_eq!(theme.as_deref(), Some("ember"));

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn hardware_settings_get_exposes_configured_device_path(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let explicit_path = if cfg!(target_os = "linux") {
            "/dev/dri/renderD128".to_string()
        } else {
            "custom-device".to_string()
        };
        let (state, app, config_path, db_path) = build_test_app(false, 8, |config| {
            config.hardware.device_path = Some(explicit_path.clone());
        })
        .await?;
        let token = create_session(state.db.as_ref()).await?;

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::GET,
                "/api/settings/hardware",
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_text(response).await;
        assert!(body.contains("\"device_path\""));

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn sse_route_emits_lagged_event_and_recovers(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 1, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::GET,
                "/api/events",
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        state.tx.send(AlchemistEvent::Log {
            level: "info".to_string(),
            job_id: None,
            message: "first".to_string(),
        })?;
        state.tx.send(AlchemistEvent::Log {
            level: "info".to_string(),
            job_id: None,
            message: "second".to_string(),
        })?;

        let mut body = response.into_body();
        let mut rendered = String::new();

        while rendered.matches("event:").count() < 2 {
            let maybe_frame =
                tokio::time::timeout(tokio::time::Duration::from_secs(2), body.frame()).await?;
            let Some(frame) = maybe_frame else {
                break;
            };
            let frame = frame?;
            if let Ok(data) = frame.into_data() {
                rendered.push_str(&String::from_utf8_lossy(&data));
            }
        }

        assert!(rendered.contains("event: lagged"));
        assert!(rendered.contains("event: log"));
        assert!(rendered.contains("\"second\""));

        cleanup_paths(&[config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn job_detail_route_includes_logs_and_failure_summary(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;
        let (job, input_path, output_path) = seed_job(state.db.as_ref(), JobState::Failed).await?;

        state
            .db
            .add_log("info", Some(job.id), "ffmpeg started")
            .await?;
        state
            .db
            .add_log("error", Some(job.id), "No such file or directory")
            .await?;

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::GET,
                &format!("/api/jobs/{}/details", job.id),
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let payload: serde_json::Value = serde_json::from_str(&body_text(response).await)?;
        assert_eq!(
            payload["job_failure_summary"].as_str(),
            Some("No such file or directory")
        );
        assert_eq!(payload["job_logs"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            payload["job_logs"][1]["message"].as_str(),
            Some("No such file or directory")
        );

        cleanup_paths(&[input_path, output_path, config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn delete_active_job_returns_conflict(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;
        let (job, input_path, output_path) =
            seed_job(state.db.as_ref(), JobState::Encoding).await?;

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::POST,
                &format!("/api/jobs/{}/delete", job.id),
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = body_text(response).await;
        assert!(body.contains("\"blocked\""));
        assert!(body.contains(&format!("\"id\":{}", job.id)));

        cleanup_paths(&[input_path, output_path, config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn batch_delete_and_restart_block_active_jobs(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;
        let (active_job, active_input, active_output) =
            seed_job(state.db.as_ref(), JobState::Encoding).await?;
        let (queued_job, queued_input, queued_output) =
            seed_job(state.db.as_ref(), JobState::Queued).await?;

        for action in ["delete", "restart"] {
            let response = app
                .clone()
                .oneshot(auth_json_request(
                    Method::POST,
                    "/api/jobs/batch",
                    &token,
                    json!({
                        "action": action,
                        "ids": [active_job.id, queued_job.id]
                    }),
                ))
                .await?;
            assert_eq!(response.status(), StatusCode::CONFLICT);
            let body = body_text(response).await;
            assert!(body.contains("\"blocked\""));
            assert!(body.contains(&format!("\"id\":{}", active_job.id)));
        }

        cleanup_paths(&[
            active_input,
            active_output,
            queued_input,
            queued_output,
            config_path,
            db_path,
        ]);
        Ok(())
    }

    #[tokio::test]
    async fn clear_completed_archives_jobs_and_preserves_stats(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;
        let (job, input_path, output_path) =
            seed_job(state.db.as_ref(), JobState::Completed).await?;

        state
            .db
            .save_encode_stats(crate::db::EncodeStatsInput {
                job_id: job.id,
                input_size: 2_000,
                output_size: 1_000,
                compression_ratio: 0.5,
                encode_time: 60.0,
                encode_speed: 1.5,
                avg_bitrate: 900.0,
                vmaf_score: Some(95.0),
                output_codec: Some("av1".to_string()),
            })
            .await?;

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::POST,
                "/api/jobs/clear-completed",
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_text(response).await;
        assert!(body.contains("\"count\":1"));
        assert!(body.contains("Historical stats were preserved"));

        assert!(state.db.get_job_by_id(job.id).await?.is_none());
        let aggregated = state.db.get_aggregated_stats().await?;
        assert_eq!(aggregated.completed_jobs, 1);
        assert_eq!(aggregated.total_input_size, 2_000);
        assert_eq!(aggregated.total_output_size, 1_000);

        cleanup_paths(&[input_path, output_path, config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn cancel_queued_job_updates_status(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;
        let (job, input_path, output_path) = seed_job(state.db.as_ref(), JobState::Queued).await?;

        let response = app
            .clone()
            .oneshot(auth_request(
                Method::POST,
                &format!("/api/jobs/{}/cancel", job.id),
                &token,
                Body::empty(),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let updated = state.db.get_job_by_id(job.id).await?.expect("updated job");
        assert_eq!(updated.status, JobState::Cancelled);

        cleanup_paths(&[input_path, output_path, config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn priority_endpoint_updates_job_priority(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;
        let (job, input_path, output_path) = seed_job(state.db.as_ref(), JobState::Queued).await?;

        let response = app
            .clone()
            .oneshot(auth_json_request(
                Method::POST,
                &format!("/api/jobs/{}/priority", job.id),
                &token,
                json!({ "priority": 10 }),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_text(response).await;
        assert!(body.contains("\"priority\":10"));

        let updated = state.db.get_job_by_id(job.id).await?.expect("updated job");
        assert_eq!(updated.priority, 10);

        cleanup_paths(&[input_path, output_path, config_path, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn watch_dir_paths_are_canonicalized_and_deduplicated(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let watch_root = temp_path("alchemist_watch_root", "dir");
        let watch_dir = watch_root.join("library");
        std::fs::create_dir_all(&watch_dir)?;

        let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
        let token = create_session(state.db.as_ref()).await?;
        let first_path = watch_dir.to_string_lossy().to_string();
        let second_path = watch_root
            .join("library/../library")
            .to_string_lossy()
            .to_string();

        let response = app
            .clone()
            .oneshot(auth_json_request(
                Method::POST,
                "/api/settings/watch-dirs",
                &token,
                json!({ "path": first_path, "is_recursive": true }),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(auth_json_request(
                Method::POST,
                "/api/settings/watch-dirs",
                &token,
                json!({ "path": second_path, "is_recursive": true }),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let dirs = state.db.get_watch_dirs().await?;
        assert_eq!(dirs.len(), 1);
        assert_eq!(
            dirs[0].path,
            std::fs::canonicalize(&watch_dir)?
                .to_string_lossy()
                .to_string()
        );

        cleanup_paths(&[watch_root, config_path, db_path]);
        Ok(())
    }
}
