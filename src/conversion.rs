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
                peak: 100.0,
                desat: 0.2,
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
    Ok(ConversionPreview {
        normalized_settings: normalized,
        command_preview: format!(
            "ffmpeg {}",
            args.iter()
                .map(|arg| shell_escape(arg))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    })
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
        "copy" => Ok(SubtitleStreamPlan::CopyAllCompatible),
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
