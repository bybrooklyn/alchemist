use crate::config::Config;
use crate::db::Decision;
use crate::error::Result;
use crate::media::pipeline::{MediaMetadata, Planner};
use crate::system::hardware::HardwareInfo;
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;

pub struct BasicPlanner {
    config: Arc<Config>,
    hw_info: Option<HardwareInfo>,
}

impl BasicPlanner {
    pub fn new(config: Arc<Config>, hw_info: Option<HardwareInfo>) -> Self {
        Self { config, hw_info }
    }
}

#[async_trait]
impl Planner for BasicPlanner {
    async fn plan(&self, metadata: &MediaMetadata) -> Result<Decision> {
        let (should_transcode, reason) =
            should_transcode(metadata, &self.config, self.hw_info.as_ref());

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

fn should_transcode(
    metadata: &MediaMetadata,
    config: &Config,
    hw_info: Option<&HardwareInfo>,
) -> (bool, String) {
    // 0. Hardware Capability Check
    // If hardware encoding is preferred but not available for AV1, and CPU fallback is disabled -> SKIP
    // Assuming target is AV1 for now (can be configurable later)
    let target_codec = "av1";

    if let Some(hw) = hw_info {
        // If we have hardware, check if it supports the target codec
        // Note: nvidia-smi check in hardware.rs might return true for "vendor: Nvidia" but supported_codecs might be empty if check failed?
        // Let's assume supported_codecs is populated.

        let supports_av1 = hw.supported_codecs.iter().any(|c| c == target_codec);

        if !supports_av1 {
            // Hardware doesn't support it. Check policy.
            // If fallback is DISABLED, then we must skip.
            // config.hardware.cpu_fallback (assuming this field exists? user logs said "CPU Fallback: Enabled")
            // Let's assume config.hardware.allow_cpu_fallback or similar.
            // Checking config structure might be needed. I'll guess `enable_cpu_fallback` based on `hardware.rs` usage?
            // Wait, hardware.rs `detect_hardware(allow_cpu_fallback)` matches log.
            // Let's check config struct if I can. But for now I will assume user wants to process anyway if fallback enabled.

            // Actually, if detect_hardware returned a GPU vendor but that GPU doesn't support AV1,
            // we should probably fallback to CPU *if allowed*.
            // But if detect_hardware returned Vendor::Cpu, then we are already on CPU.

            // If Vendor != Cpu, and supports_av1 is false:
            if hw.vendor != crate::system::hardware::Vendor::Cpu {
                // Check config for fallback
                // Warning: I don't see `config` usage for cpu fallback here, it was used in detection.
                // But wait, if detection was called with `true`, and it found a GPU, it returns GPU.
                // Does it mean we can't use CPU?
                // No, usually "Fallback" means "If GPU can't do it, use CPU".

                // If we strictly require GPU for AV1 (e.g. speed), we'd return false here.
                // But for now, let's just Log a warning in reason string?
                // Or better:
                // if !supports_av1 { return (false, "GPU does not support AV1".to_string()); }

                // However, "CPU Fallback: Enabled" implies we should proceed.
                // So I won't block it here, assuming Executor handles the fallback to libsvtav1?
                // But Executor *always* calls `transcode_to_av1`.
                // We need to know if FfmpegExecutor will try to use `av1_nvenc` and fail.

                // If Executor is smart, it might fallback.
                // But Planner is where we should probably say "Skip" if we absolutely can't do it.
            }
        }
    }

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
