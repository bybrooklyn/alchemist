//! Server-sent events (SSE) streaming.

use crate::db::{ConfigEvent, JobEvent, SystemEvent};
use axum::{
    extract::State,
    response::sse::{Event as AxumEvent, Sse},
};
use futures::stream::{self, Stream, StreamExt};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::warn;

use super::AppState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SseMessage {
    pub(crate) event_name: &'static str,
    pub(crate) data: String,
}

impl From<SseMessage> for AxumEvent {
    fn from(message: SseMessage) -> Self {
        AxumEvent::default()
            .event(message.event_name)
            .data(message.data)
    }
}

pub(crate) fn sse_message_for_job_event(event: &JobEvent) -> SseMessage {
    match event {
        JobEvent::Log {
            level,
            job_id,
            message,
        } => SseMessage {
            event_name: "log",
            data: serde_json::json!({
                "level": level,
                "job_id": job_id,
                "message": message
            })
            .to_string(),
        },
        JobEvent::Progress {
            job_id,
            percentage,
            time,
        } => SseMessage {
            event_name: "progress",
            data: serde_json::json!({
                "job_id": job_id,
                "percentage": percentage,
                "time": time
            })
            .to_string(),
        },
        JobEvent::StateChanged { job_id, status } => SseMessage {
            event_name: "status",
            data: serde_json::json!({
                "job_id": job_id,
                "status": status
            })
            .to_string(),
        },
        JobEvent::Decision {
            job_id,
            action,
            reason,
            explanation,
        } => SseMessage {
            event_name: "decision",
            data: serde_json::json!({
                "job_id": job_id,
                "action": action,
                "reason": reason,
                "explanation": explanation
            })
            .to_string(),
        },
    }
}

pub(crate) fn sse_message_for_config_event(event: &ConfigEvent) -> SseMessage {
    match event {
        ConfigEvent::Updated(config) => SseMessage {
            event_name: "config_updated",
            data: serde_json::to_string(config).unwrap_or_else(|_| "{}".to_string()),
        },
        ConfigEvent::WatchFolderAdded(path) => SseMessage {
            event_name: "watch_folder_added",
            data: serde_json::json!({ "path": path }).to_string(),
        },
        ConfigEvent::WatchFolderRemoved(path) => SseMessage {
            event_name: "watch_folder_removed",
            data: serde_json::json!({ "path": path }).to_string(),
        },
    }
}

pub(crate) fn sse_message_for_system_event(event: &SystemEvent) -> SseMessage {
    match event {
        SystemEvent::ScanStarted => SseMessage {
            event_name: "scan_started",
            data: "{}".to_string(),
        },
        SystemEvent::ScanCompleted => SseMessage {
            event_name: "scan_completed",
            data: "{}".to_string(),
        },
        SystemEvent::EngineStatusChanged => SseMessage {
            event_name: "engine_status_changed",
            data: "{}".to_string(),
        },
        SystemEvent::HardwareStateChanged => SseMessage {
            event_name: "hardware_state_changed",
            data: "{}".to_string(),
        },
    }
}

pub(crate) fn sse_lagged_message(skipped: u64) -> SseMessage {
    SseMessage {
        event_name: "lagged",
        data: serde_json::json!({ "skipped": skipped }).to_string(),
    }
}

pub(crate) fn sse_unified_stream(
    job_rx: broadcast::Receiver<JobEvent>,
    config_rx: broadcast::Receiver<ConfigEvent>,
    system_rx: broadcast::Receiver<SystemEvent>,
) -> impl Stream<Item = std::result::Result<SseMessage, Infallible>> {
    // Create individual streams for each event type
    let job_stream = stream::unfold(job_rx, |mut rx| async move {
        match rx.recv().await {
            Ok(event) => Some((Ok(sse_message_for_job_event(&event)), rx)),
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!("SSE subscriber lagged on job events; skipped {skipped} events");
                Some((Ok(sse_lagged_message(skipped)), rx))
            }
            Err(broadcast::error::RecvError::Closed) => None,
        }
    });

    let config_stream = stream::unfold(config_rx, |mut rx| async move {
        match rx.recv().await {
            Ok(event) => Some((Ok(sse_message_for_config_event(&event)), rx)),
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!("SSE subscriber lagged on config events; skipped {skipped} events");
                Some((Ok(sse_lagged_message(skipped)), rx))
            }
            Err(broadcast::error::RecvError::Closed) => None,
        }
    });

    let system_stream = stream::unfold(system_rx, |mut rx| async move {
        match rx.recv().await {
            Ok(event) => Some((Ok(sse_message_for_system_event(&event)), rx)),
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!("SSE subscriber lagged on system events; skipped {skipped} events");
                Some((Ok(sse_lagged_message(skipped)), rx))
            }
            Err(broadcast::error::RecvError::Closed) => None,
        }
    });

    // Merge all streams - this will interleave events from all channels
    futures::stream::select_all([
        job_stream.boxed(),
        config_stream.boxed(),
        system_stream.boxed(),
    ])
}

/// Maximum concurrent SSE connections to prevent resource exhaustion.
const MAX_SSE_CONNECTIONS: usize = 50;

/// RAII guard that decrements the SSE connection counter on drop.
struct SseConnectionGuard(Arc<std::sync::atomic::AtomicUsize>);

impl Drop for SseConnectionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

pub(crate) async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> std::result::Result<
    Sse<impl Stream<Item = std::result::Result<AxumEvent, Infallible>>>,
    axum::http::StatusCode,
> {
    use std::sync::atomic::Ordering;

    // Enforce connection limit
    let current = state.sse_connections.fetch_add(1, Ordering::SeqCst);
    if current >= MAX_SSE_CONNECTIONS {
        state.sse_connections.fetch_sub(1, Ordering::SeqCst);
        warn!(
            "SSE connection limit reached ({}/{}). Rejecting new connection.",
            current, MAX_SSE_CONNECTIONS
        );
        return Err(axum::http::StatusCode::TOO_MANY_REQUESTS);
    }

    // RAII guard to decrement the counter when the stream is dropped
    let guard = Arc::new(SseConnectionGuard(state.sse_connections.clone()));

    // Subscribe to all channels
    let job_rx = state.event_channels.jobs.subscribe();
    let config_rx = state.event_channels.config.subscribe();
    let system_rx = state.event_channels.system.subscribe();

    // Create unified stream from new typed channels
    let unified_stream = sse_unified_stream(job_rx, config_rx, system_rx);

    let stream = unified_stream.map(move |message| {
        let _guard = guard.clone(); // keep the guard alive as long as the stream lives
        match message {
            Ok(message) => Ok(message.into()),
            Err(never) => match never {},
        }
    });

    Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}
