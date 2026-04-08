use super::AppState;
use crate::conversion::ConversionSettings;
use crate::media::pipeline::Analyzer as _;
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio_util::io::ReaderStream;

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

async fn cleanup_expired_jobs(state: &AppState) {
    let now = chrono::Utc::now().to_rfc3339();
    let expired = match state.db.get_expired_conversion_jobs(&now).await {
        Ok(expired) => expired,
        Err(_) => return,
    };

    for job in expired {
        let _ = remove_conversion_artifacts(&job).await;
        let _ = state.db.delete_conversion_job(job.id).await;
    }
}

async fn remove_conversion_artifacts(job: &crate::db::ConversionJob) -> std::io::Result<()> {
    let upload_path = FsPath::new(&job.upload_path);
    if upload_path.exists() {
        let _ = fs::remove_file(upload_path).await;
    }
    if let Some(output_path) = &job.output_path {
        let output_path = FsPath::new(output_path);
        if output_path.exists() {
            let _ = fs::remove_file(output_path).await;
        }
    }
    Ok(())
}

pub(crate) async fn upload_conversion_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    cleanup_expired_jobs(state.as_ref()).await;

    let upload_id = uuid::Uuid::new_v4().to_string();
    let upload_dir = uploads_root().join(&upload_id);
    if let Err(err) = fs::create_dir_all(&upload_dir).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
    }

    let field = match multipart.next_field().await {
        Ok(Some(field)) => field,
        Ok(None) => return (StatusCode::BAD_REQUEST, "missing upload file").into_response(),
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    let stored_path: PathBuf = {
        let file_name = field
            .file_name()
            .map(sanitize_filename)
            .unwrap_or_else(|| "input.bin".to_string());
        let path = upload_dir.join(file_name);
        match field.bytes().await {
            Ok(bytes) => {
                if let Err(err) = fs::write(&path, bytes).await {
                    return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
                }
                path
            }
            Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
        }
    };

    let analyzer = crate::media::analyzer::FfmpegAnalyzer;
    let analysis = match analyzer.analyze(&stored_path).await {
        Ok(analysis) => analysis,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };

    let settings = ConversionSettings::default();
    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
    let conversion_job = match state
        .db
        .create_conversion_job(
            &stored_path.to_string_lossy(),
            if settings.remux_only {
                "remux"
            } else {
                "transcode"
            },
            &serde_json::to_string(&settings).unwrap_or_else(|_| "{}".to_string()),
            Some(&serde_json::to_string(&analysis).unwrap_or_else(|_| "{}".to_string())),
            &expires_at,
        )
        .await
    {
        Ok(job) => job,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
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
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let analysis: crate::media::pipeline::MediaAnalysis = match job.probe_json.as_deref() {
        Some(probe_json) => match serde_json::from_str(probe_json) {
            Ok(analysis) => analysis,
            Err(err) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
            }
        },
        None => return (StatusCode::BAD_REQUEST, "missing conversion probe").into_response(),
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
            let _ = state
                .db
                .update_conversion_job_probe(
                    job.id,
                    &serde_json::to_string(&analysis).unwrap_or_else(|_| "{}".to_string()),
                )
                .await;
            let _ = state
                .db
                .update_conversion_job_status(
                    job.id,
                    if preview.normalized_settings.remux_only {
                        "draft_remux"
                    } else {
                        "draft_transcode"
                    },
                )
                .await;
            let _ = sqlx_update_conversion_settings(
                state.as_ref(),
                job.id,
                &preview.normalized_settings,
            )
            .await;
            axum::Json(preview).into_response()
        }
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

async fn sqlx_update_conversion_settings(
    state: &AppState,
    id: i64,
    settings: &ConversionSettings,
) -> crate::error::Result<()> {
    state
        .db
        .update_conversion_job_settings(
            id,
            &serde_json::to_string(settings).unwrap_or_else(|_| "{}".to_string()),
            if settings.remux_only {
                "remux"
            } else {
                "transcode"
            },
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
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if job.linked_job_id.is_some() {
        return (StatusCode::CONFLICT, "conversion job already started").into_response();
    }

    let input_path = PathBuf::from(&job.upload_path);
    let file_stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let settings: ConversionSettings = match serde_json::from_str(&job.settings_json) {
        Ok(settings) => settings,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };

    let output_dir = outputs_root().join(job.id.to_string());
    if let Err(err) = fs::create_dir_all(&output_dir).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
    }
    let output_path = output_dir.join(format!("{file_stem}.{}", settings.output_container));
    let mtime = std::fs::metadata(&input_path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(std::time::SystemTime::now());

    if let Err(err) = state.db.enqueue_job(&input_path, &output_path, mtime).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
    }
    let linked_job = match state
        .db
        .get_job_by_input_path(&input_path.to_string_lossy())
        .await
    {
        Ok(Some(job)) => job,
        Ok(None) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "linked job missing").into_response();
        }
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };
    if let Err(err) = state
        .db
        .update_conversion_job_start(id, &output_path.to_string_lossy(), linked_job.id)
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
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
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let linked_job = match conversion_job.linked_job_id {
        Some(job_id) => match state.db.get_job_by_id(job_id).await {
            Ok(job) => job,
            Err(err) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
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
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
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
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };
    let file_name = FsPath::new(&output_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output.bin");
    let _ = state.db.mark_conversion_job_downloaded(id).await;

    let stream = ReaderStream::new(file);
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
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if let Some(linked_job_id) = job.linked_job_id {
        if let Ok(Some(linked_job)) = state.db.get_job_by_id(linked_job_id).await {
            if linked_job.is_active() {
                return (StatusCode::CONFLICT, "conversion job is still active").into_response();
            }
            let _ = state.db.delete_job(linked_job_id).await;
        }
    }

    if let Err(err) = remove_conversion_artifacts(&job).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
    }
    if let Err(err) = state.db.delete_conversion_job(id).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
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
