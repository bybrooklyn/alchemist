//! System information, hardware info, resources, health handlers.

use super::{AppState, api_error_response, config_read_error_response};
use crate::media::pipeline::{Planner as _, TranscodeDecision};
use async_compression::tokio::bufread::GzipEncoder;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures::StreamExt;
use serde::Serialize;
use std::io::ErrorKind;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::error;

const INTELLIGENCE_CACHE_TTL: Duration = Duration::from_secs(30);
const MAX_INTELLIGENCE_JOBS: i64 = 500;

#[derive(Serialize)]
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

#[derive(Serialize)]
pub(crate) struct ProcessorStatusResponse {
    blocked_reason: Option<&'static str>,
    message: String,
    manual_paused: bool,
    scheduler_paused: bool,
    draining: bool,
    active_jobs: i64,
    concurrent_limit: usize,
}

#[derive(Serialize)]
struct DuplicateGroup {
    stem: String,
    count: usize,
    paths: Vec<DuplicatePath>,
}

#[derive(Serialize)]
struct DuplicatePath {
    id: i64,
    path: String,
    status: String,
}

#[derive(Serialize)]
struct LibraryIntelligenceResponse {
    duplicate_groups: Vec<DuplicateGroup>,
    total_duplicates: usize,
    recommendation_counts: RecommendationCounts,
    recommendations: Vec<IntelligenceRecommendation>,
}

#[derive(Serialize, Default)]
struct RecommendationCounts {
    duplicates: usize,
    remux_only_candidate: usize,
    wasteful_audio_layout: usize,
    commentary_cleanup_candidate: usize,
}

#[derive(Serialize, Clone)]
struct IntelligenceRecommendation {
    #[serde(rename = "type")]
    recommendation_type: String,
    title: String,
    summary: String,
    path: String,
    suggested_action: String,
}

pub(crate) async fn system_resources_handler(State(state): State<Arc<AppState>>) -> Response {
    let mut cache = state.resources_cache.lock().await;
    if let Some((value, cached_at)) = cache.as_ref() {
        if cached_at.elapsed() < Duration::from_millis(500) {
            return axum::Json(value.clone()).into_response();
        }
    }

    let (cpu_percent, memory_used_mb, memory_total_mb, memory_percent, cpu_count) = {
        let mut sys = state.sys.lock().await;
        sys.refresh_all();

        let cpu_percent =
            sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32;
        let cpu_count = sys.cpus().len();
        let memory_used_mb = sys.used_memory() / 1024 / 1024;
        let memory_total_mb = sys.total_memory() / 1024 / 1024;
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

pub(crate) async fn processor_status_handler(State(state): State<Arc<AppState>>) -> Response {
    let stats = match state.db.get_job_stats().await {
        Ok(stats) => stats,
        Err(err) => return config_read_error_response("load processor status", &err),
    };

    let concurrent_limit = state.agent.concurrent_jobs_limit();
    let manual_paused = state.agent.is_manual_paused();
    let scheduler_paused = state.agent.is_scheduler_paused();
    let draining = state.agent.is_draining();
    let active_jobs = stats.active;

    let (blocked_reason, message) = if manual_paused {
        (
            Some("manual_paused"),
            "The engine is manually paused and will not start queued jobs.".to_string(),
        )
    } else if scheduler_paused {
        (
            Some("scheduled_pause"),
            "The schedule is currently pausing the engine.".to_string(),
        )
    } else if draining {
        (
            Some("draining"),
            "The engine is draining and will not start new queued jobs.".to_string(),
        )
    } else if active_jobs >= concurrent_limit as i64 {
        (
            Some("workers_busy"),
            "All worker slots are currently busy.".to_string(),
        )
    } else {
        (None, "Workers are available.".to_string())
    };

    axum::Json(ProcessorStatusResponse {
        blocked_reason,
        message,
        manual_paused,
        scheduler_paused,
        draining,
        active_jobs,
        concurrent_limit,
    })
    .into_response()
}

pub(crate) async fn library_intelligence_handler(State(state): State<Arc<AppState>>) -> Response {
    use std::collections::HashMap;
    use std::path::Path;

    {
        let guard = state.library_intelligence_cache.lock().await;
        if let Some((payload, cached_at)) = guard.as_ref() {
            if cached_at.elapsed() < INTELLIGENCE_CACHE_TTL {
                return axum::Json(payload.clone()).into_response();
            }
        }
    }

    let duplicate_candidates = match state.db.get_duplicate_candidates().await {
        Ok(candidates) => candidates,
        Err(err) => {
            error!("Failed to fetch duplicate candidates: {err}");
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GET_DUPLICATES_FAILED",
                err.to_string(),
            );
        }
    };

    let mut groups: HashMap<String, Vec<_>> = HashMap::new();
    for candidate in duplicate_candidates {
        let stem = Path::new(&candidate.input_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if stem.is_empty() {
            continue;
        }
        groups.entry(stem).or_default().push(candidate);
    }

    let mut duplicate_groups: Vec<DuplicateGroup> = groups
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .map(|(stem, paths)| {
            let count = paths.len();
            DuplicateGroup {
                stem,
                count,
                paths: paths
                    .into_iter()
                    .map(|candidate| DuplicatePath {
                        id: candidate.id,
                        path: candidate.input_path,
                        status: candidate.status,
                    })
                    .collect(),
            }
        })
        .collect();

    duplicate_groups.sort_by(|a, b| b.count.cmp(&a.count).then(a.stem.cmp(&b.stem)));
    let total_duplicates = duplicate_groups.iter().map(|group| group.count - 1).sum();

    let mut recommendations = Vec::new();
    let mut recommendation_counts = RecommendationCounts {
        duplicates: duplicate_groups.len(),
        ..RecommendationCounts::default()
    };

    let jobs = match state
        .db
        .get_jobs_for_intelligence(MAX_INTELLIGENCE_JOBS)
        .await
    {
        Ok(jobs) => jobs,
        Err(err) => {
            error!("Failed to fetch jobs for intelligence recommendations: {err}");
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GET_INTELLIGENCE_JOBS_FAILED",
                err.to_string(),
            );
        }
    };

    let config_snapshot = state.config.read().await.clone();
    let hw_snapshot = state.hardware_state.snapshot().await;
    let planner = crate::media::planner::BasicPlanner::new(
        std::sync::Arc::new(config_snapshot.clone()),
        hw_snapshot,
    );

    for job in jobs {
        // Use stored metadata only — no live ffprobe spawning per job.
        let metadata = match job.input_metadata() {
            Some(m) => m,
            None => continue,
        };
        let analysis = crate::media::pipeline::MediaAnalysis {
            metadata,
            warnings: vec![],
            confidence: crate::media::pipeline::AnalysisConfidence::High,
        };

        let profile: Option<crate::db::LibraryProfile> =
            match state.db.get_profile_for_path(&job.input_path).await {
                Ok(p) => p,
                Err(err) => {
                    error!(
                        "Failed to fetch profile for intelligence recommendation at {}: {}",
                        job.input_path, err
                    );
                    return api_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "GET_PROFILE_FAILED",
                        err.to_string(),
                    );
                }
            };

        if let Ok(plan) = planner
            .plan(
                &analysis,
                std::path::Path::new(&job.output_path),
                profile.as_ref(),
            )
            .await
        {
            if matches!(plan.decision, TranscodeDecision::Remux { .. }) {
                recommendation_counts.remux_only_candidate += 1;
                recommendations.push(IntelligenceRecommendation {
                    recommendation_type: "remux_only_candidate".to_string(),
                    title: "Remux-only opportunity".to_string(),
                    summary: "This file already matches the target video codec and looks like a container-normalization candidate instead of a full re-encode.".to_string(),
                    path: job.input_path.clone(),
                    suggested_action: "Queue a remux to normalize the container without re-encoding the video stream.".to_string(),
                });
            }
        }

        if analysis.metadata.audio_is_heavy {
            recommendation_counts.wasteful_audio_layout += 1;
            recommendations.push(IntelligenceRecommendation {
                recommendation_type: "wasteful_audio_layout".to_string(),
                title: "Wasteful audio layout".to_string(),
                summary: "This file contains a lossless or oversized audio stream that is likely worth transcoding for storage recovery.".to_string(),
                path: job.input_path.clone(),
                suggested_action: "Use a profile that transcodes heavy audio instead of copying it through unchanged.".to_string(),
            });
        }

        if analysis.metadata.audio_streams.iter().any(|stream| {
            stream
                .title
                .as_deref()
                .map(|title| {
                    let lower = title.to_ascii_lowercase();
                    lower.contains("commentary")
                        || lower.contains("director")
                        || lower.contains("description")
                        || lower.contains("descriptive")
                })
                .unwrap_or(false)
        }) {
            recommendation_counts.commentary_cleanup_candidate += 1;
            recommendations.push(IntelligenceRecommendation {
                recommendation_type: "commentary_cleanup_candidate".to_string(),
                title: "Commentary or descriptive track cleanup".to_string(),
                summary: "This file appears to contain commentary or descriptive audio tracks that existing stream rules could strip automatically.".to_string(),
                path: job.input_path.clone(),
                suggested_action: "Enable stream rules to strip commentary or descriptive tracks for this library.".to_string(),
            });
        }
    }

    recommendations.sort_by(|a, b| {
        a.recommendation_type
            .cmp(&b.recommendation_type)
            .then(a.path.cmp(&b.path))
    });

    let value = serde_json::json!(LibraryIntelligenceResponse {
        duplicate_groups,
        total_duplicates,
        recommendation_counts,
        recommendations,
    });

    {
        let mut guard = state.library_intelligence_cache.lock().await;
        *guard = Some((value.clone(), Instant::now()));
    }
    axum::Json(value).into_response()
}

pub(crate) async fn reanalyze_library_root_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    let mut root_paths: Vec<String> = config.scanner.directories.clone();
    drop(config);

    if let Ok(watch_dirs) = state.db.get_watch_dirs().await {
        for wd in watch_dirs {
            root_paths.push(wd.path);
        }
    }

    let mut total_reanalyzed = 0;
    for root in root_paths {
        let jobs = match state.db.get_jobs_under_root_path(&root).await {
            Ok(jobs) => jobs,
            Err(_) => continue,
        };

        let ids: Vec<i64> = jobs
            .into_iter()
            .filter(|j| !j.is_active())
            .map(|j| j.id)
            .collect();

        if let Ok(count) = state.db.batch_reanalyze_jobs(&ids).await {
            total_reanalyzed += count;
        }
    }

    axum::Json(serde_json::json!({ "count": total_reanalyzed })).into_response()
}

struct SnapshotCleanup {
    path: std::path::PathBuf,
}

impl Drop for SnapshotCleanup {
    fn drop(&mut self) {
        if self.path.exists() {
            if let Err(err) = std::fs::remove_file(&self.path) {
                if err.kind() != ErrorKind::NotFound {
                    tracing::warn!(
                        path = %self.path.display(),
                        "Failed to remove backup snapshot during drop cleanup: {err}"
                    );
                }
            }
        }
    }
}

pub(crate) async fn backup_database_handler(State(state): State<Arc<AppState>>) -> Response {
    let mut snapshot_path = crate::runtime::temp_dir();
    let token: u64 = rand::random();
    snapshot_path.push(format!("alchemist-backup-{}.db", token));

    if let Err(err) = state.db.create_online_backup(&snapshot_path).await {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "BACKUP_FAILED",
            format!("Database backup failed: {err}"),
        );
    }

    let file = match tokio::fs::File::open(&snapshot_path).await {
        Ok(file) => file,
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "OPEN_BACKUP_FAILED",
                format!("Failed to open backup snapshot: {err}"),
            );
        }
    };

    let reader = tokio::io::BufReader::new(file);
    let reader_stream = tokio_util::io::ReaderStream::new(reader);
    let cleanup = Arc::new(SnapshotCleanup {
        path: snapshot_path.clone(),
    });

    let stream = futures::stream::unfold(Some((reader_stream, cleanup)), |state| async move {
        let (mut reader, cleanup) = state?;
        match reader.next().await {
            Some(Ok(chunk)) => Some((Ok::<_, std::io::Error>(chunk), Some((reader, cleanup)))),
            Some(Err(err)) => Some((Err(err), None)),
            None => None,
        }
    });

    let body_reader = tokio_util::io::StreamReader::new(stream);
    let gzip_stream = GzipEncoder::new(body_reader);
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(gzip_stream));

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-sqlite3"),
    );
    headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"alchemist.db.gz\""),
    );

    (headers, body).into_response()
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

#[derive(Serialize)]
struct SystemInfo {
    version: String,
    os_version: String,
    is_docker: bool,
    telemetry_enabled: bool,
    ffmpeg_version: String,
}

#[derive(Serialize)]
struct UpdateInfo {
    current_version: String,
    latest_version: Option<String>,
    update_available: bool,
    release_url: Option<String>,
}

pub(crate) async fn get_system_info_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    let version = crate::version::current().to_string();
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

pub(crate) async fn get_system_update_handler() -> impl IntoResponse {
    let current_version = crate::version::current().to_string();
    match fetch_latest_stable_release().await {
        Ok(Some((latest_version, release_url))) => {
            let update_available = version_is_newer(&latest_version, &current_version);
            axum::Json(UpdateInfo {
                current_version,
                latest_version: Some(latest_version),
                update_available,
                release_url: Some(release_url),
            })
            .into_response()
        }
        Ok(None) => axum::Json(UpdateInfo {
            current_version,
            latest_version: None,
            update_available: false,
            release_url: None,
        })
        .into_response(),
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            format!("Failed to check for updates: {err}"),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct GitHubReleaseResponse {
    tag_name: String,
    html_url: String,
}

async fn fetch_latest_stable_release() -> Result<Option<(String, String)>, reqwest::Error> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!("alchemist/{}", crate::version::current()))
        .build()?;
    let response = client
        .get("https://api.github.com/repos/bybrooklyn/alchemist/releases/latest")
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let release: GitHubReleaseResponse = response.error_for_status()?.json().await?;
    Ok(Some((
        release.tag_name.trim_start_matches('v').to_string(),
        release.html_url,
    )))
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

fn parse_version(value: &str) -> (u64, u64, u64) {
    let sanitized = value.trim_start_matches('v');
    let parts = sanitized
        .split(['.', '-'])
        .filter_map(|part| part.parse::<u64>().ok())
        .collect::<Vec<_>>();
    (
        *parts.first().unwrap_or(&0),
        *parts.get(1).unwrap_or(&0),
        *parts.get(2).unwrap_or(&0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare_detects_newer_stable_release() {
        assert!(version_is_newer("0.3.1", "0.3.0"));
        assert!(!version_is_newer("0.3.0", "0.3.0"));
        assert!(!version_is_newer("0.2.9", "0.3.0"));
    }

    #[test]
    fn parse_version_ignores_prefix_and_suffix() {
        assert_eq!(parse_version("v0.3.1"), (0, 3, 1));
        assert_eq!(parse_version("0.3.1-rc.1"), (0, 3, 1));
    }
}

pub(crate) async fn get_hardware_info_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.hardware_state.snapshot().await {
        Some(info) => axum::Json(info).into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Hardware state unavailable",
        )
            .into_response(),
    }
}

pub(crate) async fn get_hardware_probe_log_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    axum::Json(state.hardware_probe_log.read().await.clone()).into_response()
}

pub(crate) async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed();
    let hours = uptime.as_secs() / 3600;
    let minutes = (uptime.as_secs() % 3600) / 60;
    let seconds = uptime.as_secs() % 60;

    axum::Json(serde_json::json!({
        "status": "ok",
        "version": crate::version::current(),
        "uptime": format!("{}h {}m {}s", hours, minutes, seconds),
        "uptime_seconds": uptime.as_secs()
    }))
}

pub(crate) async fn ready_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
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

// Filesystem browsing

#[derive(serde::Deserialize)]
pub(crate) struct FsBrowseQuery {
    path: Option<String>,
}

pub(crate) async fn fs_browse_handler(
    axum::extract::Query(query): axum::extract::Query<FsBrowseQuery>,
) -> impl IntoResponse {
    match crate::system::fs_browser::browse(query.path.as_deref()).await {
        Ok(response) => axum::Json(response).into_response(),
        Err(err) => config_read_error_response("browse server filesystem", &err),
    }
}

pub(crate) async fn fs_recommendations_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.config.read().await.clone();
    match crate::system::fs_browser::recommendations(&config, state.db.as_ref()).await {
        Ok(response) => axum::Json(response).into_response(),
        Err(err) => config_read_error_response("load folder recommendations", &err),
    }
}

pub(crate) async fn fs_preview_handler(
    axum::Json(payload): axum::Json<crate::system::fs_browser::FsPreviewRequest>,
) -> impl IntoResponse {
    match crate::system::fs_browser::preview(payload).await {
        Ok(response) => axum::Json(response).into_response(),
        Err(err) => config_read_error_response("preview selected server folders", &err),
    }
}

// Telemetry

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

pub(crate) async fn telemetry_payload_handler(State(state): State<Arc<AppState>>) -> Response {
    let config = state.config.read().await;
    if !config.system.enable_telemetry {
        return (StatusCode::FORBIDDEN, "Telemetry disabled").into_response();
    }

    let (cpu_count, memory_total_mb) = {
        let mut sys = state.sys.lock().await;
        sys.refresh_memory();
        (sys.cpus().len(), sys.total_memory() / 1024 / 1024)
    };

    let version = crate::version::current().to_string();
    let os_version = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    let is_docker = std::path::Path::new("/.dockerenv").exists();
    let uptime_seconds = state.start_time.elapsed().as_secs();
    let stats = match state.db.get_job_stats().await {
        Ok(stats) => stats,
        Err(err) => return config_read_error_response("load telemetry stats", &err),
    };

    axum::Json(TelemetryPayload {
        runtime_id: state.telemetry_runtime_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
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
