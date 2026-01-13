use crate::config::Config;
use crate::db::Decision;
use crate::error::Result;
use crate::media::pipeline::{MediaMetadata, Planner};
use crate::system::hardware::{HardwareInfo, Vendor};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;

pub struct BasicPlanner {
    config: Arc<Config>,
    hw_info: Option<HardwareInfo>,
    encoder_caps: Arc<crate::media::ffmpeg::EncoderCapabilities>,
}

impl BasicPlanner {
    pub fn new(
        config: Arc<Config>,
        hw_info: Option<HardwareInfo>,
        encoder_caps: Arc<crate::media::ffmpeg::EncoderCapabilities>,
    ) -> Self {
        Self {
            config,
            hw_info,
            encoder_caps,
        }
    }
}

#[async_trait]
impl Planner for BasicPlanner {
    async fn plan(&self, metadata: &MediaMetadata) -> Result<Decision> {
        let (should_transcode, reason) = should_transcode(
            metadata,
            &self.config,
            self.hw_info.as_ref(),
            &self.encoder_caps,
        );

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
    encoder_caps: &crate::media::ffmpeg::EncoderCapabilities,
) -> (bool, String) {
    // 0. Hardware Capability Check
    let target_codec = config.transcode.output_codec;
    let target_codec_str = target_codec.as_str();

    if !config.transcode.allow_fallback {
        let preferred_available = encoder_available_for_codec(target_codec, hw_info, encoder_caps);
        if !preferred_available {
            return (
                false,
                format!(
                    "Preferred codec {} unavailable and fallback disabled",
                    target_codec_str
                ),
            );
        }
    }

    if let Some(hw) = hw_info {
        if hw.vendor == Vendor::Cpu && !config.hardware.allow_cpu_encoding {
            return (false, "CPU encoding disabled in configuration".to_string());
        }
        // If we have hardware, check if it supports the target codec
        let supports_codec = hw.supported_codecs.iter().any(|c| c == target_codec_str);

        if !supports_codec {
            // Hardware doesn't support it. Check policy.
            // If fallback is DISABLED, then we must skip.
            if !config.hardware.allow_cpu_fallback {
                return (
                    false,
                    format!(
                        "Hardware {:?} does not support {}, and CPU fallback is disabled",
                        hw.vendor, target_codec_str
                    ),
                );
            }
            if !config.hardware.allow_cpu_encoding {
                return (false, "CPU encoding disabled in configuration".to_string());
            }
            // If fallback is enabled, we proceed (will be slow!)
        }
    } else {
        if !config.hardware.allow_cpu_fallback {
            return (
                false,
                format!(
                    "No hardware detected for {}, and CPU fallback is disabled",
                    target_codec_str
                ),
            );
        }
        if !config.hardware.allow_cpu_encoding {
            return (false, "CPU encoding disabled in configuration".to_string());
        }
    }

    // 1. Codec Check (skip if already target codec + 10-bit, or H.264 preferred and already H.264)
    if metadata.codec_name.eq_ignore_ascii_case(target_codec_str) && metadata.bit_depth == 10 {
        return (false, format!("Already {} 10-bit", target_codec_str));
    }

    if target_codec == crate::config::OutputCodec::H264
        && metadata.codec_name.eq_ignore_ascii_case("h264")
        && metadata.bit_depth <= 8
    {
        return (false, "Already H.264".to_string());
    }

    // 1b. Always transcode H.264 sources
    if metadata.codec_name.eq_ignore_ascii_case("h264") {
        return (
            true,
            "H.264 source prioritized for transcode".to_string(),
        );
    }

    // 2. Efficiency Rules (BPP)
    let bitrate = metadata.bit_rate;
    let width = metadata.width as f64;
    let height = metadata.height as f64;
    let fps = metadata.fps;

    if width == 0.0 || height == 0.0 {
        return (
            false,
            "Incomplete metadata (resolution missing)".to_string(),
        );
    }

    let bpp = if bitrate > 0.0 && fps > 0.0 {
        bitrate / (width * height * fps)
    } else {
        0.0
    };

    // Normalize BPP based on resolution
    let res_correction = if width >= 3840.0 {
        0.6 // 4K
    } else if width >= 1920.0 {
        0.8 // 1080p
    } else {
        1.0 // 720p and below
    };
    let normalized_bpp = bpp * res_correction;

    // Heuristic via config (only if bitrate/fps are known)
    if bpp > 0.0 && normalized_bpp < config.transcode.min_bpp_threshold {
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

    let reason = if bpp > 0.0 {
        format!(
            "Ready for {} transcode (Current codec: {}, BPP: {:.4})",
            target_codec_str, metadata.codec_name, bpp
        )
    } else {
        format!(
            "Ready for {} transcode (Current codec: {}, bitrate/fps unknown)",
            target_codec_str, metadata.codec_name
        )
    };

    (true, reason)
}

fn encoder_available_for_codec(
    target_codec: crate::config::OutputCodec,
    hw_info: Option<&HardwareInfo>,
    encoder_caps: &crate::media::ffmpeg::EncoderCapabilities,
) -> bool {
    let hw_vendor = hw_info.map(|h| h.vendor);
    match target_codec {
        crate::config::OutputCodec::Av1 => {
            matches!(hw_vendor, Some(Vendor::Apple))
                && encoder_caps.has_video_encoder("av1_videotoolbox")
                || matches!(hw_vendor, Some(Vendor::Intel))
                    && encoder_caps.has_video_encoder("av1_qsv")
                || matches!(hw_vendor, Some(Vendor::Nvidia))
                    && encoder_caps.has_video_encoder("av1_nvenc")
                || matches!(hw_vendor, Some(Vendor::Amd))
                    && encoder_caps.has_video_encoder(if cfg!(target_os = "windows") { "av1_amf" } else { "av1_vaapi" })
                || encoder_caps.has_libsvtav1()
                || encoder_caps.has_video_encoder("libaom-av1")
        }
        crate::config::OutputCodec::Hevc => {
            matches!(hw_vendor, Some(Vendor::Apple))
                && encoder_caps.has_video_encoder("hevc_videotoolbox")
                || matches!(hw_vendor, Some(Vendor::Intel))
                    && encoder_caps.has_video_encoder("hevc_qsv")
                || matches!(hw_vendor, Some(Vendor::Nvidia))
                    && encoder_caps.has_video_encoder("hevc_nvenc")
                || matches!(hw_vendor, Some(Vendor::Amd))
                    && encoder_caps.has_video_encoder(if cfg!(target_os = "windows") { "hevc_amf" } else { "hevc_vaapi" })
                || encoder_caps.has_libx265()
        }
        crate::config::OutputCodec::H264 => {
            matches!(hw_vendor, Some(Vendor::Apple))
                && encoder_caps.has_video_encoder("h264_videotoolbox")
                || matches!(hw_vendor, Some(Vendor::Intel))
                    && encoder_caps.has_video_encoder("h264_qsv")
                || matches!(hw_vendor, Some(Vendor::Nvidia))
                    && encoder_caps.has_video_encoder("h264_nvenc")
                || matches!(hw_vendor, Some(Vendor::Amd))
                    && encoder_caps.has_video_encoder(if cfg!(target_os = "windows") { "h264_amf" } else { "h264_vaapi" })
                || encoder_caps.has_libx264()
        }
    }
}
