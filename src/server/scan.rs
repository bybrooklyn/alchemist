//! Library scanning and watch folder handlers.

use super::{AppState, is_row_not_found, refresh_file_watcher, save_config_or_response};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use futures::{FutureExt, StreamExt, stream};
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;

pub(crate) async fn scan_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
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

pub(crate) async fn start_scan_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.library_scanner.start_scan().await {
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub(crate) async fn get_scan_status_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    axum::Json::<crate::system::scanner::ScanStatus>(state.library_scanner.get_status().await)
        .into_response()
}

// Library health handlers

#[derive(Serialize)]
struct LibraryHealthIssueResponse {
    job: crate::db::Job,
    report: crate::media::health::HealthIssueReport,
}

pub(crate) async fn library_health_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_health_summary().await {
        Ok(summary) => axum::Json(summary).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub(crate) async fn get_library_health_issues_handler(
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

async fn run_library_health_scan(db: Arc<crate::db::Db>) {
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

pub(crate) async fn start_library_health_scan_handler(
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

pub(crate) async fn rescan_library_health_issue_handler(
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

// Watch directories handlers

#[derive(Deserialize)]
pub(crate) struct AddWatchDirPayload {
    path: String,
    is_recursive: Option<bool>,
}

pub(crate) async fn get_watch_dirs_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_watch_dirs().await {
        Ok(dirs) => axum::Json(dirs).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub(crate) async fn add_watch_dir_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<AddWatchDirPayload>,
) -> impl IntoResponse {
    let normalized_path = match super::canonicalize_directory_path(&payload.path, "path") {
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

pub(crate) async fn remove_watch_dir_handler(
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

// Library profiles handlers

#[derive(Serialize)]
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

#[derive(Deserialize)]
pub(crate) struct LibraryProfilePayload {
    name: String,
    preset: String,
    codec: String,
    quality_profile: String,
    hdr_mode: String,
    audio_mode: String,
    crf_override: Option<i32>,
    notes: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct AssignWatchDirProfilePayload {
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

pub(crate) async fn list_profiles_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
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

pub(crate) async fn get_profile_presets_handler() -> impl IntoResponse {
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

pub(crate) async fn create_profile_handler(
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

pub(crate) async fn update_profile_handler(
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

pub(crate) async fn delete_profile_handler(
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

pub(crate) async fn assign_watch_dir_profile_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    axum::Json(payload): axum::Json<AssignWatchDirProfilePayload>,
) -> impl IntoResponse {
    if let Some(profile_id) = payload.profile_id {
        match state.db.get_profile(profile_id).await {
            Ok(Some(_)) => {}
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(err) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
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
