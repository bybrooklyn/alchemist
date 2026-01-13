use crate::db::{Decision, Job};
use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub path: PathBuf,
    pub duration_secs: f64,
    pub codec_name: String,
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_primaries: Option<String>,
    pub color_transfer: Option<String>,
    pub color_space: Option<String>,
    pub color_range: Option<String>,
    pub is_hdr: bool,
    pub size_bytes: u64,
    pub bit_rate: f64,
    pub fps: f64,
    pub container: String,
    pub audio_codec: Option<String>,
    pub audio_channels: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub encode_time_secs: f64,
    pub input_size: u64,
    pub output_size: u64,
    pub vmaf: Option<f64>,
}

#[async_trait]
pub trait Analyzer: Send + Sync {
    async fn analyze(&self, path: &Path) -> Result<MediaMetadata>;
}

#[async_trait]
pub trait Planner: Send + Sync {
    async fn plan(&self, metadata: &MediaMetadata) -> Result<Decision>;
}

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute(
        &self,
        job: &Job,
        decision: &Decision,
        metadata: &MediaMetadata,
    ) -> Result<ExecutionStats>;
}
