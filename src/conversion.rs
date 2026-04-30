use crate::config::{OutputCodec, TonemapAlgorithm};
use crate::error::{AlchemistError, Result};
use crate::media::ffmpeg::{FFmpegCommandBuilder, encoder_caps_clone};
use crate::media::pipeline::{
    AudioCodec, AudioStreamPlan, Encoder, EncoderBackend, FilterStep, MediaAnalysis, RateControl,
    SubtitleStreamPlan, TranscodeDecision, TranscodePlan,
};
use crate::system::hardware::HardwareInfo;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionSettings {
    pub output_container: String,
    pub remux_only: bool,
    pub video: ConversionVideoSettings,
    pub audio: ConversionAudioSettings,
    pub subtitles: ConversionSubtitleSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionVideoSettings {
    pub codec: String,
    pub mode: String,
    pub value: Option<u32>,
    pub preset: Option<String>,
    pub resolution: ConversionResolutionSettings,
    pub hdr_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResolutionSettings {
    pub mode: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub scale_factor: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionAudioSettings {
    pub codec: String,
    pub bitrate_kbps: Option<u16>,
    pub channels: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionSubtitleSettings {
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionPreview {
    pub normalized_settings: ConversionSettings,
    pub command_preview: String,
    pub summary: ConversionPreviewSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionPreviewSummary {
    pub source: ConversionSourceSummary,
    pub planned_output: ConversionPlannedOutputSummary,
    pub estimate: ConversionEstimate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionSourceSummary {
    pub file_name: String,
    pub container: String,
    pub video_codec: String,
    pub resolution: String,
    pub dynamic_range: String,
    pub duration_secs: f64,
    pub size_bytes: u64,
    pub audio: String,
    pub subtitle_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionPlannedOutputSummary {
    pub mode: String,
    pub container: String,
    pub video_codec: String,
    pub resolution: String,
    pub hdr_mode: String,
    pub audio: String,
    pub subtitles: String,
    pub encoder: Option<String>,
    pub backend: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionEstimate {
    pub estimated_output_bytes: u64,
    pub estimated_savings_bytes: u64,
    pub estimated_savings_percent: f64,
    pub confidence: String,
    pub note: String,
}

impl Default for ConversionSettings {
    fn default() -> Self {
        Self {
            output_container: "mkv".to_string(),
            remux_only: false,
            video: ConversionVideoSettings {
                codec: "hevc".to_string(),
                mode: "crf".to_string(),
                value: Some(24),
                preset: Some("medium".to_string()),
                resolution: ConversionResolutionSettings {
                    mode: "original".to_string(),
                    width: None,
                    height: None,
                    scale_factor: None,
                },
                hdr_mode: "preserve".to_string(),
            },
            audio: ConversionAudioSettings {
                codec: "copy".to_string(),
                bitrate_kbps: Some(160),
                channels: Some("auto".to_string()),
            },
            subtitles: ConversionSubtitleSettings {
                mode: "copy".to_string(),
            },
        }
    }
}

pub fn build_plan(
    analysis: &MediaAnalysis,
    output_path: &Path,
    settings: &ConversionSettings,
    hw_info: Option<HardwareInfo>,
) -> Result<TranscodePlan> {
    let normalized = normalize_settings(analysis, settings)?;
    let container = normalized.output_container.clone();

    if normalized.remux_only {
        let requested_codec = infer_source_codec(&analysis.metadata.codec_name)?;
        return Ok(TranscodePlan {
            decision: TranscodeDecision::Remux {
                reason: "conversion_remux_only".to_string(),
            },
            is_remux: true,
            copy_video: true,
            output_path: Some(output_path.to_path_buf()),
            container,
            requested_codec,
            output_codec: Some(requested_codec),
            encoder: None,
            backend: None,
            rate_control: None,
            encoder_preset: None,
            threads: 0,
            audio: AudioStreamPlan::Copy,
            audio_stream_indices: None,
            subtitles: SubtitleStreamPlan::CopyAllCompatible,
            filters: Vec::new(),
            allow_fallback: true,
            fallback: None,
        });
    }

    let requested_codec = match normalized.video.codec.as_str() {
        "copy" => infer_source_codec(&analysis.metadata.codec_name)?,
        "av1" => OutputCodec::Av1,
        "hevc" => OutputCodec::Hevc,
        "h264" => OutputCodec::H264,
        other => {
            return Err(AlchemistError::Config(format!(
                "Unsupported conversion video codec '{}'",
                other
            )));
        }
    };

    let copy_video = normalized.video.codec == "copy";
    let encoder = if copy_video {
        None
    } else {
        Some(select_encoder_for_codec(
            requested_codec,
            hw_info.as_ref(),
            &encoder_caps_clone(),
        )?)
    };

    let backend = encoder.map(|value| value.backend());
    let rate_control = if copy_video {
        None
    } else {
        let selected_encoder = encoder.ok_or_else(|| {
            AlchemistError::Config("Conversion encoder selection missing".to_string())
        })?;
        Some(build_rate_control(
            &normalized.video.mode,
            normalized.video.value,
            selected_encoder,
        )?)
    };

    let mut filters = Vec::new();
    if !copy_video {
        match normalized.video.resolution.mode.as_str() {
            "custom" => {
                let width = normalized
                    .video
                    .resolution
                    .width
                    .unwrap_or(analysis.metadata.width)
                    .max(2);
                let height = normalized
                    .video
                    .resolution
                    .height
                    .unwrap_or(analysis.metadata.height)
                    .max(2);
                filters.push(FilterStep::Scale {
                    width: even(width),
                    height: even(height),
                });
            }
            "scale_factor" => {
                let factor = normalized.video.resolution.scale_factor.unwrap_or(1.0);
                let width =
                    even(((analysis.metadata.width as f32) * factor).round().max(2.0) as u32);
                let height = even(
                    ((analysis.metadata.height as f32) * factor)
                        .round()
                        .max(2.0) as u32,
                );
                filters.push(FilterStep::Scale { width, height });
            }
            _ => {}
        }

        match normalized.video.hdr_mode.as_str() {
            "tonemap" => filters.push(FilterStep::Tonemap {
                algorithm: TonemapAlgorithm::Hable,
                peak: crate::config::default_tonemap_peak(),
                desat: crate::config::default_tonemap_desat(),
            }),
            "strip_metadata" => filters.push(FilterStep::StripHdrMetadata),
            _ => {}
        }
    }

    let subtitles = build_subtitle_plan(analysis, &normalized, copy_video)?;
    if let SubtitleStreamPlan::Burn { stream_index } = subtitles {
        filters.push(FilterStep::SubtitleBurn { stream_index });
    }

    let audio = build_audio_plan(&normalized.audio)?;

    Ok(TranscodePlan {
        decision: TranscodeDecision::Transcode {
            reason: "conversion_requested".to_string(),
        },
        is_remux: false,
        copy_video,
        output_path: Some(output_path.to_path_buf()),
        container,
        requested_codec,
        output_codec: Some(requested_codec),
        encoder,
        backend,
        rate_control,
        encoder_preset: normalized.video.preset.clone(),
        threads: 0,
        audio,
        audio_stream_indices: None,
        subtitles,
        filters,
        allow_fallback: true,
        fallback: None,
    })
}

pub fn preview_command(
    input_path: &Path,
    output_path: &Path,
    analysis: &MediaAnalysis,
    settings: &ConversionSettings,
    hw_info: Option<HardwareInfo>,
) -> Result<ConversionPreview> {
    let normalized = normalize_settings(analysis, settings)?;
    let plan = build_plan(analysis, output_path, &normalized, hw_info.clone())?;
    let args = FFmpegCommandBuilder::new(input_path, output_path, &analysis.metadata, &plan)
        .with_hardware(hw_info.as_ref())
        .build_args()?;
    let summary = build_preview_summary(analysis, &plan, &normalized.output_container);
    Ok(ConversionPreview {
        normalized_settings: normalized,
        command_preview: format!(
            "ffmpeg {}",
            args.iter()
                .map(|arg| shell_escape(arg))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        summary,
    })
}

fn build_preview_summary(
    analysis: &MediaAnalysis,
    plan: &TranscodePlan,
    requested_container: &str,
) -> ConversionPreviewSummary {
    let source = ConversionSourceSummary {
        file_name: analysis
            .metadata
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("source")
            .to_string(),
        container: analysis.metadata.container.clone(),
        video_codec: analysis.metadata.codec_name.clone(),
        resolution: format!("{}x{}", analysis.metadata.width, analysis.metadata.height),
        dynamic_range: dynamic_range_label(&analysis.metadata.dynamic_range).to_string(),
        duration_secs: analysis.metadata.duration_secs,
        size_bytes: analysis.metadata.size_bytes,
        audio: source_audio_summary(analysis),
        subtitle_count: analysis.metadata.subtitle_streams.len(),
    };

    let planned_width = plan
        .filters
        .iter()
        .find_map(|filter| match filter {
            FilterStep::Scale { width, height } => Some((*width, *height)),
            _ => None,
        })
        .unwrap_or((analysis.metadata.width, analysis.metadata.height));
    let planned_output = ConversionPlannedOutputSummary {
        mode: if plan.is_remux {
            "remux".to_string()
        } else if plan.copy_video {
            "copy".to_string()
        } else {
            "compress".to_string()
        },
        container: plan.container.clone(),
        video_codec: plan
            .output_codec
            .unwrap_or(plan.requested_codec)
            .as_str()
            .to_string(),
        resolution: format!("{}x{}", planned_width.0, planned_width.1),
        hdr_mode: planned_hdr_mode(plan),
        audio: planned_audio_summary(&plan.audio),
        subtitles: planned_subtitle_summary(&plan.subtitles),
        encoder: plan
            .encoder
            .map(|encoder| encoder.ffmpeg_encoder_name().to_string()),
        backend: plan.backend.map(|backend| backend.as_str().to_string()),
    };

    let estimate = estimate_output_size(analysis, plan, requested_container);

    ConversionPreviewSummary {
        source,
        planned_output,
        estimate,
    }
}

fn dynamic_range_label(dynamic_range: &crate::media::pipeline::DynamicRange) -> &'static str {
    match dynamic_range {
        crate::media::pipeline::DynamicRange::Sdr => "sdr",
        crate::media::pipeline::DynamicRange::Hdr10 => "hdr10",
        crate::media::pipeline::DynamicRange::Hlg => "hlg",
        crate::media::pipeline::DynamicRange::DolbyVision => "dolby_vision",
        crate::media::pipeline::DynamicRange::Unknown => "unknown",
    }
}

fn source_audio_summary(analysis: &MediaAnalysis) -> String {
    if analysis.metadata.audio_streams.is_empty() {
        return analysis
            .metadata
            .audio_codec
            .as_deref()
            .map(|codec| match analysis.metadata.audio_channels {
                Some(channels) => format!("{codec} / {channels}ch"),
                None => codec.to_string(),
            })
            .unwrap_or_else(|| "none".to_string());
    }

    let first = &analysis.metadata.audio_streams[0];
    let channels = first
        .channels
        .map(|value| format!(" / {value}ch"))
        .unwrap_or_default();
    if analysis.metadata.audio_streams.len() == 1 {
        format!("{}{}", first.codec_name, channels)
    } else {
        format!(
            "{}{} + {} more",
            first.codec_name,
            channels,
            analysis.metadata.audio_streams.len().saturating_sub(1)
        )
    }
}

fn planned_hdr_mode(plan: &TranscodePlan) -> String {
    if plan.copy_video {
        return "preserve".to_string();
    }
    if plan
        .filters
        .iter()
        .any(|filter| matches!(filter, FilterStep::Tonemap { .. }))
    {
        return "tonemap".to_string();
    }
    if plan
        .filters
        .iter()
        .any(|filter| matches!(filter, FilterStep::StripHdrMetadata))
    {
        return "strip_metadata".to_string();
    }
    "preserve".to_string()
}

fn planned_audio_summary(audio: &AudioStreamPlan) -> String {
    match audio {
        AudioStreamPlan::Copy => "copy".to_string(),
        AudioStreamPlan::Drop => "remove".to_string(),
        AudioStreamPlan::Transcode {
            codec,
            bitrate_kbps,
            channels,
        } => channels
            .map(|value| {
                format!(
                    "{} / {} kbps / {}ch",
                    codec.ffmpeg_name(),
                    bitrate_kbps,
                    value
                )
            })
            .unwrap_or_else(|| format!("{} / {} kbps", codec.ffmpeg_name(), bitrate_kbps)),
    }
}

fn planned_subtitle_summary(subtitles: &SubtitleStreamPlan) -> String {
    match subtitles {
        SubtitleStreamPlan::CopyAllCompatible => "copy compatible".to_string(),
        SubtitleStreamPlan::Drop => "remove".to_string(),
        SubtitleStreamPlan::Burn { .. } => "burn in".to_string(),
        SubtitleStreamPlan::Extract { .. } => "extract".to_string(),
    }
}

fn estimate_output_size(
    analysis: &MediaAnalysis,
    plan: &TranscodePlan,
    requested_container: &str,
) -> ConversionEstimate {
    let source_size = analysis.metadata.size_bytes;
    if source_size == 0 {
        return ConversionEstimate {
            estimated_output_bytes: 0,
            estimated_savings_bytes: 0,
            estimated_savings_percent: 0.0,
            confidence: "low".to_string(),
            note: "Source size is unavailable, so savings cannot be estimated.".to_string(),
        };
    }

    let copy_all_streams = (plan.is_remux || plan.copy_video)
        && matches!(plan.audio, AudioStreamPlan::Copy)
        && matches!(plan.subtitles, SubtitleStreamPlan::CopyAllCompatible)
        && plan.filters.is_empty();
    if copy_all_streams {
        return ConversionEstimate {
            estimated_output_bytes: source_size,
            estimated_savings_bytes: 0,
            estimated_savings_percent: 0.0,
            confidence: "high".to_string(),
            note: if requested_container == analysis.metadata.container {
                "Stream-copy output should stay close to the source size.".to_string()
            } else {
                "Remux output should stay close to the source size.".to_string()
            },
        };
    }

    let duration = analysis.metadata.duration_secs;
    if duration <= 0.0 {
        return estimate_by_ratio(
            source_size,
            codec_ratio(plan) * quality_multiplier(plan),
            "low",
            "Duration is unavailable, so the estimate uses a broad codec/quality ratio.",
        );
    }

    let source_video_bps = analysis.metadata.video_bitrate_bps.unwrap_or_else(|| {
        let total_bps = (source_size as f64 * 8.0 / duration).max(1.0) as u64;
        let audio_bps = analysis.metadata.audio_bitrate_bps.unwrap_or(0);
        total_bps.saturating_sub(audio_bps).max(total_bps / 2)
    });
    let planned_video_bps = if plan.copy_video {
        source_video_bps as f64
    } else {
        match &plan.rate_control {
            Some(RateControl::Bitrate { kbps }) => f64::from(*kbps) * 1000.0,
            _ => (source_video_bps as f64 * codec_ratio(plan) * quality_multiplier(plan)).max(1.0),
        }
    };
    let planned_audio_bps = match &plan.audio {
        AudioStreamPlan::Copy => analysis.metadata.audio_bitrate_bps.unwrap_or(0) as f64,
        AudioStreamPlan::Transcode { bitrate_kbps, .. } => f64::from(*bitrate_kbps) * 1000.0,
        AudioStreamPlan::Drop => 0.0,
    };

    let output_bits = (planned_video_bps + planned_audio_bps) * duration;
    let estimated_output_bytes = ((output_bits / 8.0) * 1.03).round().max(1.0) as u64;
    let estimated_savings_bytes = source_size.saturating_sub(estimated_output_bytes);
    let estimated_savings_percent = if source_size > 0 {
        estimated_savings_bytes as f64 / source_size as f64 * 100.0
    } else {
        0.0
    };
    let confidence = if matches!(&plan.rate_control, Some(RateControl::Bitrate { .. })) {
        "high"
    } else if analysis.metadata.video_bitrate_bps.is_some() {
        "medium"
    } else {
        "low"
    };
    let note = if matches!(&plan.rate_control, Some(RateControl::Bitrate { .. })) {
        "Estimated from the selected bitrate, duration, audio plan, and container overhead."
    } else {
        "Estimated from source bitrate, target codec, selected quality, audio plan, and container overhead."
    };

    ConversionEstimate {
        estimated_output_bytes,
        estimated_savings_bytes,
        estimated_savings_percent,
        confidence: confidence.to_string(),
        note: note.to_string(),
    }
}

fn estimate_by_ratio(
    source_size: u64,
    ratio: f64,
    confidence: &str,
    note: &str,
) -> ConversionEstimate {
    let estimated_output_bytes = (source_size as f64 * ratio.clamp(0.1, 1.2)).round() as u64;
    let estimated_savings_bytes = source_size.saturating_sub(estimated_output_bytes);
    let estimated_savings_percent = estimated_savings_bytes as f64 / source_size as f64 * 100.0;
    ConversionEstimate {
        estimated_output_bytes,
        estimated_savings_bytes,
        estimated_savings_percent,
        confidence: confidence.to_string(),
        note: note.to_string(),
    }
}

fn codec_ratio(plan: &TranscodePlan) -> f64 {
    match plan.output_codec.unwrap_or(plan.requested_codec) {
        OutputCodec::Av1 => 0.48,
        OutputCodec::Hevc => 0.62,
        OutputCodec::H264 => 0.86,
    }
}

fn quality_multiplier(plan: &TranscodePlan) -> f64 {
    let quality_value = match &plan.rate_control {
        Some(RateControl::Crf { value })
        | Some(RateControl::Cq { value })
        | Some(RateControl::QsvQuality { value }) => *value,
        _ => 24,
    };

    match quality_value {
        0..=18 => 1.2,
        19..=22 => 1.05,
        23..=26 => 0.95,
        27..=30 => 0.82,
        _ => 0.72,
    }
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=+".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn normalize_settings(
    analysis: &MediaAnalysis,
    settings: &ConversionSettings,
) -> Result<ConversionSettings> {
    let mut normalized = settings.clone();
    if normalized.output_container.trim().is_empty() {
        normalized.output_container = "mkv".to_string();
    }
    normalized.output_container = normalized.output_container.trim().to_ascii_lowercase();
    normalized.video.codec = normalized.video.codec.trim().to_ascii_lowercase();
    normalized.video.mode = normalized.video.mode.trim().to_ascii_lowercase();
    normalized.video.hdr_mode = normalized.video.hdr_mode.trim().to_ascii_lowercase();
    normalized.video.resolution.mode = normalized.video.resolution.mode.trim().to_ascii_lowercase();
    normalized.audio.codec = normalized.audio.codec.trim().to_ascii_lowercase();
    normalized.subtitles.mode = normalized.subtitles.mode.trim().to_ascii_lowercase();
    normalized.audio.channels = Some(
        normalized
            .audio
            .channels
            .as_deref()
            .unwrap_or("auto")
            .trim()
            .to_ascii_lowercase(),
    );

    if normalized.remux_only {
        normalized.video.codec = "copy".to_string();
        normalized.audio.codec = "copy".to_string();
        normalized.subtitles.mode = "copy".to_string();
        normalized.video.mode = "crf".to_string();
        normalized.video.value = None;
        normalized.video.resolution.mode = "original".to_string();
        normalized.video.hdr_mode = "preserve".to_string();
    }

    if normalized.video.codec == "copy" {
        if normalized.video.resolution.mode != "original" {
            return Err(AlchemistError::Config(
                "Video copy cannot be combined with resize controls".to_string(),
            ));
        }
        if normalized.video.hdr_mode != "preserve" {
            return Err(AlchemistError::Config(
                "Video copy cannot be combined with HDR transforms".to_string(),
            ));
        }
        if normalized.subtitles.mode == "burn" {
            return Err(AlchemistError::Config(
                "Burn-in subtitles requires video re-encoding".to_string(),
            ));
        }
    }

    if normalized.subtitles.mode == "burn"
        && !analysis
            .metadata
            .subtitle_streams
            .iter()
            .any(|stream| stream.burnable)
    {
        return Err(AlchemistError::Config(
            "No burnable subtitle stream is available for this file".to_string(),
        ));
    }

    Ok(normalized)
}

fn build_audio_plan(settings: &ConversionAudioSettings) -> Result<AudioStreamPlan> {
    match settings.codec.as_str() {
        "copy" => Ok(AudioStreamPlan::Copy),
        "aac" => Ok(AudioStreamPlan::Transcode {
            codec: AudioCodec::Aac,
            bitrate_kbps: settings.bitrate_kbps.unwrap_or(160),
            channels: parse_audio_channels(settings.channels.as_deref()),
        }),
        "opus" => Ok(AudioStreamPlan::Transcode {
            codec: AudioCodec::Opus,
            bitrate_kbps: settings.bitrate_kbps.unwrap_or(160),
            channels: parse_audio_channels(settings.channels.as_deref()),
        }),
        "mp3" => Ok(AudioStreamPlan::Transcode {
            codec: AudioCodec::Mp3,
            bitrate_kbps: settings.bitrate_kbps.unwrap_or(192),
            channels: parse_audio_channels(settings.channels.as_deref()),
        }),
        "remove" | "drop" | "none" => Ok(AudioStreamPlan::Drop),
        other => Err(AlchemistError::Config(format!(
            "Unsupported conversion audio codec '{}'",
            other
        ))),
    }
}

fn build_subtitle_plan(
    analysis: &MediaAnalysis,
    settings: &ConversionSettings,
    copy_video: bool,
) -> Result<SubtitleStreamPlan> {
    match settings.subtitles.mode.as_str() {
        "copy" => {
            if !crate::media::planner::subtitle_copy_supported(
                &settings.output_container,
                &analysis.metadata.subtitle_streams,
            ) {
                return Err(AlchemistError::Config(
                    "Subtitle copy is not supported for the selected output container with these subtitle codecs. \
                     Use 'remove' or 'burn' instead.".to_string(),
                ));
            }
            Ok(SubtitleStreamPlan::CopyAllCompatible)
        }
        "remove" | "drop" | "none" => Ok(SubtitleStreamPlan::Drop),
        "burn" => {
            if copy_video {
                return Err(AlchemistError::Config(
                    "Burn-in subtitles requires video re-encoding".to_string(),
                ));
            }
            let stream = analysis
                .metadata
                .subtitle_streams
                .iter()
                .find(|stream| stream.forced && stream.burnable)
                .or_else(|| {
                    analysis
                        .metadata
                        .subtitle_streams
                        .iter()
                        .find(|stream| stream.default && stream.burnable)
                })
                .or_else(|| {
                    analysis
                        .metadata
                        .subtitle_streams
                        .iter()
                        .find(|stream| stream.burnable)
                })
                .ok_or_else(|| {
                    AlchemistError::Config(
                        "No burnable subtitle stream is available for this file".to_string(),
                    )
                })?;
            Ok(SubtitleStreamPlan::Burn {
                stream_index: stream.stream_index,
            })
        }
        other => Err(AlchemistError::Config(format!(
            "Unsupported subtitle mode '{}'",
            other
        ))),
    }
}

fn parse_audio_channels(value: Option<&str>) -> Option<u32> {
    match value.unwrap_or("auto") {
        "auto" => None,
        "stereo" => Some(2),
        "5.1" => Some(6),
        other => other.parse::<u32>().ok(),
    }
}

fn build_rate_control(mode: &str, value: Option<u32>, encoder: Encoder) -> Result<RateControl> {
    match mode {
        "bitrate" => Ok(RateControl::Bitrate {
            kbps: value.unwrap_or(4000),
        }),
        _ => {
            let quality = value.unwrap_or(24) as u8;
            match encoder.backend() {
                EncoderBackend::Qsv => Ok(RateControl::QsvQuality { value: quality }),
                EncoderBackend::Cpu => Ok(RateControl::Crf { value: quality }),
                EncoderBackend::Videotoolbox => Ok(RateControl::Cq { value: quality }),
                _ => Ok(RateControl::Cq { value: quality }),
            }
        }
    }
}

fn select_encoder_for_codec(
    requested_codec: OutputCodec,
    hw_info: Option<&HardwareInfo>,
    encoder_caps: &crate::media::ffmpeg::EncoderCapabilities,
) -> Result<Encoder> {
    if let Some(hw) = hw_info {
        for backend in &hw.backends {
            if backend.codec != requested_codec.as_str() {
                continue;
            }
            if let Some(encoder) = encoder_from_name(&backend.encoder) {
                return Ok(encoder);
            }
        }
    }

    match requested_codec {
        OutputCodec::Av1 if encoder_caps.has_libsvtav1() => Ok(Encoder::Av1Svt),
        OutputCodec::Hevc if encoder_caps.has_libx265() => Ok(Encoder::HevcX265),
        OutputCodec::H264 if encoder_caps.has_libx264() => Ok(Encoder::H264X264),
        _ => Err(AlchemistError::Config(format!(
            "No encoder is available for requested codec '{}'",
            requested_codec.as_str()
        ))),
    }
}

fn encoder_from_name(name: &str) -> Option<Encoder> {
    match name {
        "av1_qsv" => Some(Encoder::Av1Qsv),
        "av1_nvenc" => Some(Encoder::Av1Nvenc),
        "av1_vaapi" => Some(Encoder::Av1Vaapi),
        "av1_videotoolbox" => Some(Encoder::Av1Videotoolbox),
        "av1_amf" => Some(Encoder::Av1Amf),
        "libsvtav1" => Some(Encoder::Av1Svt),
        "libaom-av1" => Some(Encoder::Av1Aom),
        "hevc_qsv" => Some(Encoder::HevcQsv),
        "hevc_nvenc" => Some(Encoder::HevcNvenc),
        "hevc_vaapi" => Some(Encoder::HevcVaapi),
        "hevc_videotoolbox" => Some(Encoder::HevcVideotoolbox),
        "hevc_amf" => Some(Encoder::HevcAmf),
        "libx265" => Some(Encoder::HevcX265),
        "h264_qsv" => Some(Encoder::H264Qsv),
        "h264_nvenc" => Some(Encoder::H264Nvenc),
        "h264_vaapi" => Some(Encoder::H264Vaapi),
        "h264_videotoolbox" => Some(Encoder::H264Videotoolbox),
        "h264_amf" => Some(Encoder::H264Amf),
        "libx264" => Some(Encoder::H264X264),
        _ => None,
    }
}

fn infer_source_codec(value: &str) -> Result<OutputCodec> {
    match value {
        "av1" => Ok(OutputCodec::Av1),
        "hevc" | "h265" => Ok(OutputCodec::Hevc),
        "h264" | "avc1" => Ok(OutputCodec::H264),
        other => Err(AlchemistError::Config(format!(
            "Source codec '{}' cannot be used with video copy mode",
            other
        ))),
    }
}

fn even(value: u32) -> u32 {
    if value % 2 == 0 {
        value
    } else {
        value.saturating_sub(1).max(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::pipeline::{
        AnalysisConfidence, AudioStreamPlan, DynamicRange, MediaAnalysis, MediaMetadata,
        RateControl, SubtitleStreamPlan, TranscodeDecision, TranscodePlan,
    };
    use std::path::PathBuf;

    fn sample_analysis() -> MediaAnalysis {
        MediaAnalysis {
            metadata: MediaMetadata {
                path: PathBuf::from("/media/Movie File.mkv"),
                duration_secs: 100.0,
                codec_name: "h264".to_string(),
                width: 1920,
                height: 1080,
                bit_depth: Some(8),
                color_primaries: None,
                color_transfer: None,
                color_space: None,
                color_range: None,
                size_bytes: 100_000_000,
                video_bitrate_bps: Some(8_000_000),
                container_bitrate_bps: Some(8_200_000),
                fps: 23.976,
                container: "mkv".to_string(),
                audio_codec: Some("aac".to_string()),
                audio_bitrate_bps: Some(192_000),
                audio_channels: Some(2),
                audio_is_heavy: false,
                subtitle_streams: Vec::new(),
                audio_streams: Vec::new(),
                dynamic_range: DynamicRange::Sdr,
            },
            warnings: Vec::new(),
            confidence: AnalysisConfidence::High,
        }
    }

    fn sample_plan() -> TranscodePlan {
        TranscodePlan {
            decision: TranscodeDecision::Transcode {
                reason: "test".to_string(),
            },
            is_remux: false,
            copy_video: false,
            output_path: Some(PathBuf::from("/tmp/output.mkv")),
            container: "mkv".to_string(),
            requested_codec: OutputCodec::Hevc,
            output_codec: Some(OutputCodec::Hevc),
            encoder: Some(Encoder::HevcX265),
            backend: Some(EncoderBackend::Cpu),
            rate_control: Some(RateControl::Crf { value: 24 }),
            encoder_preset: Some("medium".to_string()),
            threads: 0,
            audio: AudioStreamPlan::Copy,
            audio_stream_indices: None,
            subtitles: SubtitleStreamPlan::CopyAllCompatible,
            filters: Vec::new(),
            allow_fallback: true,
            fallback: None,
        }
    }

    #[test]
    fn remux_estimate_matches_source_size() {
        let analysis = sample_analysis();
        let mut plan = sample_plan();
        plan.decision = TranscodeDecision::Remux {
            reason: "test".to_string(),
        };
        plan.is_remux = true;
        plan.copy_video = true;
        plan.output_codec = Some(OutputCodec::H264);
        plan.requested_codec = OutputCodec::H264;
        plan.encoder = None;
        plan.backend = None;
        plan.rate_control = None;

        let summary = build_preview_summary(&analysis, &plan, "mkv");

        assert_eq!(summary.estimate.estimated_output_bytes, 100_000_000);
        assert_eq!(summary.estimate.estimated_savings_bytes, 0);
        assert_eq!(summary.estimate.confidence, "high");
    }

    #[test]
    fn bitrate_estimate_uses_selected_bitrate_and_audio_plan() {
        let analysis = sample_analysis();
        let mut plan = sample_plan();
        plan.rate_control = Some(RateControl::Bitrate { kbps: 2_000 });
        plan.audio = AudioStreamPlan::Transcode {
            codec: AudioCodec::Aac,
            bitrate_kbps: 128,
            channels: Some(2),
        };

        let summary = build_preview_summary(&analysis, &plan, "mkv");

        assert!(summary.estimate.estimated_output_bytes > 27_000_000);
        assert!(summary.estimate.estimated_output_bytes < 28_000_000);
        assert!(summary.estimate.estimated_savings_bytes > 70_000_000);
        assert_eq!(summary.estimate.confidence, "high");
    }

    #[test]
    fn crf_estimate_reports_medium_confidence_when_source_bitrate_exists() {
        let analysis = sample_analysis();
        let plan = sample_plan();

        let summary = build_preview_summary(&analysis, &plan, "mkv");

        assert!(summary.estimate.estimated_output_bytes > 0);
        assert!(summary.estimate.estimated_output_bytes < analysis.metadata.size_bytes);
        assert_eq!(summary.estimate.confidence, "medium");
        assert_eq!(summary.planned_output.mode, "compress");
    }

    #[test]
    fn invalid_copy_resize_settings_are_rejected() {
        let analysis = sample_analysis();
        let mut settings = ConversionSettings::default();
        settings.video.codec = "copy".to_string();
        settings.video.resolution.mode = "custom".to_string();

        let result = normalize_settings(&analysis, &settings);

        assert!(result.is_err());
    }
}
