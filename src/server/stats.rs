//! Statistics and savings dashboard handlers.

use super::{AppState, config_read_error_response};
use crate::db::Db;
use crate::error::Result;
use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use prometheus::{Encoder, IntGauge, IntGaugeVec, Opts, Registry, TextEncoder};
use std::sync::Arc;

pub(crate) struct StatsData {
    pub(crate) total: i64,
    pub(crate) completed: i64,
    pub(crate) active: i64,
    pub(crate) failed: i64,
    pub(crate) concurrent_limit: usize,
}

pub(crate) async fn get_stats_data(db: &Db, concurrent_limit: usize) -> Result<StatsData> {
    let s = db.get_stats().await?;
    let total = s
        .as_object()
        .map(|m| m.values().filter_map(|v| v.as_i64()).sum::<i64>())
        .unwrap_or(0);
    let completed = s.get("completed").and_then(|v| v.as_i64()).unwrap_or(0);
    let active = s
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| {
                    ["encoding", "analyzing", "remuxing", "resuming"].contains(&k.as_str())
                })
                .map(|(_, v)| v.as_i64().unwrap_or(0))
                .sum::<i64>()
        })
        .unwrap_or(0);
    let failed = s.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);

    Ok(StatsData {
        total,
        completed,
        active,
        failed,
        concurrent_limit,
    })
}

pub(crate) async fn stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match get_stats_data(&state.db, state.agent.concurrent_jobs_limit()).await {
        Ok(stats) => axum::Json(serde_json::json!({
            "total": stats.total,
            "completed": stats.completed,
            "active": stats.active,
            "failed": stats.failed,
            "concurrent_limit": stats.concurrent_limit
        }))
        .into_response(),
        Err(err) => config_read_error_response("load job stats", &err),
    }
}

pub(crate) async fn aggregated_stats_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
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
            .into_response()
        }
        Err(err) => config_read_error_response("load aggregated stats", &err),
    }
}

pub(crate) async fn daily_stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_daily_stats(30).await {
        Ok(stats) => axum::Json(serde_json::json!(stats)).into_response(),
        Err(err) => config_read_error_response("load daily stats", &err),
    }
}

pub(crate) async fn detailed_stats_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_detailed_encode_stats(50).await {
        Ok(stats) => axum::Json(serde_json::json!(stats)).into_response(),
        Err(err) => config_read_error_response("load detailed stats", &err),
    }
}

pub(crate) async fn savings_summary_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_savings_summary().await {
        Ok(summary) => axum::Json(summary).into_response(),
        Err(err) => config_read_error_response("load storage savings summary", &err),
    }
}

pub(crate) async fn skip_reasons_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_skip_reason_counts().await {
        Ok(counts) => {
            let items: Vec<serde_json::Value> = counts
                .into_iter()
                .map(|(code, count)| serde_json::json!({ "code": code, "count": count }))
                .collect();
            axum::Json(serde_json::json!({ "today": items })).into_response()
        }
        Err(err) => config_read_error_response("load skip reason counts", &err),
    }
}

pub(crate) async fn metrics_handler(State(state): State<Arc<AppState>>) -> Response {
    let metrics_enabled = {
        let config = state.config.read().await;
        config.system.metrics_enabled
    };
    if !metrics_enabled {
        return StatusCode::NOT_FOUND.into_response();
    }

    let status_counts = match state.db.get_status_counts().await {
        Ok(counts) => counts,
        Err(err) => return config_read_error_response("load metrics status counts", &err),
    };
    let aggregated = match state.db.get_aggregated_stats().await {
        Ok(stats) => stats,
        Err(err) => return config_read_error_response("load aggregated metrics", &err),
    };

    let registry = Registry::new();
    let jobs_by_status = match IntGaugeVec::new(
        Opts::new(
            "alchemist_jobs_total",
            "Current non-archived jobs grouped by status",
        ),
        &["status"],
    ) {
        Ok(metric) => metric,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create jobs metric: {err}"),
            )
                .into_response();
        }
    };
    let completed_jobs = match IntGauge::new(
        "alchemist_completed_jobs_total",
        "Total completed non-archived jobs",
    ) {
        Ok(metric) => metric,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create completed-jobs metric: {err}"),
            )
                .into_response();
        }
    };
    let bytes_saved = match IntGauge::new(
        "alchemist_bytes_saved_total",
        "Total bytes saved across completed encodes",
    ) {
        Ok(metric) => metric,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create bytes-saved metric: {err}"),
            )
                .into_response();
        }
    };

    for (status, count) in status_counts {
        jobs_by_status.with_label_values(&[&status]).set(count);
    }
    completed_jobs.set(aggregated.completed_jobs.max(0) as i64);
    bytes_saved.set(
        aggregated
            .total_input_size
            .saturating_sub(aggregated.total_output_size)
            .max(0) as i64,
    );

    if let Err(err) = registry.register(Box::new(jobs_by_status.clone())) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to register jobs metric: {err}"),
        )
            .into_response();
    }
    if let Err(err) = registry.register(Box::new(completed_jobs.clone())) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to register completed-jobs metric: {err}"),
        )
            .into_response();
    }
    if let Err(err) = registry.register(Box::new(bytes_saved.clone())) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to register bytes-saved metric: {err}"),
        )
            .into_response();
    }

    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    if let Err(err) = encoder.encode(&metric_families, &mut buffer) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encode metrics: {err}"),
        )
            .into_response();
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(encoder.format_type())
            .unwrap_or_else(|_| HeaderValue::from_static("text/plain; version=0.0.4")),
    );

    (headers, buffer).into_response()
}
