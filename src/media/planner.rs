use crate::config::Config;
use crate::error::Result;
use crate::media::pipeline::{
    Encoder, ExecutionPlan, HardwareCapabilities, MediaAnalysis, Planner, RateControl,
    TranscodeDecision,
};
use crate::system::hardware::{HardwareInfo, Vendor};
use async_trait::async_trait;
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
    async fn plan(
        &self,
        analysis: &MediaAnalysis,
        hardware: &HardwareCapabilities,
        output_extension: &str,
    ) -> Result<ExecutionPlan> {
        let decision = should_transcode(
            analysis,
            &self.config,
            self.hw_info.as_ref(),
            &self.encoder_caps,
        );

        let (decision, encoder, rate_control) =
            if matches!(decision, TranscodeDecision::Transcode { .. }) {
                let encoder = select_encoder(
                    self.config.transcode.output_codec,
                    hardware,
                    self.config.transcode.allow_fallback,
                );
                if let Some(selected) = encoder {
                    let rate_control = default_rate_control(&selected, &self.config);
                    (decision, Some(selected), Some(rate_control))
                } else {
                    (
                        TranscodeDecision::Skip {
                            reason: "No suitable encoder available".to_string(),
                        },
                        None,
                        None,
                    )
                }
            } else {
                (decision, None, None)
            };

        Ok(ExecutionPlan {
            decision,
            output_path: None,
            target_container: Some(output_extension.to_string()),
            encoder,
            rate_control,
            allow_fallback: self.config.transcode.allow_fallback,
        })
    }
}

fn should_transcode(
    analysis: &MediaAnalysis,
    config: &Config,
    hw_info: Option<&HardwareInfo>,
    encoder_caps: &crate::media::ffmpeg::EncoderCapabilities,
) -> TranscodeDecision {
    let metadata = &analysis.metadata;
    // 0. Hardware Capability Check
    let target_codec = config.transcode.output_codec;
    let target_codec_str = target_codec.as_str();

    if !config.transcode.allow_fallback {
        let preferred_available = encoder_available_for_codec(target_codec, hw_info, encoder_caps);
        if !preferred_available {
            return TranscodeDecision::Skip {
                reason: format!(
                    "Preferred codec {} unavailable and fallback disabled",
                    target_codec_str
                ),
            };
        }
    }

    if let Some(hw) = hw_info {
        if hw.vendor == Vendor::Cpu && !config.hardware.allow_cpu_encoding {
            return TranscodeDecision::Skip {
                reason: "CPU encoding disabled in configuration".to_string(),
            };
        }
        // If we have hardware, check if it supports the target codec
        let supports_codec = hw.supported_codecs.iter().any(|c| c == target_codec_str);

        if !supports_codec {
            // Hardware doesn't support it. Check policy.
            // If fallback is DISABLED, then we must skip.
            if !config.hardware.allow_cpu_fallback {
                return TranscodeDecision::Skip {
                    reason: format!(
                        "Hardware {:?} does not support {}, and CPU fallback is disabled",
                        hw.vendor, target_codec_str
                    ),
                };
            }
            if !config.hardware.allow_cpu_encoding {
                return TranscodeDecision::Skip {
                    reason: "CPU encoding disabled in configuration".to_string(),
                };
            }
            // If fallback is enabled, we proceed (will be slow!)
        }
    } else {
        if !config.hardware.allow_cpu_fallback {
            return TranscodeDecision::Skip {
                reason: format!(
                    "No hardware detected for {}, and CPU fallback is disabled",
                    target_codec_str
                ),
            };
        }
        if !config.hardware.allow_cpu_encoding {
            return TranscodeDecision::Skip {
                reason: "CPU encoding disabled in configuration".to_string(),
            };
        }
    }

    // 1. Codec Check (skip if already target codec + 10-bit, or H.264 preferred and already H.264)
    if metadata.codec_name.eq_ignore_ascii_case(target_codec_str) && metadata.bit_depth == Some(10)
    {
        return TranscodeDecision::Skip {
            reason: format!("Already {} 10-bit", target_codec_str),
        };
    }

    if target_codec == crate::config::OutputCodec::H264
        && metadata.codec_name.eq_ignore_ascii_case("h264")
        && metadata.bit_depth.is_some_and(|depth| depth <= 8)
    {
        return TranscodeDecision::Skip {
            reason: "Already H.264".to_string(),
        };
    }

    // 2. Efficiency Rules (BPP)
    let estimated_container_bitrate = if metadata.size_bytes > 0 && metadata.duration_secs > 0.0 {
        Some(((metadata.size_bytes as f64 * 8.0) / metadata.duration_secs) as u64)
    } else {
        None
    };
    let video_bitrate_available = metadata.video_bitrate_bps.is_some();
    let bitrate = metadata.video_bitrate_bps.or_else(|| {
        if matches!(analysis.confidence, crate::media::pipeline::AnalysisConfidence::High) {
            None
        } else {
            metadata.container_bitrate_bps.or(estimated_container_bitrate)
        }
    });
    let width = metadata.width as f64;
    let height = metadata.height as f64;
    let fps = metadata.fps;

    if width == 0.0 || height == 0.0 {
        return TranscodeDecision::Skip {
            reason: "Incomplete metadata (resolution missing)".to_string(),
        };
    }

    let bpp = if bitrate.unwrap_or(0) == 0 || fps <= 0.0 {
        None
    } else {
        Some((bitrate.unwrap_or(0) as f64) / (width * height * fps))
    };

    // Normalize BPP based on resolution
    let res_correction = if width >= 3840.0 {
        0.6 // 4K
    } else if width >= 1920.0 {
        0.8 // 1080p
    } else {
        1.0 // 720p and below
    };
    let normalized_bpp = bpp.map(|value| value * res_correction);

    // Heuristic via config (only if bitrate/fps are known)
    let mut threshold = match analysis.confidence {
        crate::media::pipeline::AnalysisConfidence::High => config.transcode.min_bpp_threshold,
        crate::media::pipeline::AnalysisConfidence::Medium => config.transcode.min_bpp_threshold * 0.7,
        crate::media::pipeline::AnalysisConfidence::Low => config.transcode.min_bpp_threshold * 0.5,
    };
    if target_codec == crate::config::OutputCodec::Av1 {
        threshold *= 0.7;
    }
    if metadata.codec_name.eq_ignore_ascii_case("h264") {
        threshold *= 0.6;
    }
    if video_bitrate_available && normalized_bpp.is_some_and(|value| value < threshold) {
        return TranscodeDecision::Skip {
            reason: format!(
                "BPP too low ({:.4} normalized < {:.2}), avoiding quality murder",
                normalized_bpp.unwrap_or_default(),
                threshold
            ),
        };
    }

    // 3. Projected Size Logic
    let size_bytes = metadata.size_bytes;
    let min_size_bytes = config.transcode.min_file_size_mb * 1024 * 1024;
    if size_bytes < min_size_bytes {
        return TranscodeDecision::Skip {
            reason: format!(
                "File too small ({}MB < {}MB) to justify transcode overhead",
                size_bytes / 1024 / 1024,
                config.transcode.min_file_size_mb
            ),
        };
    }

    // 1b. Always transcode H.264 sources, but only after guardrails pass.
    if metadata.codec_name.eq_ignore_ascii_case("h264") {
        return TranscodeDecision::Transcode {
            reason: "H.264 source prioritized for transcode".to_string(),
        };
    }

    let reason = format!(
        "Ready for {} transcode (Current codec: {}, BPP: {})",
        target_codec_str,
        metadata.codec_name,
        bpp.map(|value| format!("{:.4}", value))
            .unwrap_or_else(|| "unknown".to_string())
    );

    TranscodeDecision::Transcode { reason }
}

fn select_encoder(
    target_codec: crate::config::OutputCodec,
    hardware: &HardwareCapabilities,
    allow_fallback: bool,
) -> Option<Encoder> {
    let mut candidates: Vec<Encoder> = Vec::new();
    match target_codec {
        crate::config::OutputCodec::Av1 => {
            candidates.extend([
                Encoder::Av1Videotoolbox,
                Encoder::Av1Qsv,
                Encoder::Av1Nvenc,
                Encoder::Av1Vaapi,
                Encoder::Av1Amf,
                Encoder::Av1Svt,
                Encoder::Av1Aom,
            ]);
            if allow_fallback {
                candidates.extend([
                    Encoder::HevcVideotoolbox,
                    Encoder::HevcQsv,
                    Encoder::HevcNvenc,
                    Encoder::HevcVaapi,
                    Encoder::HevcAmf,
                    Encoder::HevcX265,
                    Encoder::H264Videotoolbox,
                    Encoder::H264Qsv,
                    Encoder::H264Nvenc,
                    Encoder::H264Vaapi,
                    Encoder::H264Amf,
                    Encoder::H264X264,
                ]);
            }
        }
        crate::config::OutputCodec::Hevc => {
            candidates.extend([
                Encoder::HevcVideotoolbox,
                Encoder::HevcQsv,
                Encoder::HevcNvenc,
                Encoder::HevcVaapi,
                Encoder::HevcAmf,
                Encoder::HevcX265,
            ]);
            if allow_fallback {
                candidates.extend([
                    Encoder::H264Videotoolbox,
                    Encoder::H264Qsv,
                    Encoder::H264Nvenc,
                    Encoder::H264Vaapi,
                    Encoder::H264Amf,
                    Encoder::H264X264,
                ]);
            }
        }
        crate::config::OutputCodec::H264 => {
            candidates.extend([
                Encoder::H264Videotoolbox,
                Encoder::H264Qsv,
                Encoder::H264Nvenc,
                Encoder::H264Vaapi,
                Encoder::H264Amf,
                Encoder::H264X264,
            ]);
            if allow_fallback {
                candidates.extend([
                    Encoder::HevcVideotoolbox,
                    Encoder::HevcQsv,
                    Encoder::HevcNvenc,
                    Encoder::HevcVaapi,
                    Encoder::HevcAmf,
                    Encoder::HevcX265,
                ]);
            }
        }
    }

    candidates
        .into_iter()
        .find(|candidate| hardware.encoders.contains(candidate))
}

fn default_rate_control(encoder: &Encoder, config: &Config) -> RateControl {
    match encoder {
        Encoder::Av1Qsv | Encoder::HevcQsv | Encoder::H264Qsv => {
            RateControl::QsvQuality {
                value: parse_quality_u8(config.transcode.quality_profile.qsv_quality(), 23),
            }
        }
        Encoder::Av1Nvenc | Encoder::HevcNvenc | Encoder::H264Nvenc => RateControl::Cq { value: 25 },
        Encoder::Av1Videotoolbox
        | Encoder::HevcVideotoolbox
        | Encoder::H264Videotoolbox => RateControl::Cq {
            value: parse_quality_u8(config.transcode.quality_profile.videotoolbox_quality(), 65),
        },
        _ => {
            let (_, crf) = config.hardware.cpu_preset.params();
            RateControl::Crf {
                value: crf.parse().unwrap_or(24),
            }
        }
    }
}

fn parse_quality_u8(value: &str, default_value: u8) -> u8 {
    value.parse().unwrap_or(default_value)
}

pub fn build_hardware_capabilities(
    caps: &crate::media::ffmpeg::EncoderCapabilities,
    hw_info: Option<&HardwareInfo>,
) -> HardwareCapabilities {
    let mut encoders = Vec::new();

    let has = |name: &str| caps.has_video_encoder(name);

    if has("av1_videotoolbox") {
        encoders.push(Encoder::Av1Videotoolbox);
    }
    if has("av1_qsv") {
        encoders.push(Encoder::Av1Qsv);
    }
    if has("av1_nvenc") {
        encoders.push(Encoder::Av1Nvenc);
    }
    if has("av1_vaapi") {
        encoders.push(Encoder::Av1Vaapi);
    }
    if has("av1_amf") {
        encoders.push(Encoder::Av1Amf);
    }
    if has("libsvtav1") {
        encoders.push(Encoder::Av1Svt);
    }
    if has("libaom-av1") {
        encoders.push(Encoder::Av1Aom);
    }
    if has("hevc_videotoolbox") {
        encoders.push(Encoder::HevcVideotoolbox);
    }
    if has("hevc_qsv") {
        encoders.push(Encoder::HevcQsv);
    }
    if has("hevc_nvenc") {
        encoders.push(Encoder::HevcNvenc);
    }
    if has("hevc_vaapi") {
        encoders.push(Encoder::HevcVaapi);
    }
    if has("hevc_amf") {
        encoders.push(Encoder::HevcAmf);
    }
    if has("libx265") {
        encoders.push(Encoder::HevcX265);
    }
    if has("h264_videotoolbox") {
        encoders.push(Encoder::H264Videotoolbox);
    }
    if has("h264_qsv") {
        encoders.push(Encoder::H264Qsv);
    }
    if has("h264_nvenc") {
        encoders.push(Encoder::H264Nvenc);
    }
    if has("h264_vaapi") {
        encoders.push(Encoder::H264Vaapi);
    }
    if has("h264_amf") {
        encoders.push(Encoder::H264Amf);
    }
    if has("libx264") {
        encoders.push(Encoder::H264X264);
    }

    let constraints = std::collections::HashMap::new();

    let _ = hw_info; // placeholder for future constraint enrichment

    HardwareCapabilities {
        encoders,
        constraints,
    }
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
