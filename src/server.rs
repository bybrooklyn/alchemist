use crate::config::Config;
use crate::db::{AlchemistEvent, Db, Job, JobState};
use crate::error::Result;
use crate::Agent;
use crate::Transcoder;
use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{Path, State},
    response::sse::{Event as AxumEvent, Sse},
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
#[folder = "public/"]
struct Assets;

pub struct AppState {
    pub db: Arc<Db>,
    pub config: Arc<Config>,
    pub agent: Arc<Agent>,
    pub transcoder: Arc<Transcoder>,
    pub tx: broadcast::Sender<AlchemistEvent>,
}

pub async fn run_server(
    db: Arc<Db>,
    config: Arc<Config>,
    agent: Arc<Agent>,
    transcoder: Arc<Transcoder>,
    tx: broadcast::Sender<AlchemistEvent>,
) -> Result<()> {
    let state = Arc::new(AppState {
        db,
        config,
        agent,
        transcoder,
        tx,
    });

    let app = Router::new()
        .route("/", get(dashboard_handler))
        .route("/settings", get(settings_handler))
        .route("/api/scan", post(scan_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/jobs/table", get(jobs_table_handler))
        .route("/api/jobs/:id/cancel", post(cancel_job_handler))
        .route("/api/jobs/:id/restart", post(restart_job_handler))
        .route("/api/events", get(sse_handler))
        .route("/assets/*file", get(static_handler))
        .with_state(state);

    let addr = "127.0.0.1:3000";
    info!("listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    active_page: &'static str,
    stats: StatsData,
    jobs: Vec<Job>,
}

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
    active_page: &'static str,
    config: Arc<Config>,
}

#[derive(Template)]
#[template(path = "partials/stats.html")]
struct StatsPartialTemplate {
    stats: StatsData,
}

#[derive(Template)]
#[template(path = "partials/jobs_table.html")]
struct JobsTablePartialTemplate {
    jobs: Vec<Job>,
}

struct StatsData {
    total: i64,
    completed: i64,
    active: i64,
    failed: i64,
}

async fn get_stats_data(db: &Db) -> StatsData {
    let s = db.get_stats().await.unwrap_or_else(|_| serde_json::json!({}));
    let total = s.as_object().map(|m| m.values().filter_map(|v| v.as_i64()).sum::<i64>()).unwrap_or(0);
    let completed = s.get("completed").and_then(|v| v.as_i64()).unwrap_or(0);
    let active = s.as_object().map(|m| m.iter().filter(|(k, _)| ["encoding", "analyzing", "resuming"].contains(&k.as_str())).map(|(_, v)| v.as_i64().unwrap_or(0)).sum::<i64>()).unwrap_or(0);
    let failed = s.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);

    StatsData { total, completed, active, failed }
}

async fn dashboard_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let stats = get_stats_data(&state.db).await;
    let jobs = state.db.get_all_jobs().await.unwrap_or_default();
    DashboardTemplate {
        active_page: "dashboard",
        stats,
        jobs,
    }
}

async fn settings_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    SettingsTemplate {
        active_page: "settings",
        config: state.config.clone(),
    }
}

async fn scan_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let dirs = state.config.scanner.directories.iter().map(std::path::PathBuf::from).collect();
    let _ = state.agent.scan_and_enqueue(dirs).await;
    axum::http::StatusCode::OK
}

async fn stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let stats = get_stats_data(&state.db).await;
    StatsPartialTemplate { stats }
}

async fn jobs_table_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let jobs = state.db.get_all_jobs().await.unwrap_or_default();
    JobsTablePartialTemplate { jobs }
}

async fn cancel_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if state.transcoder.cancel_job(id) {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::NOT_FOUND
    }
}

async fn restart_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let _ = state.db.update_job_status(id, JobState::Queued).await;
    axum::http::StatusCode::OK
}

async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = std::result::Result<AxumEvent, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event) => {
            let json = serde_json::to_string(&event).ok()?;
            Some(Ok(AxumEvent::default().data(json)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

async fn static_handler(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                content.data,
            ).into_response()
        }
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}
