//! Server-sent events (SSE) streaming.

use crate::db::{AlchemistEvent, ConfigEvent, JobEvent, SystemEvent};
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

pub(crate) fn sse_message_for_event(event: &AlchemistEvent) -> SseMessage {
    match event {
        AlchemistEvent::Log {
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
        AlchemistEvent::Progress {
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
        AlchemistEvent::JobStateChanged { job_id, status } => SseMessage {
            event_name: "status",
            data: serde_json::json!({
                "job_id": job_id,
                "status": status
            })
            .to_string(),
        },
        AlchemistEvent::Decision {
            job_id,
            action,
            reason,
        } => SseMessage {
            event_name: "decision",
            data: serde_json::json!({
                "job_id": job_id,
                "action": action,
                "reason": reason
            })
            .to_string(),
        },
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
        } => SseMessage {
            event_name: "decision",
            data: serde_json::json!({
                "job_id": job_id,
                "action": action,
                "reason": reason
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

pub(crate) fn sse_message_stream(
    rx: broadcast::Receiver<AlchemistEvent>,
) -> impl Stream<Item = std::result::Result<SseMessage, Infallible>> {
    stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(event) => Some((Ok(sse_message_for_event(&event)), rx)),
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!("SSE subscriber lagged; skipped {skipped} events");
                Some((Ok(sse_lagged_message(skipped)), rx))
            }
            Err(broadcast::error::RecvError::Closed) => None,
        }
    })
}

pub(crate) async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = std::result::Result<AxumEvent, Infallible>>> {
    // Subscribe to all channels
    let job_rx = state.event_channels.jobs.subscribe();
    let config_rx = state.event_channels.config.subscribe();
    let system_rx = state.event_channels.system.subscribe();
    let legacy_rx = state.tx.subscribe();

    // Create unified stream from new typed channels
    let unified_stream = sse_unified_stream(job_rx, config_rx, system_rx);

    // Create legacy stream for backwards compatibility
    let legacy_stream = sse_message_stream(legacy_rx);

    // Merge both streams
    let combined_stream =
        futures::stream::select(unified_stream, legacy_stream).map(|message| match message {
            Ok(message) => Ok(message.into()),
            Err(never) => match never {},
        });

    Sse::new(combined_stream).keep_alive(axum::response::sse::KeepAlive::default())
}
