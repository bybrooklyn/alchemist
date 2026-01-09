use crate::config::Config;
use crate::db::Decision;
use crate::error::Result;
use crate::media::pipeline::{MediaMetadata, Planner};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;

pub struct BasicPlanner {
    config: Arc<Config>,
}

impl BasicPlanner {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Planner for BasicPlanner {
    async fn plan(&self, metadata: &MediaMetadata) -> Result<Decision> {
        let (should_transcode, reason) = should_transcode(metadata, &self.config);

        // We don't have job_id here in plan() signature...
        // Wait, Decision struct requires job_id, created_at, id (db fields).
        // The Planner should probably return a "Plan" or "DecisionIntent", not the DB struct Decision.
        // OR the trait should take job_id?
        // Let's check the Decision struct in db.rs again.
        // It has `id`, `job_id`, `action`, `reason`, `created_at`.

        // Ideally Planner returns an enum or struct describing the intent, and the caller (Agent) saves it to DB.
        // The trait currently says Result<Decision>.
        // I will return a dummy Decision with id=0, job_id=0, created_at=now.
        // The caller will likely persist it or use the action/reason.
        // Actually, looking at implementation_plan, it says `Result<Decision>`.
        // I should probably update the trait or the struct usage.
        // For now, I'll return the struct with 0s, assuming caller handles DB insertion or we change the trait later.

        Ok(Decision {
            id: 0,     // Placeholder
            job_id: 0, // Placeholder
            action: if should_transcode {
                "encode".to_string()
            } else {
                "skip".to_string()
            },
            reason,
            created_at: Utc::now(),
        })
    }
}

fn should_transcode(metadata: &MediaMetadata, config: &Config) -> (bool, String) {
    // 1. Codec Check (skip if already AV1 + 10-bit)
    if metadata.codec_name == "av1" && metadata.bit_depth == 10 {
        return (false, "Already AV1 10-bit".to_string());
    }

    // 2. Efficiency Rules (BPP)
    let bitrate = metadata.bit_rate;
    let width = metadata.width as f64;
    let height = metadata.height as f64;
    let fps = metadata.fps;

    if width == 0.0 || height == 0.0 || bitrate == 0.0 {
        return (
            false,
            "Incomplete metadata (bitrate/resolution)".to_string(),
        );
    }

    let bpp = bitrate / (width * height * fps);

    // Normalize BPP based on resolution
    let res_correction = if width >= 3840.0 {
        0.6 // 4K
    } else if width >= 1920.0 {
        0.8 // 1080p
    } else {
        1.0 // 720p and below
    };
    let normalized_bpp = bpp * res_correction;

    // Heuristic via config
    if normalized_bpp < config.transcode.min_bpp_threshold {
        return (
            false,
            format!(
                "BPP too low ({:.4} normalized < {:.2}), avoiding quality murder",
                normalized_bpp, config.transcode.min_bpp_threshold
            ),
        );
    }

    // 3. Projected Size Logic
    let size_bytes = metadata.size_bytes;
    let min_size_bytes = config.transcode.min_file_size_mb * 1024 * 1024;
    if size_bytes < min_size_bytes {
        return (
            false,
            format!(
                "File too small ({}MB < {}MB) to justify transcode overhead",
                size_bytes / 1024 / 1024,
                config.transcode.min_file_size_mb
            ),
        );
    }

    (
        true,
        format!(
            "Ready for AV1 transcode (Current codec: {}, BPP: {:.4})",
            metadata.codec_name, bpp
        ),
    )
}
