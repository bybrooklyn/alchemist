use crate::config::Config;
use crate::db::{AlchemistEvent, Db, JobState};
use crate::error::{AlchemistError, Result};
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
use futures::stream::Stream;
use rand::rngs::OsRng;
use rand::Rng;
use reqwest::Url;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::{error, info};
use uuid::Uuid;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

pub struct AppState {
    pub db: Arc<Db>,
    pub config: Arc<RwLock<Config>>,
    pub agent: Arc<Agent>,
    pub transcoder: Arc<Transcoder>,
    pub tx: broadcast::Sender<AlchemistEvent>,
    pub setup_required: Arc<AtomicBool>,
    pub start_time: Instant,
    pub telemetry_runtime_id: String,
    pub notification_manager: Arc<crate::notifications::NotificationManager>,
    pub sys: std::sync::Mutex<sysinfo::System>,
    pub file_watcher: Arc<crate::system::watcher::FileWatcher>,
    pub library_scanner: Arc<crate::system::scanner::LibraryScanner>,
    login_rate_limiter: Mutex<HashMap<IpAddr, RateLimitEntry>>,
    global_rate_limiter: Mutex<RateLimitEntry>,
}

struct RateLimitEntry {
    tokens: f64,
    last_refill: Instant,
}

const LOGIN_RATE_LIMIT_CAPACITY: f64 = 10.0;
const LOGIN_RATE_LIMIT_REFILL_PER_SEC: f64 = 1.0;
const GLOBAL_RATE_LIMIT_CAPACITY: f64 = 120.0;
const GLOBAL_RATE_LIMIT_REFILL_PER_SEC: f64 = 60.0;

pub async fn run_server(
    db: Arc<Db>,
    config: Arc<RwLock<Config>>,
    agent: Arc<Agent>,
    transcoder: Arc<Transcoder>,
    tx: broadcast::Sender<AlchemistEvent>,
    setup_required: bool,
    notification_manager: Arc<crate::notifications::NotificationManager>,
    file_watcher: Arc<crate::system::watcher::FileWatcher>,
) -> Result<()> {
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
        tx,
        setup_required: Arc::new(AtomicBool::new(setup_required)),
        start_time: std::time::Instant::now(),
        telemetry_runtime_id: Uuid::new_v4().to_string(),
        notification_manager,
        sys: std::sync::Mutex::new(sys),
        file_watcher,
        library_scanner,
        login_rate_limiter: Mutex::new(HashMap::new()),
        global_rate_limiter: Mutex::new(RateLimitEntry {
            tokens: GLOBAL_RATE_LIMIT_CAPACITY,
            last_refill: Instant::now(),
        }),
    });

    let cleanup_db = state.db.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = cleanup_db.cleanup_sessions().await {
                error!("Failed to cleanup sessions: {}", e);
            }
            tokio::time::sleep(Duration::from_secs(60 * 60)).await;
        }
    });

    let app = Router::new()
        // API Routes
        .route("/api/scan/start", post(start_scan_handler))
        .route("/api/scan/status", get(get_scan_status_handler))
        .route("/api/scan", post(scan_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/stats/aggregated", get(aggregated_stats_handler))
        .route("/api/stats/daily", get(daily_stats_handler))
        .route("/api/stats/detailed", get(detailed_stats_handler))
        .route("/api/jobs/table", get(jobs_table_handler))
        .route("/api/jobs/batch", post(batch_jobs_handler))
        .route("/api/logs/history", get(logs_history_handler))
        .route("/api/logs", delete(clear_logs_handler))
        .route("/api/jobs/restart-failed", post(restart_failed_handler))
        .route("/api/jobs/clear-completed", post(clear_completed_handler))
        .route("/api/jobs/:id/cancel", post(cancel_job_handler))
        .route("/api/jobs/:id/restart", post(restart_job_handler))
        .route("/api/jobs/:id/delete", post(delete_job_handler))
        .route("/api/jobs/:id/details", get(get_job_detail_handler))
        .route("/api/events", get(sse_handler))
        .route("/api/engine/pause", post(pause_engine_handler))
        .route("/api/engine/resume", post(resume_engine_handler))
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
            "/api/settings/watch-dirs",
            get(get_watch_dirs_handler).post(add_watch_dir_handler),
        )
        .route(
            "/api/settings/watch-dirs/:id",
            delete(remove_watch_dir_handler),
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
        .with_state(state);

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

async fn refresh_file_watcher(state: &AppState) {
    if state.setup_required.load(Ordering::Relaxed) {
        if let Err(e) = state.file_watcher.watch(&[]) {
            error!("Failed to stop file watcher: {}", e);
        }
        return;
    }

    let mut watch_dirs: HashMap<PathBuf, bool> = HashMap::new();

    {
        let config = state.config.read().await;
        if config.scanner.watch_enabled {
            for dir in &config.scanner.directories {
                watch_dirs.insert(PathBuf::from(dir), true);
            }
        }
    }

    match state.db.get_watch_dirs().await {
        Ok(dirs) => {
            for dir in dirs {
                watch_dirs
                    .entry(PathBuf::from(dir.path))
                    .and_modify(|recursive| *recursive |= dir.is_recursive)
                    .or_insert(dir.is_recursive);
            }
        }
        Err(e) => error!("Failed to fetch watch dirs from DB: {}", e),
    }

    let mut all_dirs: Vec<crate::system::watcher::WatchPath> = watch_dirs
        .into_iter()
        .map(|(path, recursive)| crate::system::watcher::WatchPath { path, recursive })
        .collect();
    all_dirs.sort_by(|a, b| a.path.cmp(&b.path));

    if all_dirs.is_empty() {
        if let Err(e) = state.file_watcher.watch(&[]) {
            error!("Failed to stop file watcher: {}", e);
        }
        return;
    }

    if let Err(e) = state.file_watcher.watch(&all_dirs) {
        error!("Failed to update file watcher: {}", e);
    }
}

async fn setup_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(serde_json::json!({
        "setup_required": state.setup_required.load(Ordering::Relaxed),
        "enable_telemetry": config.system.enable_telemetry
    }))
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
    })
}

async fn update_transcode_settings_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<TranscodeSettingsPayload>,
) -> impl IntoResponse {
    let mut config = state.config.write().await;

    // Validate
    if payload.concurrent_jobs == 0 {
        return (StatusCode::BAD_REQUEST, "concurrent_jobs must be > 0").into_response();
    }
    if payload.size_reduction_threshold < 0.0 || payload.size_reduction_threshold > 1.0 {
        return (
            StatusCode::BAD_REQUEST,
            "size_reduction_threshold must be 0.0-1.0",
        )
            .into_response();
    }

    config.transcode.concurrent_jobs = payload.concurrent_jobs;
    config.transcode.size_reduction_threshold = payload.size_reduction_threshold;
    config.transcode.min_bpp_threshold = payload.min_bpp_threshold;
    config.transcode.min_file_size_mb = payload.min_file_size_mb;
    config.transcode.output_codec = payload.output_codec;
    config.transcode.quality_profile = payload.quality_profile;
    config.transcode.threads = payload.threads;

    if let Err(e) = config.save(std::path::Path::new("config.toml")) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save config: {}", e),
        )
            .into_response();
    }

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
}

async fn get_hardware_settings_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(HardwareSettingsPayload {
        allow_cpu_fallback: config.hardware.allow_cpu_fallback,
        allow_cpu_encoding: config.hardware.allow_cpu_encoding,
        cpu_preset: config.hardware.cpu_preset.to_string(),
        preferred_vendor: config.hardware.preferred_vendor.clone(),
    })
}

async fn update_hardware_settings_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<HardwareSettingsPayload>,
) -> impl IntoResponse {
    let mut config = state.config.write().await;

    config.hardware.allow_cpu_fallback = payload.allow_cpu_fallback;
    config.hardware.allow_cpu_encoding = payload.allow_cpu_encoding;
    config.hardware.cpu_preset = match payload.cpu_preset.to_lowercase().as_str() {
        "slow" => crate::config::CpuPreset::Slow,
        "medium" => crate::config::CpuPreset::Medium,
        "fast" => crate::config::CpuPreset::Fast,
        "faster" => crate::config::CpuPreset::Faster,
        _ => crate::config::CpuPreset::Medium,
    };
    config.hardware.preferred_vendor = payload.preferred_vendor;

    if let Err(e) = config.save(std::path::Path::new("config.toml")) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save config: {}", e),
        )
            .into_response();
    }

    StatusCode::OK.into_response()
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SystemSettingsPayload {
    monitoring_poll_interval: f64,
    enable_telemetry: bool,
}

async fn get_system_settings_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    axum::Json(SystemSettingsPayload {
        monitoring_poll_interval: config.system.monitoring_poll_interval,
        enable_telemetry: config.system.enable_telemetry,
    })
}

async fn update_system_settings_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SystemSettingsPayload>,
) -> impl IntoResponse {
    let mut config = state.config.write().await;

    if payload.monitoring_poll_interval < 0.5 || payload.monitoring_poll_interval > 10.0 {
        return (
            StatusCode::BAD_REQUEST,
            "monitoring_poll_interval must be between 0.5 and 10.0 seconds",
        )
            .into_response();
    }

    config.system.monitoring_poll_interval = payload.monitoring_poll_interval;
    config.system.enable_telemetry = payload.enable_telemetry;

    if let Err(e) = config.save(std::path::Path::new("config.toml")) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save config: {}", e),
        )
            .into_response();
    }

    (StatusCode::OK, "Settings updated").into_response()
}

#[derive(serde::Deserialize)]
struct SetupConfig {
    username: String,
    password: String,
    size_reduction_threshold: f64,
    min_file_size_mb: u64,
    concurrent_jobs: usize,
    directories: Vec<String>,
    allow_cpu_encoding: bool,
    enable_telemetry: bool,
}

async fn setup_complete_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<IncludeTokenQuery>,
    axum::Json(payload): axum::Json<SetupConfig>,
) -> impl IntoResponse {
    if !state.setup_required.load(Ordering::Relaxed) {
        return (StatusCode::FORBIDDEN, "Setup already completed").into_response();
    }

    // Create User
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

    let user_id = match state
        .db
        .create_user(&payload.username, &password_hash)
        .await
    {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create user: {}", e),
            )
                .into_response()
        }
    };

    // Create Initial Session
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

    // Save Config
    let mut config_lock = state.config.write().await;
    let mut next_config = config_lock.clone();
    next_config.transcode.concurrent_jobs = payload.concurrent_jobs;
    next_config.transcode.size_reduction_threshold = payload.size_reduction_threshold;
    next_config.transcode.min_file_size_mb = payload.min_file_size_mb;
    next_config.hardware.allow_cpu_encoding = payload.allow_cpu_encoding;
    next_config.scanner.directories = payload.directories;
    next_config.system.enable_telemetry = payload.enable_telemetry;

    if let Err(e) = next_config.validate() {
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    *config_lock = next_config;

    // Serialize to TOML
    let toml_string = match toml::to_string_pretty(&*config_lock) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to serialize config: {}", e),
            )
                .into_response()
        }
    };
    drop(config_lock);

    // Write to file
    if let Err(e) = std::fs::write("config.toml", toml_string) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to write config.toml: {}", e),
        )
            .into_response();
    }

    // Update Setup State (Hot Reload)
    state.setup_required.store(false, Ordering::Relaxed);
    state
        .agent
        .set_concurrent_jobs(payload.concurrent_jobs)
        .await;
    state.agent.resume();
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
    let include_token = query.include_token.unwrap_or(false);
    let body = if include_token {
        serde_json::json!({ "status": "saved", "token": token })
    } else {
        serde_json::json!({ "status": "saved" })
    };
    (
        [(header::SET_COOKIE, cookie)],
        axum::Json(body),
    )
        .into_response()
}

#[derive(serde::Deserialize, serde::Serialize)]
struct UiPreferences {
    active_theme_id: Option<String>,
}

async fn get_preferences_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let active_theme_id = state
        .db
        .get_preference("active_theme_id")
        .await
        .unwrap_or(None);
    axum::Json(UiPreferences { active_theme_id })
}

async fn update_preferences_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<UiPreferences>,
) -> impl IntoResponse {
    if let Some(theme_id) = payload.active_theme_id {
        if let Err(e) = state.db.set_preference("active_theme_id", &theme_id).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save preference: {}", e),
            )
                .into_response();
        }
    }
    StatusCode::OK.into_response()
}

async fn index_handler() -> impl IntoResponse {
    static_handler(Uri::from_static("/index.html")).await
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    if let Some(content) = Assets::get(&path) {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response();
    }

    // Attempt to serve index.html for directory paths (e.g. /jobs -> jobs/index.html)
    if !path.contains('.') {
        let index_path = format!("{}/index.html", path);
        if let Some(content) = Assets::get(&index_path) {
            let mime = mime_guess::from_path("index.html").first_or_octet_stream();
            return ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response();
        }
    }

    // Default fallback to 404 for missing files, except for the SPA root fallback if intended.
    // Given we are using Astro as SSG for these pages, if it's not found, it's a 404.
    StatusCode::NOT_FOUND.into_response()
}

struct StatsData {
    total: i64,
    completed: i64,
    active: i64,
    failed: i64,
    concurrent_limit: usize,
}

async fn get_stats_data(db: &Db, config: &Config) -> StatsData {
    let s = db
        .get_stats()
        .await
        .unwrap_or_else(|_| serde_json::json!({}));
    let total = s
        .as_object()
        .map(|m| m.values().filter_map(|v| v.as_i64()).sum::<i64>())
        .unwrap_or(0);
    let completed = s.get("completed").and_then(|v| v.as_i64()).unwrap_or(0);
    let active = s
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| ["encoding", "analyzing", "resuming"].contains(&k.as_str()))
                .map(|(_, v)| v.as_i64().unwrap_or(0))
                .sum::<i64>()
        })
        .unwrap_or(0);
    let failed = s.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);

    StatsData {
        total,
        completed,
        active,
        failed,
        concurrent_limit: config.transcode.concurrent_jobs,
    }
}

async fn stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    let stats = get_stats_data(&state.db, &config).await;
    axum::Json(serde_json::json!({
        "total": stats.total,
        "completed": stats.completed,
        "active": stats.active,
        "failed": stats.failed,
        "concurrent_limit": stats.concurrent_limit
    }))
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
        }
        Err(_) => axum::Json(serde_json::json!({
            "total_input_bytes": 0,
            "total_output_bytes": 0,
            "total_savings_bytes": 0,
            "total_time_seconds": 0,
            "total_jobs": 0,
            "avg_vmaf": 0.0
        })),
    }
}

async fn daily_stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_daily_stats(30).await {
        Ok(stats) => axum::Json(serde_json::json!(stats)).into_response(),
        Err(_) => axum::Json(serde_json::json!([])).into_response(),
    }
}

async fn detailed_stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_detailed_encode_stats(50).await {
        Ok(stats) => axum::Json(serde_json::json!(stats)).into_response(),
        Err(_) => axum::Json(serde_json::json!([])).into_response(),
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
    if state.transcoder.cancel_job(id) {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn restart_failed_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let _ = state.db.restart_failed_jobs().await;
    StatusCode::OK
}

async fn clear_completed_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let _ = state.db.clear_completed_jobs().await;
    StatusCode::OK
}

async fn pause_engine_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.agent.pause();
    axum::Json(serde_json::json!({ "status": "paused" }))
}

async fn resume_engine_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.agent.resume();
    axum::Json(serde_json::json!({ "status": "running" }))
}

async fn engine_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": if state.agent.is_paused() { "paused" } else { "running" }
    }))
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

#[derive(serde::Deserialize)]
struct LoginPayload {
    username: String,
    password: String,
}

#[derive(serde::Deserialize)]
struct IncludeTokenQuery {
    include_token: Option<bool>,
}

async fn login_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(query): Query<IncludeTokenQuery>,
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
    let include_token = query.include_token.unwrap_or(false);
    let body = if include_token {
        serde_json::json!({ "token": token })
    } else {
        serde_json::json!({ "status": "ok" })
    };
    (
        [(header::SET_COOKIE, cookie)],
        axum::Json(body),
    )
        .into_response()
}

async fn logout_handler(State(state): State<Arc<AppState>>, req: Request) -> impl IntoResponse {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|auth_str| {
            if auth_str.starts_with("Bearer ") {
                Some(auth_str[7..].to_string())
            } else {
                None
            }
        })
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

        // Protected API endpoints -> Require Token
        let mut token = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|auth_str| {
                if auth_str.starts_with("Bearer ") {
                    Some(auth_str[7..].to_string())
                } else {
                    None
                }
            });

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
    if !allow_global_request(&state).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Too many requests").into_response();
    }
    next.run(req).await
}

async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = std::result::Result<AxumEvent, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event) => {
            let (event_name, data) = match &event {
                AlchemistEvent::Log {
                    level,
                    job_id,
                    message,
                } => (
                    "log",
                    serde_json::json!({
                        "level": level,
                        "job_id": job_id,
                        "message": message
                    })
                    .to_string(),
                ),
                AlchemistEvent::Progress {
                    job_id,
                    percentage,
                    time,
                } => (
                    "progress",
                    serde_json::json!({
                        "job_id": job_id,
                        "percentage": percentage,
                        "time": time
                    })
                    .to_string(),
                ),
                AlchemistEvent::JobStateChanged { job_id, status } => (
                    "status",
                    serde_json::json!({
                        "job_id": job_id,
                        "status": status // Uses serde impl (lowercase)
                    })
                    .to_string(),
                ),
                AlchemistEvent::Decision {
                    job_id,
                    action,
                    reason,
                } => (
                    "decision",
                    serde_json::json!({
                        "job_id": job_id,
                        "action": action,
                        "reason": reason
                    })
                    .to_string(),
                ),
            };
            Some(Ok(AxumEvent::default().event(event_name).data(data)))
        }
        Err(_) => None,
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
    // Use a block to limit the scope of the lock
    let (cpu_percent, memory_used_mb, memory_total_mb, memory_percent, cpu_count) = {
        let mut sys = match state.sys.lock() {
            Ok(sys) => sys,
            Err(e) => {
                error!("System monitor lock poisoned: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "System monitor unavailable")
                    .into_response();
            }
        };
        // Full refresh for better accuracy when polled less frequently
        sys.refresh_all();

        // Get CPU usage (average across all cores)
        let cpu_percent =
            sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32;

        let cpu_count = sys.cpus().len();

        // Memory info
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

    // Uptime
    let uptime_seconds = state.start_time.elapsed().as_secs();

    // Active jobs from database
    let stats = state.db.get_job_stats().await.unwrap_or_default();
    let config = state.config.read().await;

    // Query GPU utilization (using spawn_blocking to avoid blocking)
    let (gpu_utilization, gpu_memory_percent) =
        tokio::task::spawn_blocking(|| query_gpu_utilization())
            .await
            .unwrap_or((None, None));

    axum::Json(SystemResources {
        cpu_percent,
        memory_used_mb,
        memory_total_mb,
        memory_percent,
        uptime_seconds,
        active_jobs: stats.active,
        concurrent_limit: config.transcode.concurrent_jobs,
        cpu_count,
        gpu_utilization,
        gpu_memory_percent,
    })
    .into_response()
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
    let limit = params.limit.unwrap_or(50).min(200);
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
    sort_desc: Option<bool>,
}

async fn jobs_table_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<JobTableParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(200);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let statuses = if let Some(s) = params.status {
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

    match state
        .db
        .get_jobs_filtered(
            limit,
            offset,
            statuses,
            params.search,
            params.sort,
            params.sort_desc.unwrap_or(false),
        )
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
    let result = match payload.action.as_str() {
        "cancel" => state.db.batch_cancel_jobs(&payload.ids).await,
        "delete" => state.db.batch_delete_jobs(&payload.ids).await,
        "restart" => state.db.batch_restart_jobs(&payload.ids).await,
        _ => return (StatusCode::BAD_REQUEST, "Invalid action").into_response(),
    };

    match result {
        Ok(count) => axum::Json(serde_json::json!({ "count": count })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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
    if let Err(msg) = validate_notification_url(&payload.endpoint_url) {
        return (StatusCode::BAD_REQUEST, msg).into_response();
    }

    let events_json = serde_json::to_string(&payload.events).unwrap_or_default();
    match state
        .db
        .add_notification_target(
            &payload.name,
            &payload.target_type,
            &payload.endpoint_url,
            payload.auth_token.as_deref(),
            &events_json,
            payload.enabled,
        )
        .await
    {
        Ok(t) => axum::Json(serde_json::json!(t)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_notification_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.delete_notification_target(id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn test_notification_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<AddNotificationTargetPayload>,
) -> impl IntoResponse {
    if let Err(msg) = validate_notification_url(&payload.endpoint_url) {
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
        || payload
            .days_of_week
            .iter()
            .any(|day| *day < 0 || *day > 6)
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

    let days_json = serde_json::to_string(&payload.days_of_week).unwrap_or_default();
    match state
        .db
        .add_schedule_window(
            &start_time,
            &end_time,
            &days_json,
            payload.enabled,
        )
        .await
    {
        Ok(w) => axum::Json(serde_json::json!(w)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_schedule_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.delete_schedule_window(id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct AddWatchDirPayload {
    path: String,
    is_recursive: Option<bool>,
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
    match state
        .db
        .add_watch_dir(&payload.path, payload.is_recursive.unwrap_or(true))
        .await
    {
        Ok(dir) => {
            refresh_file_watcher(&state).await;
            axum::Json(dir).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn remove_watch_dir_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.remove_watch_dir(id).await {
        Ok(_) => {
            refresh_file_watcher(&state).await;
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn restart_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.get_job_by_id(id).await {
        Ok(Some(job)) => {
            if let Err(e) = state
                .db
                .update_job_status(job.id, crate::db::JobState::Queued)
                .await
            {
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
    match state.db.delete_job(id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Serialize)]
struct JobDetailResponse {
    job: crate::db::Job,
    metadata: Option<crate::media::pipeline::MediaMetadata>,
    encode_stats: Option<crate::db::DetailedEncodeStats>,
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

    // Try to get metadata
    let analyzer = crate::media::analyzer::FfmpegAnalyzer;
    use crate::media::pipeline::Analyzer;
    let metadata = analyzer
        .analyze(std::path::Path::new(&job.input_path))
        .await
        .ok();

    // Try to get encode stats (using the subquery result or a specific query)
    // For now we'll just query the encode_stats table if completed
    let encode_stats = if job.status == crate::db::JobState::Completed {
        state.db.get_encode_stats_by_job_id(id).await.ok()
    } else {
        None
    };

    axum::Json(JobDetailResponse {
        job,
        metadata,
        encode_stats,
    })
    .into_response()
}

async fn get_file_settings_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_file_settings().await {
        Ok(s) => axum::Json(serde_json::json!(s)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct UpdateFileSettingsPayload {
    delete_source: bool,
    output_extension: String,
    output_suffix: String,
    replace_strategy: String,
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

    match state
        .db
        .update_file_settings(
            payload.delete_source,
            &payload.output_extension,
            &payload.output_suffix,
            &payload.replace_strategy,
        )
        .await
    {
        Ok(s) => axum::Json(serde_json::json!(s)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
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
                return (StatusCode::INTERNAL_SERVER_ERROR, "System monitor unavailable")
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
    let stats = state.db.get_job_stats().await.unwrap_or_default();

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
    let config = state.config.read().await;
    match crate::system::hardware::detect_hardware_async(config.hardware.allow_cpu_fallback).await {
        Ok(info) => axum::Json(info).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
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

async fn allow_global_request(state: &AppState) -> bool {
    let mut entry = state.global_rate_limiter.lock().await;
    let now = Instant::now();

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
    format!(
        "alchemist_session={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000",
        token
    )
}

fn build_clear_session_cookie() -> String {
    "alchemist_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0".to_string()
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

fn validate_notification_url(raw: &str) -> std::result::Result<(), &'static str> {
    let url = Url::parse(raw).map_err(|_| "endpoint_url must be a valid URL")?;
    match url.scheme() {
        "http" | "https" => {}
        _ => return Err("endpoint_url must use http or https"),
    }

    let host = url
        .host_str()
        .ok_or("endpoint_url must include a host")?;

    if host.eq_ignore_ascii_case("localhost") {
        return Err("endpoint_url host is not allowed");
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) {
            return Err("endpoint_url host is not allowed");
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
