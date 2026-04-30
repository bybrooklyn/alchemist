use super::{AppState, api_error_response};
use crate::conversion::ConversionSettings;
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use tokio::{fs, io::AsyncWriteExt};
use tokio_util::io::ReaderStream;
use tracing::warn;

const DRAFT_RETENTION_HOURS: i64 = 24;

#[derive(Serialize)]
pub(crate) struct ConversionUploadResponse {
    conversion_job_id: i64,
    probe: crate::media::pipeline::MediaAnalysis,
    normalized_settings: ConversionSettings,
}

#[derive(Deserialize)]
pub(crate) struct ConversionPreviewPayload {
    conversion_job_id: i64,
    settings: ConversionSettings,
}

#[derive(Serialize)]
pub(crate) struct ConversionJobStatusResponse {
    id: i64,
    status: String,
    progress: f64,
    linked_job_id: Option<i64>,
    output_path: Option<String>,
    download_ready: bool,
    probe: Option<crate::media::pipeline::MediaAnalysis>,
}

fn conversion_root() -> PathBuf {
    crate::runtime::temp_dir()
}

fn uploads_root() -> PathBuf {
    conversion_root().join("uploads")
}

fn outputs_root() -> PathBuf {
    conversion_root().join("outputs")
}

fn sqlite_timestamp_now() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn sqlite_timestamp_after_hours(hours: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::hours(hours))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn upload_limit_bytes(limit_gb: u32) -> u64 {
    u64::from(limit_gb) * 1024 * 1024 * 1024
}

fn request_content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
}

async fn remove_file_if_exists(path: &FsPath) -> std::io::Result<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

async fn remove_dir_if_exists(path: &FsPath) -> std::io::Result<()> {
    match fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

async fn cleanup_upload_path(path: &FsPath) {
    let _ = remove_file_if_exists(path).await;
    if let Some(parent) = path.parent() {
        let _ = remove_dir_if_exists(parent).await;
    }
}

fn managed_artifact_parent(path: &FsPath) -> Option<PathBuf> {
    let parent = path.parent()?;
    let uploads_root = uploads_root();
    let outputs_root = outputs_root();
    if (parent.starts_with(&uploads_root) && parent != uploads_root.as_path())
        || (parent.starts_with(&outputs_root) && parent != outputs_root.as_path())
    {
        return Some(parent.to_path_buf());
    }
    None
}

async fn cleanup_expired_jobs(state: &AppState) {
    let now = sqlite_timestamp_now();
    let expired = match state.db.get_conversion_jobs_ready_for_cleanup(&now).await {
        Ok(expired) => expired,
        Err(_) => return,
    };

    for job in expired {
        if let Err(err) = remove_conversion_artifacts(&job).await {
            warn!(
                "Failed to remove expired conversion artifacts for {}: {}",
                job.id, err
            );
            continue;
        }
        if let Err(err) = state.db.delete_conversion_job(job.id).await {
            warn!(
                "Failed to delete expired conversion job {}: {}",
                job.id, err
            );
        }
    }
}

async fn remove_conversion_artifacts(job: &crate::db::ConversionJob) -> std::io::Result<()> {
    let upload_path = FsPath::new(&job.upload_path);
    if upload_path.exists() {
        remove_file_if_exists(upload_path).await?;
        if let Some(parent) = managed_artifact_parent(upload_path) {
            remove_dir_if_exists(&parent).await?;
        }
    }
    if let Some(output_path) = &job.output_path {
        let output_path = FsPath::new(output_path);
        if output_path.exists() {
            remove_file_if_exists(output_path).await?;
            if let Some(parent) = managed_artifact_parent(output_path) {
                remove_dir_if_exists(&parent).await?;
            }
        }
    }
    Ok(())
}

pub(crate) async fn upload_conversion_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    cleanup_expired_jobs(state.as_ref()).await;

    let upload_limit_gb = state.config.read().await.system.conversion_upload_limit_gb;
    let upload_limit = upload_limit_bytes(upload_limit_gb);
    if request_content_length(&headers).is_some_and(|value| value > upload_limit) {
        return api_error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "CONVERSION_UPLOAD_LIMIT_EXCEEDED",
            format!("Upload exceeds configured limit of {} GiB", upload_limit_gb),
        );
    }

    let mut field = match multipart.next_field().await {
        Ok(Some(field)) => field,
        Ok(None) => {
            return api_error_response(
                StatusCode::BAD_REQUEST,
                "CONVERSION_UPLOAD_FILE_MISSING",
                "missing upload file",
            );
        }
        Err(err) => {
            return api_error_response(
                StatusCode::BAD_REQUEST,
                "CONVERSION_UPLOAD_FIELD_FAILED",
                err.to_string(),
            );
        }
    };

    let upload_id = uuid::Uuid::new_v4().to_string();
    let upload_dir = uploads_root().join(&upload_id);
    if let Err(err) = fs::create_dir_all(&upload_dir).await {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CONVERSION_UPLOAD_DIR_CREATE_FAILED",
            err.to_string(),
        );
    }

    let file_name = field
        .file_name()
        .map(sanitize_filename)
        .unwrap_or_else(|| "input.bin".to_string());
    let stored_path = upload_dir.join(file_name);
    let mut output_file = match fs::File::create(&stored_path).await {
        Ok(file) => file,
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONVERSION_UPLOAD_FILE_CREATE_FAILED",
                err.to_string(),
            );
        }
    };
    let mut written_bytes = 0_u64;
    loop {
        match field.chunk().await {
            Ok(Some(chunk)) => {
                written_bytes = written_bytes.saturating_add(chunk.len() as u64);
                if written_bytes > upload_limit {
                    let _ = output_file.flush().await;
                    drop(output_file);
                    cleanup_upload_path(&stored_path).await;
                    return api_error_response(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "CONVERSION_UPLOAD_LIMIT_EXCEEDED",
                        format!("Upload exceeds configured limit of {} GiB", upload_limit_gb),
                    );
                }

                if let Err(err) = output_file.write_all(&chunk).await {
                    drop(output_file);
                    cleanup_upload_path(&stored_path).await;
                    return api_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "CONVERSION_UPLOAD_WRITE_FAILED",
                        err.to_string(),
                    );
                }
            }
            Ok(None) => break,
            Err(err) => {
                drop(output_file);
                cleanup_upload_path(&stored_path).await;
                return api_error_response(
                    StatusCode::BAD_REQUEST,
                    "CONVERSION_UPLOAD_CHUNK_FAILED",
                    err.to_string(),
                );
            }
        }
    }
    if let Err(err) = output_file.flush().await {
        drop(output_file);
        cleanup_upload_path(&stored_path).await;
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CONVERSION_UPLOAD_FLUSH_FAILED",
            err.to_string(),
        );
    }
    drop(output_file);

    let analyzer = crate::media::analyzer::FfmpegAnalyzer;
    let analysis = match analyzer.analyze_with_cache(&state.db, &stored_path).await {
        Ok(analysis) => analysis,
        Err(err) => {
            cleanup_upload_path(&stored_path).await;
            return api_error_response(
                StatusCode::BAD_REQUEST,
                "CONVERSION_UPLOAD_ANALYSIS_FAILED",
                err.to_string(),
            );
        }
    };

    let settings = ConversionSettings::default();
    let settings_json = match serde_json::to_string(&settings) {
        Ok(value) => value,
        Err(err) => {
            cleanup_upload_path(&stored_path).await;
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONVERSION_UPLOAD_SETTINGS_SERIALIZE_FAILED",
                err.to_string(),
            );
        }
    };
    let probe_json = match serde_json::to_string(&analysis) {
        Ok(value) => value,
        Err(err) => {
            cleanup_upload_path(&stored_path).await;
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONVERSION_UPLOAD_PROBE_SERIALIZE_FAILED",
                err.to_string(),
            );
        }
    };
    let expires_at = sqlite_timestamp_after_hours(DRAFT_RETENTION_HOURS);
    let conversion_job = match state
        .db
        .create_conversion_job(
            &stored_path.to_string_lossy(),
            if settings.remux_only {
                "remux"
            } else {
                "transcode"
            },
            &settings_json,
            Some(&probe_json),
            &expires_at,
        )
        .await
    {
        Ok(job) => job,
        Err(err) => {
            cleanup_upload_path(&stored_path).await;
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONVERSION_UPLOAD_JOB_CREATE_FAILED",
                err.to_string(),
            );
        }
    };

    axum::Json(ConversionUploadResponse {
        conversion_job_id: conversion_job.id,
        probe: analysis,
        normalized_settings: settings,
    })
    .into_response()
}

pub(crate) async fn preview_conversion_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<ConversionPreviewPayload>,
) -> impl IntoResponse {
    cleanup_expired_jobs(state.as_ref()).await;

    let Some(job) = (match state.db.get_conversion_job(payload.conversion_job_id).await {
        Ok(job) => job,
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONVERSION_JOB_LOAD_FAILED",
                err.to_string(),
            );
        }
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let analysis: crate::media::pipeline::MediaAnalysis = match job.probe_json.as_deref() {
        Some(probe_json) => match serde_json::from_str(probe_json) {
            Ok(analysis) => analysis,
            Err(err) => {
                return api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CONVERSION_PROBE_DESERIALIZE_FAILED",
                    err.to_string(),
                );
            }
        },
        None => {
            return api_error_response(
                StatusCode::BAD_REQUEST,
                "CONVERSION_PROBE_MISSING",
                "missing conversion probe",
            );
        }
    };

    let preview_output = outputs_root().join(format!(
        "preview-{}.{}",
        job.id, payload.settings.output_container
    ));
    let hw_info = state.hardware_state.snapshot().await;
    match crate::conversion::preview_command(
        FsPath::new(&job.upload_path),
        &preview_output,
        &analysis,
        &payload.settings,
        hw_info,
    ) {
        Ok(preview) => {
            if let Err(err) = persist_conversion_preview(
                state.as_ref(),
                job.id,
                &analysis,
                &preview.normalized_settings,
            )
            .await
            {
                return api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CONVERSION_PREVIEW_PERSIST_FAILED",
                    err.to_string(),
                );
            }
            axum::Json(preview).into_response()
        }
        Err(err) => api_error_response(
            StatusCode::BAD_REQUEST,
            "CONVERSION_PREVIEW_FAILED",
            err.to_string(),
        ),
    }
}

async fn persist_conversion_preview(
    state: &AppState,
    id: i64,
    analysis: &crate::media::pipeline::MediaAnalysis,
    settings: &ConversionSettings,
) -> crate::error::Result<()> {
    let settings_json = serde_json::to_string(settings)
        .map_err(|err| crate::error::AlchemistError::Unknown(err.to_string()))?;
    let probe_json = serde_json::to_string(analysis)
        .map_err(|err| crate::error::AlchemistError::Unknown(err.to_string()))?;
    state
        .db
        .persist_conversion_job_preview(
            id,
            &settings_json,
            if settings.remux_only {
                "remux"
            } else {
                "transcode"
            },
            if settings.remux_only {
                "draft_remux"
            } else {
                "draft_transcode"
            },
            &probe_json,
        )
        .await
}

pub(crate) async fn start_conversion_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    cleanup_expired_jobs(state.as_ref()).await;

    let Some(job) = (match state.db.get_conversion_job(id).await {
        Ok(job) => job,
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONVERSION_JOB_LOAD_FAILED",
                err.to_string(),
            );
        }
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if job.linked_job_id.is_some() {
        return api_error_response(
            StatusCode::CONFLICT,
            "CONVERSION_JOB_ALREADY_STARTED",
            "conversion job already started",
        );
    }

    let input_path = PathBuf::from(&job.upload_path);
    let file_stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let settings: ConversionSettings = match serde_json::from_str(&job.settings_json) {
        Ok(settings) => settings,
        Err(err) => {
            return api_error_response(
                StatusCode::BAD_REQUEST,
                "CONVERSION_SETTINGS_DESERIALIZE_FAILED",
                err.to_string(),
            );
        }
    };

    let output_dir = outputs_root().join(job.id.to_string());
    if let Err(err) = fs::create_dir_all(&output_dir).await {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CONVERSION_OUTPUT_DIR_CREATE_FAILED",
            err.to_string(),
        );
    }
    let output_path = output_dir.join(format!("{file_stem}.{}", settings.output_container));
    let mtime = std::fs::metadata(&input_path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(std::time::SystemTime::now());

    if let Err(err) = state.db.enqueue_job(&input_path, &output_path, mtime).await {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "JOB_ENQUEUE_FAILED",
            err.to_string(),
        );
    }
    let linked_job = match state
        .db
        .get_job_by_input_path(&input_path.to_string_lossy())
        .await
    {
        Ok(Some(job)) => job,
        Ok(None) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "LINKED_JOB_MISSING",
                "linked job missing",
            );
        }
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "LINKED_JOB_LOAD_FAILED",
                err.to_string(),
            );
        }
    };
    if let Err(err) = state
        .db
        .update_conversion_job_start(id, &output_path.to_string_lossy(), linked_job.id)
        .await
    {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CONVERSION_JOB_START_UPDATE_FAILED",
            err.to_string(),
        );
    }

    StatusCode::OK.into_response()
}

pub(crate) async fn get_conversion_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    cleanup_expired_jobs(state.as_ref()).await;

    let Some(conversion_job) = (match state.db.get_conversion_job(id).await {
        Ok(job) => job,
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONVERSION_JOB_LOAD_FAILED",
                err.to_string(),
            );
        }
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let linked_job = match conversion_job.linked_job_id {
        Some(job_id) => match state.db.get_job_by_id(job_id).await {
            Ok(job) => job,
            Err(err) => {
                return api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "LINKED_JOB_LOAD_FAILED",
                    err.to_string(),
                );
            }
        },
        None => None,
    };
    let probe = conversion_job
        .probe_json
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok());
    let download_ready = conversion_job
        .output_path
        .as_deref()
        .map(FsPath::new)
        .is_some_and(|path| path.exists());

    axum::Json(ConversionJobStatusResponse {
        id: conversion_job.id,
        status: linked_job
            .as_ref()
            .map(|job| job.status.to_string())
            .unwrap_or(conversion_job.status),
        progress: linked_job.as_ref().map(|job| job.progress).unwrap_or(0.0),
        linked_job_id: conversion_job.linked_job_id,
        output_path: conversion_job.output_path,
        download_ready,
        probe,
    })
    .into_response()
}

pub(crate) async fn download_conversion_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    cleanup_expired_jobs(state.as_ref()).await;

    let Some(job) = (match state.db.get_conversion_job(id).await {
        Ok(job) => job,
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONVERSION_JOB_LOAD_FAILED",
                err.to_string(),
            );
        }
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let Some(output_path) = job.output_path.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !FsPath::new(&output_path).exists() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let file = match fs::File::open(&output_path).await {
        Ok(file) => file,
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONVERSION_OUTPUT_OPEN_FAILED",
                err.to_string(),
            );
        }
    };
    let file_name = FsPath::new(&output_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output.bin");
    let retention_hours = state
        .config
        .read()
        .await
        .system
        .conversion_download_retention_hours;
    let expires_at = sqlite_timestamp_after_hours(i64::from(retention_hours));
    let stream = futures::stream::unfold(
        Some((ReaderStream::new(file), state.db.clone(), id, expires_at)),
        |state| async move {
            let (mut reader, db, job_id, expires_at) = state?;
            match reader.next().await {
                Some(Ok(chunk)) => Some((Ok(chunk), Some((reader, db, job_id, expires_at)))),
                Some(Err(err)) => Some((Err(err), None)),
                None => {
                    if let Err(err) = db.mark_conversion_job_downloaded(job_id, &expires_at).await {
                        warn!(
                            "Failed to mark conversion job {} as downloaded after full stream: {}",
                            job_id, err
                        );
                    }
                    None
                }
            }
        },
    );
    let body = Body::from_stream(stream);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", file_name))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );
    (headers, body).into_response()
}

pub(crate) async fn delete_conversion_job_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    cleanup_expired_jobs(state.as_ref()).await;

    let Some(job) = (match state.db.get_conversion_job(id).await {
        Ok(job) => job,
        Err(err) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONVERSION_JOB_LOAD_FAILED",
                err.to_string(),
            );
        }
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if let Some(linked_job_id) = job.linked_job_id {
        if let Ok(Some(linked_job)) = state.db.get_job_by_id(linked_job_id).await {
            if linked_job.is_active() {
                return api_error_response(
                    StatusCode::CONFLICT,
                    "CONVERSION_JOB_ACTIVE",
                    "conversion job is still active",
                );
            }
            let _ = state.db.delete_job(linked_job_id).await;
        }
    }

    if let Err(err) = remove_conversion_artifacts(&job).await {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CONVERSION_ARTIFACT_REMOVE_FAILED",
            err.to_string(),
        );
    }
    if let Err(err) = state.db.delete_conversion_job(id).await {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CONVERSION_JOB_DELETE_FAILED",
            err.to_string(),
        );
    }
    StatusCode::OK.into_response()
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect()
}
