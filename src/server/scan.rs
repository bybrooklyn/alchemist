//! Library scanning and watch folder handlers.

use super::{
    AppState, api_accepted_response, api_error_response, api_ok_response, is_row_not_found,
    refresh_file_watcher, save_config_or_response,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use futures::{FutureExt, StreamExt, stream};
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;
use std::sync::Arc;
use std::sync::atomic::Ordering;
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

    if let Err(e) = state.agent.scan_and_enqueue(dirs).await {
        error!("Scan failed: {e}");
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SCAN_FAILED",
            e.to_string(),
        );
    }

    // Trigger analysis after scan completes so jobs
    // get skip/transcode decisions immediately, matching
    // boot and setup scan behavior
    let agent = state.agent.clone();
    tokio::spawn(async move {
        agent.analyze_pending_jobs().await;
    });

    api_ok_response()
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct StartScanParams {
    /// PERF-3: when `true`, bypass probe cache reads and the aggressive
    /// directory pruning shortcut for this scan. Defaults to `false`.
    #[serde(default)]
    pub full: bool,
}

pub(crate) async fn start_scan_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<StartScanParams>,
) -> impl IntoResponse {
    match state
        .library_scanner
        .start_scan_with_options(params.full)
        .await
    {
        Ok(_) => api_accepted_response(),
        Err(e) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "START_SCAN_FAILED",
            e.to_string(),
        ),
    }
}

pub(crate) async fn get_scan_status_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    axum::Json::<crate::system::scanner::ScanStatus>(state.library_scanner.get_status().await)
        .into_response()
}

// F-2 — Library plan preview.
//
// Walks `path` (recursively, bounded by `max_files`), runs the planner in
// dry-run mode against each discovered file, and returns counts plus a
// short sample so users can see what Alchemist *would* do before
// enqueueing a watch folder. No database mutation, no enqueueing.

#[derive(Debug, Deserialize)]
pub(crate) struct LibraryPreviewRequest {
    pub path: String,
    #[serde(default)]
    pub max_files: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LibraryPreviewSample {
    pub path: String,
    pub action: String,
    pub reason: String,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize, Default)]
pub(crate) struct LibraryPreviewCounts {
    pub skip: usize,
    pub remux: usize,
    pub encode: usize,
    pub error: usize,
}

#[derive(Debug, Serialize, Default)]
pub(crate) struct LibraryPreviewBytes {
    pub skip: i64,
    pub remux: i64,
    pub encode: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct LibraryPreviewResponse {
    pub scanned: usize,
    pub truncated: bool,
    pub counts: LibraryPreviewCounts,
    pub bytes_under_consideration: LibraryPreviewBytes,
    pub samples: Vec<LibraryPreviewSample>,
}

const PREVIEW_DEFAULT_MAX_FILES: usize = 60;
const PREVIEW_MAX_FILES_CAP: usize = 200;
const PREVIEW_SAMPLE_LIMIT: usize = 20;

/// Resolve `candidate` and confirm it canonicalizes inside one of the
/// configured library directories or watch folders. Mirrors the bound
/// `enqueue_job_from_submitted_path` enforces, so the preview endpoint
/// cannot be pointed at arbitrary host paths.
async fn preview_path_is_within_allowed_root(
    state: &AppState,
    candidate: &std::path::Path,
) -> bool {
    let Ok(canonical) = tokio::fs::canonicalize(candidate).await else {
        return false;
    };

    let mut allowed_roots: Vec<std::path::PathBuf> = {
        let config = state.config.read().await;
        config
            .scanner
            .directories
            .iter()
            .map(std::path::PathBuf::from)
            .collect()
    };
    if let Ok(watch_dirs) = state.db.get_watch_dirs().await {
        for wd in watch_dirs {
            allowed_roots.push(std::path::PathBuf::from(wd.path));
        }
    }

    for root in allowed_roots {
        if let Ok(canonical_root) = tokio::fs::canonicalize(&root).await {
            if canonical.starts_with(&canonical_root) {
                return true;
            }
        }
    }
    false
}

pub(crate) async fn preview_library_path_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<LibraryPreviewRequest>,
) -> impl IntoResponse {
    // Single-flight: a preview can ffprobe up to PREVIEW_MAX_FILES_CAP files,
    // so reject overlapping requests rather than multiplying subprocess load.
    struct PreviewGuard(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for PreviewGuard {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    if state
        .library_preview_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return api_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "PREVIEW_BUSY",
            "A library preview is already running. Try again in a moment.",
        );
    }
    let _preview_guard = PreviewGuard(state.library_preview_in_progress.clone());

    let max_files = payload
        .max_files
        .unwrap_or(PREVIEW_DEFAULT_MAX_FILES)
        .clamp(1, PREVIEW_MAX_FILES_CAP);

    let preview_root = std::path::PathBuf::from(&payload.path);
    if !preview_root.exists() {
        return api_error_response(
            StatusCode::BAD_REQUEST,
            "PREVIEW_PATH_MISSING",
            format!("Path does not exist: {}", payload.path),
        );
    }
    if !preview_root.is_dir() {
        return api_error_response(
            StatusCode::BAD_REQUEST,
            "PREVIEW_NOT_DIRECTORY",
            format!("Preview path must be a directory: {}", payload.path),
        );
    }
    if !preview_path_is_within_allowed_root(&state, &preview_root).await {
        return api_error_response(
            StatusCode::FORBIDDEN,
            "PREVIEW_PATH_FORBIDDEN",
            "Preview path must be within a configured library folder or watch directory.",
        );
    }

    let scan_target = preview_root.clone();
    let files = match tokio::task::spawn_blocking(move || {
        let scanner = crate::media::scanner::Scanner::new();
        scanner.scan_with_recursion(vec![(scan_target, true)])
    })
    .await
    {
        Ok(files) => files,
        Err(err) => {
            error!("Preview scan worker failed: {err}");
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PREVIEW_SCAN_FAILED",
                err.to_string(),
            );
        }
    };

    let truncated = files.len() > max_files;
    let to_process: Vec<_> = files.into_iter().take(max_files).collect();

    let file_settings = state
        .db
        .get_file_settings()
        .await
        .unwrap_or_else(|_| crate::media::pipeline::default_file_settings());
    let config_snapshot = Arc::new(state.config.read().await.clone());
    let hw_info = state.hardware_state.snapshot().await;
    let planner = crate::media::planner::BasicPlanner::new(config_snapshot, hw_info);
    let analyzer = crate::media::analyzer::FfmpegAnalyzer;
    use crate::media::pipeline::Planner as _;

    let mut counts = LibraryPreviewCounts::default();
    let mut bytes = LibraryPreviewBytes::default();
    let mut samples = Vec::new();

    for discovered in &to_process {
        let path = &discovered.path;
        let path_string = path.display().to_string();

        if let Ok(Some(reason)) =
            crate::media::pipeline::skip_reason_for_discovered_path(&state.db, path, &file_settings)
                .await
        {
            counts.skip += 1;
            if samples.len() < PREVIEW_SAMPLE_LIMIT {
                samples.push(LibraryPreviewSample {
                    path: path_string,
                    action: "skip".to_string(),
                    reason: reason.to_string(),
                    size_bytes: 0,
                });
            }
            continue;
        }

        let analysis = match analyzer.analyze_with_cache(&state.db, path).await {
            Ok(a) => a,
            Err(err) => {
                counts.error += 1;
                if samples.len() < PREVIEW_SAMPLE_LIMIT {
                    samples.push(LibraryPreviewSample {
                        path: path_string,
                        action: "error".to_string(),
                        reason: err.to_string(),
                        size_bytes: 0,
                    });
                }
                continue;
            }
        };

        let output_path =
            file_settings.output_path_for_source(path, discovered.source_root.as_deref());
        let profile = state
            .db
            .get_profile_for_path(&path.to_string_lossy())
            .await
            .ok()
            .flatten();

        let plan = match planner
            .plan(&analysis, &output_path, profile.as_ref())
            .await
        {
            Ok(p) => p,
            Err(err) => {
                counts.error += 1;
                if samples.len() < PREVIEW_SAMPLE_LIMIT {
                    samples.push(LibraryPreviewSample {
                        path: path_string,
                        action: "error".to_string(),
                        reason: err.to_string(),
                        size_bytes: 0,
                    });
                }
                continue;
            }
        };

        let size_bytes = analysis.metadata.size_bytes as i64;
        let (action, reason) = match &plan.decision {
            crate::media::pipeline::TranscodeDecision::Skip { reason } => {
                counts.skip += 1;
                bytes.skip = bytes.skip.saturating_add(size_bytes);
                ("skip".to_string(), reason.clone())
            }
            crate::media::pipeline::TranscodeDecision::Remux { reason } => {
                counts.remux += 1;
                bytes.remux = bytes.remux.saturating_add(size_bytes);
                ("remux".to_string(), reason.clone())
            }
            crate::media::pipeline::TranscodeDecision::Transcode { reason } => {
                counts.encode += 1;
                bytes.encode = bytes.encode.saturating_add(size_bytes);
                ("encode".to_string(), reason.clone())
            }
        };

        if samples.len() < PREVIEW_SAMPLE_LIMIT {
            samples.push(LibraryPreviewSample {
                path: path_string,
                action,
                reason,
                size_bytes,
            });
        }
    }

    let response = LibraryPreviewResponse {
        scanned: to_process.len(),
        truncated,
        counts,
        bytes_under_consideration: bytes,
        samples,
    };
    axum::Json(response).into_response()
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
        Err(err) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "GET_HEALTH_SUMMARY_FAILED",
            err.to_string(),
        ),
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
        Err(err) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "GET_HEALTH_ISSUES_FAILED",
            err.to_string(),
        ),
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
) -> axum::response::Response {
    if state
        .library_health_scan_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return api_error_response(
            StatusCode::CONFLICT,
            "HEALTH_SCAN_IN_PROGRESS",
            "A library health scan is already running",
        );
    }

    let db = state.db.clone();
    let flag = state.library_health_scan_in_progress.clone();
    tokio::spawn(async move {
        // run_library_health_scan already wraps its body in AssertUnwindSafe.catch_unwind(),
        // so a panic in the inner task still returns and clears the flag here.
        run_library_health_scan(db).await;
        flag.store(false, Ordering::SeqCst);
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
        Ok(None) => {
            return api_error_response(StatusCode::NOT_FOUND, "JOB_NOT_FOUND", "Job not found");
        }
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GET_JOB_FAILED",
                err.to_string(),
            );
        }
    };

    match crate::media::health::HealthChecker::check_file(FsPath::new(&job.output_path)).await {
        Ok(issue) => {
            if let Err(err) = state.db.record_health_check(job.id, issue.as_ref()).await {
                return api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "RECORD_HEALTH_CHECK_FAILED",
                    err.to_string(),
                );
            }
            axum::Json(serde_json::json!({
                "job_id": job.id,
                "issue_found": issue.is_some(),
            }))
            .into_response()
        }
        Err(err) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "HEALTH_CHECK_FAILED",
            err.to_string(),
        ),
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
        Err(e) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "GET_WATCH_DIRS_FAILED",
            e.to_string(),
        ),
    }
}

pub(crate) async fn add_watch_dir_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<AddWatchDirPayload>,
) -> impl IntoResponse {
    let normalized_path = match super::canonicalize_directory_path(&payload.path, "path") {
        Ok(path) => path,
        Err(msg) => return api_error_response(StatusCode::BAD_REQUEST, "INVALID_PATH", msg),
    };

    let normalized_path = normalized_path.to_string_lossy().to_string();
    let mut next_config = state.config.read().await.clone();
    if next_config
        .scanner
        .extra_watch_dirs
        .iter()
        .any(|watch_dir| watch_dir.path == normalized_path)
    {
        return api_error_response(
            StatusCode::CONFLICT,
            "WATCH_DIR_EXISTS",
            "watch folder already exists",
        );
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
            .unwrap_or_else(api_ok_response),
        Err(e) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "GET_WATCH_DIRS_FAILED",
            e.to_string(),
        ),
    }
}

#[derive(Deserialize)]
pub(crate) struct SyncWatchDirsPayload {
    dirs: Vec<crate::config::WatchDirConfig>,
}

pub(crate) async fn sync_watch_dirs_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SyncWatchDirsPayload>,
) -> impl IntoResponse {
    let mut next_config = state.config.read().await.clone();
    next_config.scanner.extra_watch_dirs = payload.dirs;

    if let Err(response) = save_config_or_response(&state, &next_config).await {
        return *response;
    }

    {
        let mut config = state.config.write().await;
        *config = next_config;
    }

    refresh_file_watcher(&state).await;

    match state.db.get_watch_dirs().await {
        Ok(dirs) => axum::Json(dirs).into_response(),
        Err(e) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "GET_WATCH_DIRS_FAILED",
            e.to_string(),
        ),
    }
}

pub(crate) async fn remove_watch_dir_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let dir = match state.db.get_watch_dirs().await {
        Ok(dirs) => dirs.into_iter().find(|dir| dir.id == id),
        Err(e) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GET_WATCH_DIRS_FAILED",
                e.to_string(),
            );
        }
    };
    let Some(dir) = dir else {
        return api_error_response(
            StatusCode::NOT_FOUND,
            "WATCH_DIR_NOT_FOUND",
            "Watch folder not found",
        );
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
    api_ok_response()
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
        Err(err) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "GET_PROFILES_FAILED",
            err.to_string(),
        ),
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
        return api_error_response(StatusCode::BAD_REQUEST, "INVALID_PROFILE", message);
    }

    let new_profile = to_new_library_profile(payload);
    let id = match state.db.create_profile(new_profile).await {
        Ok(id) => id,
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CREATE_PROFILE_FAILED",
                err.to_string(),
            );
        }
    };

    match state.db.get_profile(id).await {
        Ok(Some(profile)) => (
            StatusCode::CREATED,
            axum::Json(library_profile_response(profile)),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::CREATED,
            axum::Json(serde_json::json!({ "ok": true, "id": id })),
        )
            .into_response(),
        Err(err) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "GET_PROFILE_FAILED",
            err.to_string(),
        ),
    }
}

pub(crate) async fn update_profile_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    axum::Json(payload): axum::Json<LibraryProfilePayload>,
) -> impl IntoResponse {
    if is_builtin_profile_id(id) {
        return api_error_response(
            StatusCode::CONFLICT,
            "BUILTIN_PROFILE_READ_ONLY",
            "Built-in presets are read-only",
        );
    }
    if let Err(message) = validate_library_profile_payload(&payload) {
        return api_error_response(StatusCode::BAD_REQUEST, "INVALID_PROFILE", message);
    }

    match state
        .db
        .update_profile(id, to_new_library_profile(payload))
        .await
    {
        Ok(_) => match state.db.get_profile(id).await {
            Ok(Some(profile)) => axum::Json(library_profile_response(profile)).into_response(),
            Ok(None) => api_error_response(
                StatusCode::NOT_FOUND,
                "PROFILE_NOT_FOUND",
                "Profile not found",
            ),
            Err(err) => api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GET_PROFILE_FAILED",
                err.to_string(),
            ),
        },
        Err(err) if is_row_not_found(&err) => api_error_response(
            StatusCode::NOT_FOUND,
            "PROFILE_NOT_FOUND",
            "Profile not found",
        ),
        Err(err) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "UPDATE_PROFILE_FAILED",
            err.to_string(),
        ),
    }
}

pub(crate) async fn delete_profile_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if is_builtin_profile_id(id) {
        return api_error_response(
            StatusCode::CONFLICT,
            "BUILTIN_PROFILE_DELETE_BLOCKED",
            "Built-in presets cannot be deleted",
        );
    }

    match state.db.count_watch_dirs_using_profile(id).await {
        Ok(count) if count > 0 => api_error_response(
            StatusCode::CONFLICT,
            "PROFILE_IN_USE",
            "Profile is still assigned to one or more watch folders",
        ),
        Ok(_) => match state.db.delete_profile(id).await {
            Ok(_) => api_ok_response(),
            Err(err) if is_row_not_found(&err) => api_error_response(
                StatusCode::NOT_FOUND,
                "PROFILE_NOT_FOUND",
                "Profile not found",
            ),
            Err(err) => api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DELETE_PROFILE_FAILED",
                err.to_string(),
            ),
        },
        Err(err) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "COUNT_PROFILE_USAGE_FAILED",
            err.to_string(),
        ),
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
            Ok(None) => {
                return api_error_response(
                    StatusCode::NOT_FOUND,
                    "PROFILE_NOT_FOUND",
                    "Profile not found",
                );
            }
            Err(err) => {
                return api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "GET_PROFILE_FAILED",
                    err.to_string(),
                );
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
                .unwrap_or_else(api_ok_response),
            Err(err) => api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GET_WATCH_DIRS_FAILED",
                err.to_string(),
            ),
        },
        Err(err) if is_row_not_found(&err) => api_error_response(
            StatusCode::NOT_FOUND,
            "WATCH_DIR_NOT_FOUND",
            "Watch folder not found",
        ),
        Err(err) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ASSIGN_PROFILE_FAILED",
            err.to_string(),
        ),
    }
}

pub(crate) async fn reanalyze_watch_dir_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let watch_dir = match state.db.get_watch_dirs().await {
        Ok(dirs) => dirs.into_iter().find(|d| d.id == id),
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GET_WATCH_DIRS_FAILED",
                err.to_string(),
            );
        }
    };

    let Some(watch_dir) = watch_dir else {
        return api_error_response(
            StatusCode::NOT_FOUND,
            "WATCH_DIR_NOT_FOUND",
            "Watch folder not found",
        );
    };

    let jobs = match state.db.get_jobs_under_root_path(&watch_dir.path).await {
        Ok(jobs) => jobs,
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GET_JOBS_FAILED",
                err.to_string(),
            );
        }
    };

    let ids: Vec<i64> = jobs
        .into_iter()
        .filter(|j| !j.is_active())
        .map(|j| j.id)
        .collect();

    match state.db.batch_reanalyze_jobs(&ids).await {
        Ok(count) => axum::Json(serde_json::json!({ "count": count })).into_response(),
        Err(err) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "BATCH_REANALYZE_FAILED",
            err.to_string(),
        ),
    }
}
