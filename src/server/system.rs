//! System information, hardware info, resources, health handlers.

use super::{AppState, config_read_error_response};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::error;

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
