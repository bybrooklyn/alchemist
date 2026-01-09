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
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::info;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

pub struct AppState {
    pub db: Arc<Db>,
    pub config: Arc<Config>,
    pub agent: Arc<Agent>,
    pub transcoder: Arc<Transcoder>,
    pub tx: broadcast::Sender<AlchemistEvent>,
    pub setup_required: bool,
}

pub async fn run_server(
    db: Arc<Db>,
    config: Arc<Config>,
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
        setup_required,
    });

    let app = Router::new()
        // API Routes
        .route("/api/scan", post(scan_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/jobs/table", get(jobs_table_handler))
        .route("/api/jobs/restart-failed", post(restart_failed_handler))
        .route("/api/jobs/clear-completed", post(clear_completed_handler))
        .route("/api/jobs/:id/cancel", post(cancel_job_handler))
        .route("/api/jobs/:id/restart", post(restart_job_handler))
        .route("/api/events", get(sse_handler))
        .route("/api/engine/pause", post(pause_engine_handler))
        .route("/api/engine/resume", post(resume_engine_handler))
        .route("/api/engine/status", get(engine_status_handler))
        // Setup Routes
        .route("/api/setup/status", get(setup_status_handler))
        .route("/api/setup/complete", post(setup_complete_handler))
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
        "setup_required": state.setup_required
    }))
}

#[derive(serde::Deserialize)]
struct SetupConfig {
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
    if !state.setup_required {
        return (StatusCode::FORBIDDEN, "Setup already completed").into_response();
    }

    // Create config object
    let mut config = Config::default();
    config.transcode.concurrent_jobs = payload.concurrent_jobs;
    config.transcode.size_reduction_threshold = payload.size_reduction_threshold;
    config.transcode.min_file_size_mb = payload.min_file_size_mb;
    config.hardware.allow_cpu_encoding = payload.allow_cpu_encoding;
    config.scanner.directories = payload.directories;

    // Serialize to TOML
    let toml_string = match toml::to_string_pretty(&config) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to serialize config: {}", e),
            )
                .into_response()
        }
    };

    // Write to file
    if let Err(e) = std::fs::write("config.toml", toml_string) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to write config.toml: {}", e),
        )
            .into_response();
    }

    info!("Configuration saved via web setup. Restarting recommended.");

    axum::Json(serde_json::json!({ "status": "saved" })).into_response()
}

async fn index_handler() -> impl IntoResponse {
    static_handler(Uri::from_static("/index.html")).await
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => {
            if path.contains('.') {
                StatusCode::NOT_FOUND.into_response()
            } else {
                // Fallback to index.html for client-side routing if we add it later
                // For now, it might be better to 404 if we are strict MPA
                // But let's try to serve index.html if it's a route
                match Assets::get("index.html") {
                    Some(content) => {
                        let mime = mime_guess::from_path("index.html").first_or_octet_stream();
                        ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
                    }
                    None => StatusCode::NOT_FOUND.into_response(),
                }
            }
        }
    }
}

struct StatsData {
    total: i64,
    completed: i64,
    active: i64,
    failed: i64,
}

async fn get_stats_data(db: &Db) -> StatsData {
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
    }
}

async fn stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let stats = get_stats_data(&state.db).await;
    axum::Json(serde_json::json!({
        "total": stats.total,
        "completed": stats.completed,
        "active": stats.active,
        "failed": stats.failed
    }))
}

async fn jobs_table_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let jobs = state.db.get_all_jobs().await.unwrap_or_default();
    axum::Json(jobs)
}

async fn scan_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let dirs = state
        .config
        .scanner
        .directories
        .iter()
        .map(std::path::PathBuf::from)
        .collect();
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

async fn auth_middleware(
    State(_state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();
    // Allow setup routes without auth
    if path.starts_with("/api/setup") {
        return next.run(req).await;
    }

    if let Ok(password) = std::env::var("ALCHEMIST_PASSWORD") {
        if !password.is_empty() {
            // For static assets, we might want to bypass auth or require cookie auth
            // For now, implementing simple bearer token check from original code
            // NOTE: Browser won't send Bearer token for initial page load naturally.
            // We might need to move to Cookie auth or allow basic auth.
            // But for now, let's keep it as is or allow "/" to be public if needed?
            // The user didn't specify auth changes. I'll leave the middleware applied globally.

            let authorized = req
                .headers()
                .get("Authorization")
                .and_then(|h| h.to_str().ok())
                .map(|s| s == format!("Bearer {}", password))
                .unwrap_or(false);

            if !authorized {
                // If requesting HTML, maybe return 401 asking for auth?
                // Or just 401.
                return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
            }
        }
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
