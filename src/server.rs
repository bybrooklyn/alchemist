use crate::config::Config;
use crate::db::{AlchemistEvent, Db, JobState};
use crate::error::Result;
use crate::Agent;
use crate::Transcoder;
use axum::{
    extract::{Path, Request, State},
    http::{header, StatusCode, Uri},
    middleware::{self, Next},
    response::{
        sse::{Event as AxumEvent, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Router,
};
use futures::stream::Stream;
use rust_embed::RustEmbed;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::info;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::rngs::OsRng;
use rand::Rng;
use std::sync::atomic::{AtomicBool, Ordering};
use chrono::Utc;

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
}

pub async fn run_server(
    db: Arc<Db>,
    config: Arc<RwLock<Config>>,
    agent: Arc<Agent>,
    transcoder: Arc<Transcoder>,
    tx: broadcast::Sender<AlchemistEvent>,
    setup_required: bool,
) -> Result<()> {
    let state = Arc::new(AppState {
        db,
        config,
        agent,
        transcoder,
        tx,
        setup_required: Arc::new(AtomicBool::new(setup_required)),
        start_time: std::time::Instant::now(),
    });

    let app = Router::new()
        // API Routes
        .route("/api/scan", post(scan_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/stats/aggregated", get(aggregated_stats_handler))
        .route("/api/jobs/table", get(jobs_table_handler))
        .route("/api/jobs/restart-failed", post(restart_failed_handler))
        .route("/api/jobs/clear-completed", post(clear_completed_handler))
        .route("/api/jobs/:id/cancel", post(cancel_job_handler))
        .route("/api/jobs/:id/restart", post(restart_job_handler))
        .route("/api/events", get(sse_handler))
        .route("/api/engine/pause", post(pause_engine_handler))
        .route("/api/engine/resume", post(resume_engine_handler))
        .route("/api/engine/status", get(engine_status_handler))
        .route(
            "/api/settings/transcode",
            get(get_transcode_settings_handler).post(update_transcode_settings_handler),
        )
        // Health Check Routes
        .route("/api/health", get(health_handler))
        .route("/api/ready", get(ready_handler))
        // Setup Routes
        .route("/api/setup/status", get(setup_status_handler))
        .route("/api/setup/complete", post(setup_complete_handler))
        .route("/api/auth/login", post(login_handler))
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
        .with_state(state);

    let addr = "0.0.0.0:3000";
    info!("listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn setup_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "setup_required": state.setup_required.load(Ordering::Relaxed)
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
         return (StatusCode::BAD_REQUEST, "size_reduction_threshold must be 0.0-1.0").into_response();
    }

    config.transcode.concurrent_jobs = payload.concurrent_jobs;
    config.transcode.size_reduction_threshold = payload.size_reduction_threshold;
    config.transcode.min_bpp_threshold = payload.min_bpp_threshold;
    config.transcode.min_file_size_mb = payload.min_file_size_mb;
    config.transcode.output_codec = payload.output_codec;
    config.transcode.quality_profile = payload.quality_profile;

    if let Err(e) = config.save(std::path::Path::new("config.toml")) {
         return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save config: {}", e),
        )
            .into_response();
    }

    StatusCode::OK.into_response()
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
}

async fn setup_complete_handler(
    State(state): State<Arc<AppState>>,
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
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Hashing failed: {}", e)).into_response(),
    };

    let user_id = match state.db.create_user(&payload.username, &password_hash).await {
         Ok(id) => id,
         Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create user: {}", e)).into_response(),
    };

    // Create Initial Session
    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let expires_at = Utc::now() + chrono::Duration::days(30);
    
    if let Err(e) = state.db.create_session(user_id, &token, expires_at).await {
         return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create session: {}", e)).into_response();
    }

    // Save Config
    let mut config_lock = state.config.write().await;
    config_lock.transcode.concurrent_jobs = payload.concurrent_jobs;
    config_lock.transcode.size_reduction_threshold = payload.size_reduction_threshold;
    config_lock.transcode.min_file_size_mb = payload.min_file_size_mb;
    config_lock.hardware.allow_cpu_encoding = payload.allow_cpu_encoding;
    config_lock.scanner.directories = payload.directories;

    // Serialize to TOML
    let toml_string = match toml::to_string_pretty(&*config_lock) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to serialize config: {}", e)).into_response(),
    };

    // Write to file
    if let Err(e) = std::fs::write("config.toml", toml_string) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write config.toml: {}", e)).into_response();
    }
    
    // Update Setup State (Hot Reload)
    state.setup_required.store(false, Ordering::Relaxed);
    
    // Start Scan (optional, but good for UX)
    let dirs = config_lock.scanner.directories.iter().map(std::path::PathBuf::from).collect();
    let _ = state.agent.scan_and_enqueue(dirs).await;

    info!("Configuration saved via web setup. Auth info created.");

    axum::Json(serde_json::json!({ "status": "saved", "token": token })).into_response()
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

async fn jobs_table_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let jobs = state.db.get_all_jobs().await.unwrap_or_default();
    axum::Json(jobs)
}

async fn scan_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    let dirs = config
        .scanner
        .directories
        .iter()
        .map(std::path::PathBuf::from)
        .collect();
    drop(config); // Release lock before awaiting scan (though scan might take long time? no scan_and_enqueue is async but returns quickly? Let's check Agent::scan_and_enqueue)
                  // Agent::scan_and_enqueue is async. We should probably release lock before calling it if it takes long time.
                  // It does 'Scanner::new().scan()' which IS synchronous and blocking?
                  // Looking at Agent::scan_and_enqueue: `let files = scanner.scan(directories);`
                  // If scanner.scan is slow, we are holding the config lock? No, I dropped it.

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

async fn restart_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let _ = state.db.update_job_status(id, JobState::Queued).await;
    StatusCode::OK
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

async fn login_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<LoginPayload>,
) -> impl IntoResponse {
    let user = match state.db.get_user_by_username(&payload.username).await {
        Ok(Some(u)) => u,
        _ => return (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response(),
    };

    let parsed_hash = match PasswordHash::new(&user.password_hash) {
        Ok(h) => h,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Invalid hash format").into_response(),
    };

    if Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response();
    }

    // Create session
    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let expires_at = Utc::now() + chrono::Duration::days(30);

    if let Err(e) = state.db.create_session(user.id, &token, expires_at).await {
         return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create session: {}", e)).into_response();
    }

    axum::Json(serde_json::json!({ "token": token })).into_response()
}

async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();

    // 1. API Protection: Only lock down /api routes
    if path.starts_with("/api") {
        // Public API endpoints
        if path.starts_with("/api/setup") || path.starts_with("/api/auth/login") {
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

        // Fallback: Check query param "token" (for EventSource which can't set headers)
        if token.is_none() {
            if let Some(query) = req.uri().query() {
                if let Ok(params) = serde_urlencoded::from_str::<std::collections::HashMap<String, String>>(query) {
                    if let Some(t) = params.get("token") {
                         token = Some(t.clone());
                    }
                }
            }
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

async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = std::result::Result<AxumEvent, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event) => {
            let (event_name, data) = match &event {
                AlchemistEvent::Log { message, .. } => ("log", message.clone()),
                AlchemistEvent::Progress {
                    job_id,
                    percentage,
                    time,
                } => (
                    "progress",
                    format!(
                        "{{\"job_id\": {}, \"percentage\": {:.1}, \"time\": \"{}\"}}",
                        job_id, percentage, time
                    ),
                ),
                AlchemistEvent::JobStateChanged { job_id, status } => (
                    "status",
                    format!("{{\"job_id\": {}, \"status\": \"{:?}\"}}", job_id, status),
                ),
                AlchemistEvent::Decision {
                    job_id,
                    action,
                    reason,
                } => (
                    "decision",
                    format!(
                        "{{\"job_id\": {}, \"action\": \"{}\", \"reason\": \"{}\"}}",
                        job_id, action, reason
                    ),
                ),
            };
            Some(Ok(AxumEvent::default().event(event_name).data(data)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}
