//! Job CRUD, batch operations, queue control handlers.

use super::{AppState, is_row_not_found};
use crate::db::{Job, JobState};
use crate::error::Result;
use crate::explanations::Explanation;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::SystemTime,
};

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

#[derive(Deserialize)]
pub(crate) struct EnqueueJobPayload {
    path: String,
}

#[derive(Serialize)]
pub(crate) struct EnqueueJobResponse {
    enqueued: bool,
    message: String,
}

pub(crate) fn blocked_jobs_response(message: impl Into<String>, blocked: &[Job]) -> Response {
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

fn resolve_source_root(path: &FsPath, watch_dirs: &[crate::db::WatchDir]) -> Option<PathBuf> {
    watch_dirs
        .iter()
        .map(|watch_dir| PathBuf::from(&watch_dir.path))
        .filter(|watch_dir| path.starts_with(watch_dir))
        .max_by_key(|watch_dir| watch_dir.components().count())
}

async fn purge_resume_sessions_for_jobs(state: &AppState, ids: &[i64]) {
    let sessions = match state.db.get_resume_sessions_by_job_ids(ids).await {
        Ok(sessions) => sessions,
        Err(err) => {
            tracing::warn!("Failed to load resume sessions for purge: {}", err);
            return;
        }
    };

    for session in sessions {
        if let Err(err) = state.db.delete_resume_session(session.job_id).await {
            tracing::warn!(
                job_id = session.job_id,
                "Failed to delete resume session rows: {err}"
            );
            continue;
        }

        let temp_dir = PathBuf::from(&session.temp_dir);
        if temp_dir.exists() {
            if let Err(err) = tokio::fs::remove_dir_all(&temp_dir).await {
                tracing::warn!(
                    job_id = session.job_id,
                    path = %temp_dir.display(),
                    "Failed to remove resume temp dir: {err}"
                );
            }
        }
    }
}

pub(crate) async fn enqueue_job_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<EnqueueJobPayload>,
) -> impl IntoResponse {
    let submitted_path = payload.path.trim();
    if submitted_path.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(EnqueueJobResponse {
                enqueued: false,
                message: "Path must not be empty.".to_string(),
            }),
        )
            .into_response();
    }

    let requested_path = PathBuf::from(submitted_path);
    if !requested_path.is_absolute() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(EnqueueJobResponse {
                enqueued: false,
                message: "Path must be absolute.".to_string(),
            }),
        )
            .into_response();
    }

    let canonical_path = match std::fs::canonicalize(&requested_path) {
        Ok(path) => path,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                axum::Json(EnqueueJobResponse {
                    enqueued: false,
                    message: format!("Unable to resolve path: {err}"),
                }),
            )
                .into_response();
        }
    };

    let metadata = match std::fs::metadata(&canonical_path) {
        Ok(metadata) => metadata,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                axum::Json(EnqueueJobResponse {
                    enqueued: false,
                    message: format!("Unable to read file metadata: {err}"),
                }),
            )
                .into_response();
        }
    };
    if !metadata.is_file() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(EnqueueJobResponse {
                enqueued: false,
                message: "Path must point to a file.".to_string(),
            }),
        )
            .into_response();
    }

    let extension = canonical_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let supported = crate::media::scanner::Scanner::new().extensions;
    if extension
        .as_deref()
        .is_none_or(|value| !supported.iter().any(|candidate| candidate == value))
    {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(EnqueueJobResponse {
                enqueued: false,
                message: "File type is not supported for enqueue.".to_string(),
            }),
        )
            .into_response();
    }

    let watch_dirs = match state.db.get_watch_dirs().await {
        Ok(watch_dirs) => watch_dirs,
        Err(err) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
        }
    };

    let discovered = crate::media::pipeline::DiscoveredMedia {
        path: canonical_path.clone(),
        mtime: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        source_root: resolve_source_root(&canonical_path, &watch_dirs),
    };

    match crate::media::pipeline::enqueue_discovered_with_db(state.db.as_ref(), discovered).await {
        Ok(true) => (
            StatusCode::OK,
            axum::Json(EnqueueJobResponse {
                enqueued: true,
                message: format!("Enqueued {}.", canonical_path.display()),
            }),
        )
            .into_response(),
        Ok(false) => (
            StatusCode::OK,
            axum::Json(EnqueueJobResponse {
                enqueued: false,
                message:
                    "File was not enqueued because it matched existing output or dedupe rules."
                        .to_string(),
            }),
        )
            .into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub(crate) async fn request_job_cancel(state: &AppState, job: &Job) -> Result<bool> {
    state.transcoder.add_cancel_request(job.id).await;
    match job.status {
        JobState::Queued => {
            state
                .db
                .update_job_status(job.id, JobState::Cancelled)
                .await?;
            state.transcoder.remove_cancel_request(job.id).await;
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
            state.transcoder.remove_cancel_request(job.id).await;
            Ok(true)
        }
        JobState::Encoding | JobState::Remuxing => Ok(state.transcoder.cancel_job(job.id)),
        _ => Ok(false),
    }
}

#[derive(Deserialize)]
pub(crate) struct JobTableParams {
    limit: Option<i64>,
    page: Option<i64>,
    status: Option<String>,
    search: Option<String>,
    sort: Option<String>,
    sort_by: Option<String>,
    sort_desc: Option<bool>,
    archived: Option<String>,
}

pub(crate) async fn jobs_table_handler(
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
        if list.is_empty() { None } else { Some(list) }
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
        Ok(jobs) => {
            let job_ids = jobs.iter().map(|job| job.id).collect::<Vec<_>>();
            let explanations = match state.db.get_job_decision_explanations(&job_ids).await {
                Ok(explanations) => explanations,
                Err(e) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
                }
            };

            let payload = jobs
                .into_iter()
                .map(|job| JobResponse {
                    decision_explanation: explanations.get(&job.id).cloned(),
                    job,
                })
                .collect::<Vec<_>>();

            axum::Json(payload).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct BatchActionPayload {
    action: String,
    ids: Vec<i64>,
}

pub(crate) async fn batch_jobs_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<BatchActionPayload>,
) -> impl IntoResponse {
    let jobs = match state.db.get_jobs_by_ids(&payload.ids).await {
        Ok(jobs) => jobs,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    match payload.action.as_str() {
        "cancel" => {
            // Add all cancel requests first (in-memory, cheap).
            for job in &jobs {
                state.transcoder.add_cancel_request(job.id).await;
            }

            // Collect IDs that can be immediately set to Cancelled in the DB.
            let mut immediate_ids: Vec<i64> = Vec::new();
            let mut active_count: u64 = 0;

            for job in &jobs {
                match job.status {
                    JobState::Queued => {
                        immediate_ids.push(job.id);
                    }
                    JobState::Analyzing | JobState::Resuming => {
                        if state.transcoder.cancel_job(job.id) {
                            immediate_ids.push(job.id);
                        }
                    }
                    JobState::Encoding | JobState::Remuxing => {
                        if state.transcoder.cancel_job(job.id) {
                            active_count += 1;
                        }
                    }
                    _ => {}
                }
            }

            // Single batch DB update instead of N individual queries.
            if !immediate_ids.is_empty() {
                match state.db.batch_cancel_jobs(&immediate_ids).await {
                    Ok(_) => {}
                    Err(e) => {
                        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
                    }
                }
                // Remove cancel requests for jobs already resolved in DB.
                for id in &immediate_ids {
                    state.transcoder.remove_cancel_request(*id).await;
                }
            }

            let count = immediate_ids.len() as u64 + active_count;
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
                Ok(count) => {
                    if payload.action == "delete" {
                        purge_resume_sessions_for_jobs(state.as_ref(), &payload.ids).await;
                    }
                    axum::Json(serde_json::json!({ "count": count })).into_response()
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        _ => (StatusCode::BAD_REQUEST, "Invalid action").into_response(),
    }
}

pub(crate) async fn cancel_job_handler(
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

pub(crate) async fn restart_failed_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
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

pub(crate) async fn clear_completed_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let completed_job_ids = match state.db.get_jobs_by_status(JobState::Completed).await {
        Ok(jobs) => jobs.into_iter().map(|job| job.id).collect::<Vec<_>>(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    match state.db.clear_completed_jobs().await {
        Ok(count) => {
            purge_resume_sessions_for_jobs(state.as_ref(), &completed_job_ids).await;
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

pub(crate) async fn restart_job_handler(
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

pub(crate) async fn delete_job_handler(
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

    state.transcoder.cancel_job(id);

    match state.db.delete_job(id).await {
        Ok(_) => {
            purge_resume_sessions_for_jobs(state.as_ref(), &[id]).await;
            StatusCode::OK.into_response()
        }
        Err(e) if is_row_not_found(&e) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct UpdateJobPriorityPayload {
    priority: i32,
}

pub(crate) async fn update_job_priority_handler(
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
pub(crate) struct JobResponse {
    #[serde(flatten)]
    job: Job,
    decision_explanation: Option<Explanation>,
}

#[derive(Serialize)]
pub(crate) struct JobDetailResponse {
    job: Job,
    metadata: Option<crate::media::pipeline::MediaMetadata>,
    encode_stats: Option<crate::db::DetailedEncodeStats>,
    encode_attempts: Vec<crate::db::EncodeAttempt>,
    job_logs: Vec<crate::db::LogEntry>,
    job_failure_summary: Option<String>,
    decision_explanation: Option<Explanation>,
    failure_explanation: Option<Explanation>,
    queue_position: Option<u32>,
}

pub(crate) async fn get_job_detail_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let job = match state.db.get_job_by_id(id).await {
        Ok(Some(j)) => j,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let metadata = job.input_metadata();

    // Try to get encode stats (using the subquery result or a specific query)
    // For now we'll just query the encode_stats table if completed
    let encode_stats = if job.status == JobState::Completed {
        match state.db.get_encode_stats_by_job_id(id).await {
            Ok(stats) => Some(stats),
            Err(err) if is_row_not_found(&err) => None,
            Err(err) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
            }
        }
    } else {
        None
    };

    let job_logs = match state.db.get_logs_for_job(id, 200).await {
        Ok(logs) => logs,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    let decision_explanation = match state.db.get_job_decision_explanation(id).await {
        Ok(explanation) => explanation,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    let (job_failure_summary, failure_explanation) = if job.status == JobState::Failed {
        let legacy_summary = job_logs
            .iter()
            .rev()
            .find(|entry| entry.level.eq_ignore_ascii_case("error"))
            .map(|entry| entry.message.clone());
        let stored_failure = match state.db.get_job_failure_explanation(id).await {
            Ok(explanation) => explanation,
            Err(err) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
            }
        };
        let summary = stored_failure
            .as_ref()
            .map(|explanation| explanation.legacy_reason.clone())
            .or(legacy_summary.clone());
        let explanation = stored_failure.or_else(|| {
            legacy_summary
                .as_deref()
                .map(crate::explanations::failure_from_summary)
        });
        (summary, explanation)
    } else {
        (None, None)
    };

    let encode_attempts = match state.db.get_encode_attempts_by_job(id).await {
        Ok(attempts) => attempts,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    let queue_position = if job.status == JobState::Queued {
        match state.db.get_queue_position(id).await {
            Ok(position) => position,
            Err(err) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
            }
        }
    } else {
        None
    };

    axum::Json(JobDetailResponse {
        job,
        metadata,
        encode_stats,
        encode_attempts,
        job_logs,
        job_failure_summary,
        decision_explanation,
        failure_explanation,
        queue_position,
    })
    .into_response()
}

// Engine control handlers

pub(crate) async fn pause_engine_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.agent.stop_drain();
    state.agent.pause();
    axum::Json(serde_json::json!({ "status": "paused" }))
}

pub(crate) async fn resume_engine_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.agent.stop_drain();
    state.agent.resume();
    axum::Json(serde_json::json!({ "status": "running" }))
}

pub(crate) async fn drain_engine_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.agent.drain();
    axum::Json(serde_json::json!({ "status": "draining" }))
}

pub(crate) async fn stop_drain_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.agent.stop_drain();
    axum::Json(serde_json::json!({ "status": "running" }))
}

pub(crate) async fn restart_engine_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.agent.restart().await;
    axum::Json(serde_json::json!({ "status": "running" }))
}

pub(crate) async fn engine_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
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

pub(crate) async fn get_engine_mode_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    let cpu_count = {
        let sys = state.sys.lock().await;
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
pub(crate) struct SetEngineModePayload {
    mode: crate::config::EngineMode,
    // Optional manual override of concurrent jobs.
    // If provided, bypasses mode auto-computation.
    concurrent_jobs_override: Option<usize>,
    // Optional manual thread override (0 = auto).
    threads_override: Option<usize>,
}

pub(crate) async fn set_engine_mode_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SetEngineModePayload>,
) -> impl IntoResponse {
    let cpu_count = {
        let sys = state.sys.lock().await;
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
    if let Err(e) = super::save_config_or_response(&state, &config).await {
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

// Logs handlers

#[derive(Deserialize)]
pub(crate) struct LogParams {
    page: Option<i64>,
    limit: Option<i64>,
}

pub(crate) async fn logs_history_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<LogParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    match state.db.get_logs(limit, offset).await {
        Ok(logs) => axum::Json(logs).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub(crate) async fn clear_logs_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.clear_logs().await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
