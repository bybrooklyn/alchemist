use crate::explanations::Explanation;
use serde::{Deserialize, Serialize};

use super::types::JobState;

// Typed event channels for separating high-volume vs low-volume events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum JobEvent {
    StateChanged {
        job_id: i64,
        status: JobState,
    },
    Progress {
        job_id: i64,
        percentage: f64,
        time: String,
    },
    Decision {
        job_id: i64,
        action: String,
        reason: String,
        explanation: Option<Explanation>,
    },
    Log {
        level: String,
        job_id: Option<i64>,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConfigEvent {
    Updated(Box<crate::config::Config>),
    WatchFolderAdded(String),
    WatchFolderRemoved(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SystemEvent {
    ScanStarted,
    ScanCompleted,
    EngineIdle,
    EngineStatusChanged,
    HardwareStateChanged,
}

pub struct EventChannels {
    pub jobs: tokio::sync::broadcast::Sender<JobEvent>, // 1000 capacity - high volume
    pub config: tokio::sync::broadcast::Sender<ConfigEvent>, // 50 capacity - rare
    pub system: tokio::sync::broadcast::Sender<SystemEvent>, // 100 capacity - medium
}
