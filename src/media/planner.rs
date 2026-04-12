use crate::config::{AudioMode, Config, HdrMode, OutputCodec, QualityProfile, SubtitleMode};
use crate::error::Result;
use crate::media::pipeline::{
    AudioCodec, AudioStreamPlan, Encoder, FallbackKind, FilterStep, MediaAnalysis, PlannedFallback,
    Planner, RateControl, SidecarOutputPlan, SubtitleStreamMetadata, SubtitleStreamPlan,
    TranscodeDecision, TranscodePlan,
};
use crate::system::hardware::{HardwareBackend, HardwareInfo, Vendor};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

pub struct BasicPlanner {
    config: Arc<Config>,
    hw_info: Option<HardwareInfo>,
    encoder_caps: crate::media::ffmpeg::EncoderCapabilities,
}

impl BasicPlanner {
    pub fn new(config: Arc<Config>, hw_info: Option<HardwareInfo>) -> Self {
        Self {
            config,
            hw_info,
            encoder_caps: crate::media::ffmpeg::encoder_caps_clone(),
        }
    }
}

#[derive(Default)]
struct EncoderInventory {
    gpu: Vec<Encoder>,
    cpu: Vec<Encoder>,
}

impl EncoderInventory {
    fn is_empty(&self) -> bool {
        self.gpu.is_empty() && self.cpu.is_empty()
    }

    fn has_requested_codec_without_fallback(&self, codec: OutputCodec) -> bool {
        if !self.gpu.is_empty() {
            has_codec(&self.gpu, codec)
        } else {
            has_codec(&self.cpu, codec)
        }
    }
}

impl Planner for BasicPlanner {
    async fn plan(
        &self,
        analysis: &MediaAnalysis,
        output_path: &Path,
        profile: Option<&crate::db::LibraryProfile>,
    ) -> Result<TranscodePlan> {
        let container = normalize_container(output_path, &analysis.metadata.container);
        let requested_codec = profile
            .map(|profile| output_codec_from_profile(&profile.codec))
            .unwrap_or(self.config.transcode.output_codec);
        let quality_profile = profile
            .map(|profile| quality_profile_from_profile(&profile.quality_profile))
            .unwrap_or(self.config.transcode.quality_profile);
        let hdr_mode = profile
            .map(|profile| hdr_mode_from_profile(&profile.hdr_mode))
            .unwrap_or(self.config.transcode.hdr_mode);
        let audio_mode = profile.map(|profile| audio_mode_from_profile(&profile.audio_mode));
        let crf_override = profile.and_then(|profile| profile.crf_override);
        let decision = should_transcode(analysis, &self.config, requested_codec, &container);

        if let TranscodeDecision::Skip { reason } = &decision {
            return Ok(skip_plan(
                reason.clone(),
                container,
                requested_codec,
                self.config.transcode.allow_fallback,
                self.config.transcode.threads,
            ));
        }

        if let TranscodeDecision::Remux { reason } = &decision {
            return Ok(TranscodePlan {
                decision: TranscodeDecision::Remux {
                    reason: reason.clone(),
                },
                is_remux: true,
                copy_video: true,
                output_path: None,
                container,
                requested_codec,
                output_codec: Some(requested_codec),
                encoder: None,
                backend: None,
                rate_control: None,
                encoder_preset: None,
                threads: self.config.transcode.threads,
                audio: AudioStreamPlan::Copy,
                audio_stream_indices: None,
                subtitles: SubtitleStreamPlan::CopyAllCompatible,
                filters: Vec::new(),
                allow_fallback: self.config.transcode.allow_fallback,
                fallback: None,
            });
        }

        let available_encoders =
            build_available_encoders(&self.config, self.hw_info.as_ref(), &self.encoder_caps);

        if available_encoders.is_empty() {
            return Ok(skip_plan(
                format!(
                    "no_available_encoders|requested_codec={},allow_cpu_fallback={},allow_cpu_encoding={}",
                    requested_codec.as_str(),
                    self.config.hardware.allow_cpu_fallback,
                    self.config.hardware.allow_cpu_encoding
                ),
                container,
                requested_codec,
                self.config.transcode.allow_fallback,
                self.config.transcode.threads,
            ));
        }

        if !self.config.transcode.allow_fallback
            && !available_encoders.has_requested_codec_without_fallback(requested_codec)
        {
            return Ok(skip_plan(
                format!(
                    "preferred_codec_unavailable_fallback_disabled|codec={}",
                    requested_codec.as_str()
                ),
                container,
                requested_codec,
                self.config.transcode.allow_fallback,
                self.config.transcode.threads,
            ));
        }

        let Some((encoder, fallback)) = select_encoder(
            requested_codec,
            &available_encoders,
            self.config.transcode.allow_fallback,
        ) else {
            return Ok(skip_plan(
                format!(
                    "no_suitable_encoder|requested_codec={}",
                    requested_codec.as_str()
                ),
                container,
                requested_codec,
                self.config.transcode.allow_fallback,
                self.config.transcode.threads,
            ));
        };

        let subtitles = match plan_subtitles(
            &analysis.metadata.subtitle_streams,
            &container,
            output_path,
            self.config.transcode.subtitle_mode,
        ) {
            Ok(plan) => plan,
            Err(reason) => {
                return Ok(skip_plan(
                    reason,
                    container,
                    requested_codec,
                    self.config.transcode.allow_fallback,
                    self.config.transcode.threads,
                ));
            }
        };

        let audio = plan_audio(
            analysis.metadata.audio_codec.as_deref(),
            analysis.metadata.audio_channels,
            analysis.metadata.audio_is_heavy,
            &container,
            audio_mode,
            &self.encoder_caps,
        );
        let audio_stream_indices = filter_audio_streams(
            &analysis.metadata.audio_streams,
            &self.config.transcode.stream_rules,
        );
        let filters = plan_filters(analysis, encoder, &self.config, &subtitles, hdr_mode);
        let (rate_control, encoder_preset) =
            encoder_runtime_settings(encoder, &self.config, quality_profile, crf_override);

        Ok(TranscodePlan {
            decision,
            is_remux: false,
            copy_video: false,
            output_path: None,
            container,
            requested_codec,
            output_codec: Some(encoder.output_codec()),
            encoder: Some(encoder),
            backend: Some(encoder.backend()),
            rate_control: Some(rate_control),
            encoder_preset,
            threads: self.config.transcode.threads,
            audio,
            audio_stream_indices,
            subtitles,
            filters,
            allow_fallback: self.config.transcode.allow_fallback,
            fallback,
        })
    }
}

fn skip_plan(
    reason: String,
    container: String,
    requested_codec: OutputCodec,
    allow_fallback: bool,
    threads: usize,
) -> TranscodePlan {
    TranscodePlan {
        decision: TranscodeDecision::Skip { reason },
        is_remux: false,
        copy_video: false,
        output_path: None,
        container,
        requested_codec,
        output_codec: None,
        encoder: None,
        backend: None,
        rate_control: None,
        encoder_preset: None,
        threads,
        audio: AudioStreamPlan::Copy,
        audio_stream_indices: None,
        subtitles: SubtitleStreamPlan::CopyAllCompatible,
        filters: Vec::new(),
        allow_fallback,
        fallback: None,
    }
}

fn normalize_container(output_path: &Path, input_container: &str) -> String {
    output_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            input_container
                .split(',')
                .next()
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "mkv".to_string())
}

fn should_transcode(
    analysis: &MediaAnalysis,
    config: &Config,
    target_codec: OutputCodec,
    target_container: &str,
) -> TranscodeDecision {
    let metadata = &analysis.metadata;
    let target_codec_str = target_codec.as_str();
    let input_container = primary_container(&metadata.container);

    let already_target_codec_reason = if metadata.codec_name.eq_ignore_ascii_case(target_codec_str)
        && metadata.bit_depth == Some(10)
    {
        Some(format!(
            "already_target_codec|codec={target_codec_str},bit_depth=10"
        ))
    } else if target_codec == OutputCodec::H264
        && metadata.codec_name.eq_ignore_ascii_case("h264")
        && metadata.bit_depth.is_some_and(|depth| depth <= 8)
    {
        Some(format!(
            "already_target_codec|codec=h264,bit_depth={}",
            metadata.bit_depth.unwrap_or(8)
        ))
    } else {
        None
    };

    if let Some(skip_reason) = already_target_codec_reason {
        if container_requires_remux(&input_container, target_container) {
            return TranscodeDecision::Remux {
                reason: format!(
                    "already_target_codec_wrong_container|container={},target_extension={}",
                    input_container, target_container
                ),
            };
        }

        return TranscodeDecision::Skip {
            reason: skip_reason,
        };
    }

    let estimated_container_bitrate = if metadata.size_bytes > 0 && metadata.duration_secs > 0.0 {
        Some(((metadata.size_bytes as f64 * 8.0) / metadata.duration_secs) as u64)
    } else {
        None
    };
    let bitrate = metadata.video_bitrate_bps.or_else(|| {
        if matches!(
            analysis.confidence,
            crate::media::pipeline::AnalysisConfidence::High
        ) {
            None
        } else {
            metadata
                .container_bitrate_bps
                .or(estimated_container_bitrate)
        }
    });
    let width = metadata.width as f64;
    let height = metadata.height as f64;
    let fps = metadata.fps;

    if width == 0.0 || height == 0.0 {
        return TranscodeDecision::Skip {
            reason: "incomplete_metadata|missing=resolution".to_string(),
        };
    }

    let bpp = if bitrate.unwrap_or(0) == 0 || fps <= 0.0 {
        None
    } else {
        Some((bitrate.unwrap_or(0) as f64) / (width * height * fps))
    };

    let res_correction = if width >= 3840.0 {
        0.6
    } else if width >= 1920.0 {
        0.8
    } else {
        1.0
    };
    let normalized_bpp = bpp.map(|value| value * res_correction);

    // Raise threshold for uncertain analysis: low confidence = fewer speculative encodes.
    let mut threshold = match analysis.confidence {
        crate::media::pipeline::AnalysisConfidence::High => config.transcode.min_bpp_threshold,
        crate::media::pipeline::AnalysisConfidence::Medium => {
            config.transcode.min_bpp_threshold * 1.3
        }
        crate::media::pipeline::AnalysisConfidence::Low => config.transcode.min_bpp_threshold * 1.8,
    };
    if target_codec == OutputCodec::Av1 {
        threshold *= 0.7;
    }
    if metadata.codec_name.eq_ignore_ascii_case("h264") {
        threshold *= 0.6;
    }
    if normalized_bpp.is_some_and(|value| value < threshold) {
        return TranscodeDecision::Skip {
            reason: format!(
                "bpp_below_threshold|bpp={:.3},threshold={:.3}",
                normalized_bpp.unwrap_or_default(),
                threshold
            ),
        };
    }

    let min_size_bytes = config.transcode.min_file_size_mb * 1024 * 1024;
    if metadata.size_bytes < min_size_bytes {
        return TranscodeDecision::Skip {
            reason: format!(
                "below_min_file_size|size_mb={},threshold_mb={}",
                metadata.size_bytes / 1024 / 1024,
                config.transcode.min_file_size_mb
            ),
        };
    }

    if metadata.codec_name.eq_ignore_ascii_case("h264") {
        return TranscodeDecision::Transcode {
            reason: "transcode_h264_source|current_codec=h264".to_string(),
        };
    }

    TranscodeDecision::Transcode {
        reason: format!(
            "transcode_recommended|target_codec={},current_codec={},bpp={}",
            target_codec_str,
            metadata.codec_name,
            bpp.map(|value| format!("{value:.4}"))
                .unwrap_or_else(|| "unknown".to_string())
        ),
    }
}

fn build_available_encoders(
    config: &Config,
    hw_info: Option<&HardwareInfo>,
    caps: &crate::media::ffmpeg::EncoderCapabilities,
) -> EncoderInventory {
    let mut available = EncoderInventory::default();
    let mut seen = HashSet::new();

    if let Some(hw) = hw_info {
        for backend in &hw.backends {
            if let Some(encoder) = encoder_for_backend(backend.kind, backend.codec.as_str()) {
                if seen.insert(encoder) {
                    available.gpu.push(encoder);
                }
            }
        }
    }

    let allow_cpu = config.hardware.allow_cpu_encoding
        && match hw_info {
            Some(hw) => hw.vendor == Vendor::Cpu || config.hardware.allow_cpu_fallback,
            None => config.hardware.allow_cpu_fallback,
        };

    if allow_cpu {
        for encoder in [
            (Encoder::Av1Svt, caps.has_libsvtav1()),
            (Encoder::Av1Aom, caps.has_video_encoder("libaom-av1")),
            (Encoder::HevcX265, caps.has_libx265()),
            (Encoder::H264X264, caps.has_libx264()),
        ] {
            if encoder.1 && seen.insert(encoder.0) {
                available.cpu.push(encoder.0);
            }
        }
    }

    available
}

fn encoder_for_backend(kind: HardwareBackend, codec: &str) -> Option<Encoder> {
    match (kind, codec.to_ascii_lowercase().as_str()) {
        (HardwareBackend::Qsv, "av1") => Some(Encoder::Av1Qsv),
        (HardwareBackend::Qsv, "hevc") => Some(Encoder::HevcQsv),
        (HardwareBackend::Qsv, "h264") => Some(Encoder::H264Qsv),
        (HardwareBackend::Nvenc, "av1") => Some(Encoder::Av1Nvenc),
        (HardwareBackend::Nvenc, "hevc") => Some(Encoder::HevcNvenc),
        (HardwareBackend::Nvenc, "h264") => Some(Encoder::H264Nvenc),
        (HardwareBackend::Vaapi, "av1") => Some(Encoder::Av1Vaapi),
        (HardwareBackend::Vaapi, "hevc") => Some(Encoder::HevcVaapi),
        (HardwareBackend::Vaapi, "h264") => Some(Encoder::H264Vaapi),
        (HardwareBackend::Amf, "av1") => Some(Encoder::Av1Amf),
        (HardwareBackend::Amf, "hevc") => Some(Encoder::HevcAmf),
        (HardwareBackend::Amf, "h264") => Some(Encoder::H264Amf),
        (HardwareBackend::Videotoolbox, "av1") => Some(Encoder::Av1Videotoolbox),
        (HardwareBackend::Videotoolbox, "hevc") => Some(Encoder::HevcVideotoolbox),
        (HardwareBackend::Videotoolbox, "h264") => Some(Encoder::H264Videotoolbox),
        _ => None,
    }
}

fn has_codec(encoders: &[Encoder], codec: OutputCodec) -> bool {
    encoders
        .iter()
        .any(|encoder| encoder.output_codec() == codec)
}

fn select_encoder(
    target_codec: OutputCodec,
    available_encoders: &EncoderInventory,
    allow_fallback: bool,
) -> Option<(Encoder, Option<PlannedFallback>)> {
    if let Some(encoder) = first_available(
        &available_encoders.gpu,
        requested_gpu_candidates(target_codec),
    ) {
        return Some((encoder, None));
    }

    if !available_encoders.gpu.is_empty() {
        if allow_fallback {
            if let Some(encoder) = first_available(
                &available_encoders.gpu,
                fallback_gpu_candidates(target_codec),
            ) {
                return Some((encoder, Some(codec_fallback(target_codec, encoder))));
            }

            if let Some(encoder) = first_available(
                &available_encoders.cpu,
                requested_cpu_candidates(target_codec),
            ) {
                return Some((encoder, Some(cpu_fallback(target_codec, encoder))));
            }

            if let Some(encoder) = first_available(
                &available_encoders.cpu,
                fallback_cpu_candidates(target_codec),
            ) {
                return Some((encoder, Some(cpu_fallback(target_codec, encoder))));
            }
        }

        return None;
    }

    if let Some(encoder) = first_available(
        &available_encoders.cpu,
        requested_cpu_candidates(target_codec),
    ) {
        return Some((encoder, None));
    }

    if allow_fallback {
        if let Some(encoder) = first_available(
            &available_encoders.cpu,
            fallback_cpu_candidates(target_codec),
        ) {
            return Some((encoder, Some(codec_fallback(target_codec, encoder))));
        }
    }

    None
}

fn first_available(available_encoders: &[Encoder], candidates: &[Encoder]) -> Option<Encoder> {
    available_encoders
        .iter()
        .copied()
        .find(|candidate| candidates.contains(candidate))
}

fn requested_gpu_candidates(target_codec: OutputCodec) -> &'static [Encoder] {
    match target_codec {
        OutputCodec::Av1 => &[
            Encoder::Av1Videotoolbox,
            Encoder::Av1Qsv,
            Encoder::Av1Nvenc,
            Encoder::Av1Vaapi,
            Encoder::Av1Amf,
        ],
        OutputCodec::Hevc => &[
            Encoder::HevcVideotoolbox,
            Encoder::HevcQsv,
            Encoder::HevcNvenc,
            Encoder::HevcVaapi,
            Encoder::HevcAmf,
        ],
        OutputCodec::H264 => &[
            Encoder::H264Videotoolbox,
            Encoder::H264Qsv,
            Encoder::H264Nvenc,
            Encoder::H264Vaapi,
            Encoder::H264Amf,
        ],
    }
}

fn fallback_gpu_candidates(target_codec: OutputCodec) -> &'static [Encoder] {
    match target_codec {
        OutputCodec::Av1 => &[
            Encoder::HevcVideotoolbox,
            Encoder::HevcQsv,
            Encoder::HevcNvenc,
            Encoder::HevcVaapi,
            Encoder::HevcAmf,
            Encoder::H264Videotoolbox,
            Encoder::H264Qsv,
            Encoder::H264Nvenc,
            Encoder::H264Vaapi,
            Encoder::H264Amf,
        ],
        OutputCodec::Hevc => &[
            Encoder::H264Videotoolbox,
            Encoder::H264Qsv,
            Encoder::H264Nvenc,
            Encoder::H264Vaapi,
            Encoder::H264Amf,
        ],
        OutputCodec::H264 => &[],
    }
}

fn requested_cpu_candidates(target_codec: OutputCodec) -> &'static [Encoder] {
    match target_codec {
        OutputCodec::Av1 => &[Encoder::Av1Svt, Encoder::Av1Aom],
        OutputCodec::Hevc => &[Encoder::HevcX265],
        OutputCodec::H264 => &[Encoder::H264X264],
    }
}

fn fallback_cpu_candidates(target_codec: OutputCodec) -> &'static [Encoder] {
    match target_codec {
        OutputCodec::Av1 => &[Encoder::HevcX265, Encoder::H264X264],
        OutputCodec::Hevc => &[Encoder::H264X264],
        OutputCodec::H264 => &[],
    }
}

fn codec_fallback(requested_codec: OutputCodec, encoder: Encoder) -> PlannedFallback {
    PlannedFallback {
        kind: FallbackKind::Codec,
        reason: format!(
            "Requested {} but planned {} via {}",
            requested_codec.as_str(),
            encoder.output_codec().as_str(),
            encoder.ffmpeg_encoder_name()
        ),
    }
}

fn cpu_fallback(requested_codec: OutputCodec, encoder: Encoder) -> PlannedFallback {
    PlannedFallback {
        kind: FallbackKind::Cpu,
        reason: format!(
            "Requested {} but planned software {} via {}",
            requested_codec.as_str(),
            encoder.output_codec().as_str(),
            encoder.ffmpeg_encoder_name()
        ),
    }
}

fn encoder_runtime_settings(
    encoder: Encoder,
    config: &Config,
    quality_profile: QualityProfile,
    crf_override: Option<i32>,
) -> (RateControl, Option<String>) {
    let (rate_control, encoder_preset) = match encoder {
        Encoder::Av1Qsv | Encoder::HevcQsv | Encoder::H264Qsv => (
            RateControl::QsvQuality {
                value: parse_quality_u8(quality_profile.qsv_quality(), 23),
            },
            None,
        ),
        Encoder::Av1Nvenc => (
            RateControl::Cq { value: 28 },
            Some(quality_profile.nvenc_preset().to_string()),
        ),
        Encoder::HevcNvenc => (
            RateControl::Cq { value: 24 },
            Some(quality_profile.nvenc_preset().to_string()),
        ),
        Encoder::H264Nvenc => (
            RateControl::Cq { value: 21 },
            Some(quality_profile.nvenc_preset().to_string()),
        ),
        Encoder::Av1Videotoolbox | Encoder::HevcVideotoolbox | Encoder::H264Videotoolbox => (
            RateControl::Cq {
                value: parse_quality_u8(quality_profile.videotoolbox_quality(), 65),
            },
            None,
        ),
        Encoder::Av1Svt => {
            let (preset, crf) = config.hardware.cpu_preset.params();
            (
                RateControl::Crf {
                    value: crf.parse().unwrap_or(28),
                },
                Some(preset.to_string()),
            )
        }
        Encoder::Av1Aom => {
            let (cpu_used, default_crf) = match config.hardware.cpu_preset {
                crate::config::CpuPreset::Slow => ("2", 24u8),
                crate::config::CpuPreset::Medium => ("4", 28u8),
                crate::config::CpuPreset::Fast => ("6", 30u8),
                crate::config::CpuPreset::Faster => ("8", 32u8),
            };
            (
                RateControl::Crf { value: default_crf },
                Some(cpu_used.to_string()),
            )
        }
        Encoder::HevcX265 => {
            let preset = config.hardware.cpu_preset.as_str().to_string();
            let default_crf = match config.hardware.cpu_preset {
                crate::config::CpuPreset::Slow => 20,
                crate::config::CpuPreset::Medium => 24,
                crate::config::CpuPreset::Fast => 26,
                crate::config::CpuPreset::Faster => 28,
            };
            (RateControl::Crf { value: default_crf }, Some(preset))
        }
        Encoder::H264X264 => {
            let preset = config.hardware.cpu_preset.as_str().to_string();
            let default_crf = match config.hardware.cpu_preset {
                crate::config::CpuPreset::Slow => 18,
                crate::config::CpuPreset::Medium => 21,
                crate::config::CpuPreset::Fast => 23,
                crate::config::CpuPreset::Faster => 25,
            };
            (RateControl::Crf { value: default_crf }, Some(preset))
        }
        Encoder::Av1Vaapi | Encoder::HevcVaapi | Encoder::H264Vaapi => {
            (RateControl::Cq { value: 26 }, None)
        }
        Encoder::Av1Amf | Encoder::HevcAmf | Encoder::H264Amf => {
            (RateControl::Cq { value: 24 }, None)
        }
    };

    (
        apply_crf_override(rate_control, crf_override),
        encoder_preset,
    )
}

fn plan_audio(
    audio_codec: Option<&str>,
    audio_channels: Option<u32>,
    audio_is_heavy: bool,
    container: &str,
    audio_mode: Option<AudioMode>,
    encoder_caps: &crate::media::ffmpeg::EncoderCapabilities,
) -> AudioStreamPlan {
    if let Some(audio_mode) = audio_mode {
        return match audio_mode {
            AudioMode::Copy => {
                let Some(audio_codec) = audio_codec else {
                    return AudioStreamPlan::Copy;
                };
                if audio_copy_supported(container, audio_codec) {
                    AudioStreamPlan::Copy
                } else {
                    AudioStreamPlan::Transcode {
                        codec: AudioCodec::Aac,
                        bitrate_kbps: audio_bitrate_kbps(AudioCodec::Aac, audio_channels),
                        channels: None,
                    }
                }
            }
            AudioMode::Aac => AudioStreamPlan::Transcode {
                codec: AudioCodec::Aac,
                bitrate_kbps: audio_bitrate_kbps(AudioCodec::Aac, audio_channels),
                channels: None,
            },
            AudioMode::AacStereo => AudioStreamPlan::Transcode {
                codec: AudioCodec::Aac,
                bitrate_kbps: audio_bitrate_kbps(AudioCodec::Aac, Some(2)),
                channels: Some(2),
            },
        };
    }

    let Some(audio_codec) = audio_codec else {
        return AudioStreamPlan::Copy;
    };

    let compatible = audio_copy_supported(container, audio_codec);
    if !compatible || audio_is_heavy {
        // Use Opus for MKV if libopus is available,
        // otherwise fall back to AAC which is always
        // present. MP4 always uses AAC.
        let codec = if container != "mp4" && encoder_caps.has_libopus() {
            AudioCodec::Opus
        } else {
            AudioCodec::Aac
        };
        return AudioStreamPlan::Transcode {
            codec,
            bitrate_kbps: audio_bitrate_kbps(codec, audio_channels),
            channels: None,
        };
    }

    AudioStreamPlan::Copy
}

/// Apply stream rules to determine which audio stream indices to keep.
/// Returns None if all streams should be kept (no filtering needed), or
/// Some(Vec<usize>) with the stream indices to keep.
fn filter_audio_streams(
    streams: &[crate::media::pipeline::AudioStreamMetadata],
    rules: &crate::config::StreamRules,
) -> Option<Vec<usize>> {
    if streams.is_empty() {
        return None;
    }

    if rules.strip_audio_by_title.is_empty()
        && rules.keep_audio_languages.is_empty()
        && !rules.keep_only_default_audio
    {
        return None;
    }

    let mut kept: Vec<usize> = streams
        .iter()
        .filter(|stream| {
            if !rules.strip_audio_by_title.is_empty() {
                if let Some(title) = &stream.title {
                    let title_lower = title.to_lowercase();
                    if rules
                        .strip_audio_by_title
                        .iter()
                        .any(|keyword| title_lower.contains(&keyword.to_lowercase()))
                    {
                        return false;
                    }
                }
            }

            if !rules.keep_audio_languages.is_empty() {
                if let Some(language) = &stream.language {
                    if !rules
                        .keep_audio_languages
                        .iter()
                        .any(|allowed| allowed.eq_ignore_ascii_case(language))
                    {
                        return false;
                    }
                }
            }

            if rules.keep_only_default_audio
                && rules.keep_audio_languages.is_empty()
                && !stream.default
            {
                return false;
            }

            true
        })
        .map(|stream| stream.stream_index)
        .collect();

    if kept.is_empty() {
        let fallback = streams
            .iter()
            .find(|stream| stream.default)
            .or_else(|| streams.first());
        if let Some(fallback) = fallback {
            kept.push(fallback.stream_index);
        }
    }

    if kept.len() == streams.len() {
        return None;
    }

    Some(kept)
}

fn audio_copy_supported(container: &str, codec: &str) -> bool {
    let codec = codec.to_ascii_lowercase();
    match container {
        "mp4" | "m4v" | "mov" => matches!(
            codec.as_str(),
            "aac" | "alac" | "ac3" | "eac3" | "mp3" | "mp4a"
        ),
        _ => true,
    }
}

fn audio_bitrate_kbps(codec: AudioCodec, channels: Option<u32>) -> u16 {
    let channels = channels.unwrap_or(2);
    match codec {
        AudioCodec::Aac => {
            if channels <= 2 {
                192
            } else if channels <= 6 {
                384
            } else {
                512
            }
        }
        AudioCodec::Opus => {
            if channels <= 2 {
                160
            } else if channels <= 6 {
                256
            } else {
                320
            }
        }
        AudioCodec::Mp3 => {
            if channels <= 2 {
                192
            } else if channels <= 6 {
                320
            } else {
                384
            }
        }
    }
}

fn plan_subtitles(
    subtitle_streams: &[SubtitleStreamMetadata],
    container: &str,
    output_path: &Path,
    mode: SubtitleMode,
) -> std::result::Result<SubtitleStreamPlan, String> {
    match mode {
        SubtitleMode::Copy => {
            if !subtitle_copy_supported(container, subtitle_streams) {
                return Err(format!(
                    "Container {} cannot safely copy subtitle codecs {:?}",
                    container,
                    subtitle_streams
                        .iter()
                        .map(|stream| stream.codec_name.clone())
                        .collect::<Vec<_>>()
                ));
            }
            Ok(SubtitleStreamPlan::CopyAllCompatible)
        }
        SubtitleMode::None => Ok(SubtitleStreamPlan::Drop),
        SubtitleMode::Burn => select_burn_subtitle_stream(subtitle_streams)
            .map(|stream| SubtitleStreamPlan::Burn {
                stream_index: stream.stream_index,
            })
            .ok_or_else(|| "No burnable text subtitle stream available".to_string()),
        SubtitleMode::Extract => {
            if subtitle_streams.is_empty() {
                Ok(SubtitleStreamPlan::Drop)
            } else {
                let outputs = sidecar_outputs_for(output_path, subtitle_streams);
                if outputs.is_empty() {
                    Ok(SubtitleStreamPlan::Drop)
                } else {
                    Ok(SubtitleStreamPlan::Extract { outputs })
                }
            }
        }
    }
}

pub(crate) fn subtitle_copy_supported(
    container: &str,
    subtitle_streams: &[SubtitleStreamMetadata],
) -> bool {
    if subtitle_streams.is_empty() {
        return true;
    }

    match container {
        "mp4" | "m4v" | "mov" => subtitle_streams.iter().all(|stream| {
            matches!(
                stream.codec_name.to_ascii_lowercase().as_str(),
                "mov_text" | "tx3g"
            )
        }),
        _ => true,
    }
}

fn select_burn_subtitle_stream(
    subtitle_streams: &[SubtitleStreamMetadata],
) -> Option<&SubtitleStreamMetadata> {
    subtitle_streams
        .iter()
        .find(|stream| stream.burnable && stream.forced)
        .or_else(|| {
            subtitle_streams
                .iter()
                .find(|stream| stream.burnable && stream.default)
        })
        .or_else(|| subtitle_streams.iter().find(|stream| stream.burnable))
}

fn sidecar_outputs_for(
    output_path: &Path,
    subtitle_streams: &[SubtitleStreamMetadata],
) -> Vec<SidecarOutputPlan> {
    let parent = output_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let mut language_counts = HashMap::new();
    let mut outputs = Vec::new();

    for stream in subtitle_streams {
        let codec_name = stream.codec_name.to_ascii_lowercase();
        let Some((codec, extension)) = subtitle_sidecar_codec_and_extension(&codec_name) else {
            if matches!(
                codec_name.as_str(),
                "dvd_subtitle" | "dvdsub" | "hdmv_pgs_subtitle"
            ) {
                tracing::warn!(
                    "Skipping subtitle stream {} ({}): bitmap subtitles cannot be extracted to text sidecars",
                    stream.stream_index,
                    stream.codec_name
                );
            } else {
                tracing::warn!(
                    "Skipping subtitle stream {} ({}): unsupported subtitle codec for sidecar extraction",
                    stream.stream_index,
                    stream.codec_name
                );
            }
            continue;
        };

        let language = normalized_subtitle_language(stream.language.as_deref());
        let ordinal = language_counts
            .entry(language.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        let filename = if *ordinal == 1 {
            format!("{stem}.{language}.{extension}")
        } else {
            format!("{stem}.{language}.{}.{extension}", *ordinal)
        };
        let final_path = parent.join(&filename);
        let temp_path = parent.join(format!("{filename}.alchemist-part"));

        outputs.push(SidecarOutputPlan {
            stream_index: stream.stream_index,
            codec: codec.to_string(),
            final_path,
            temp_path,
        });
    }

    outputs
}

fn subtitle_sidecar_codec_and_extension(codec_name: &str) -> Option<(&'static str, &'static str)> {
    match codec_name {
        "subrip" | "srt" => Some(("srt", "srt")),
        "ass" | "ssa" => Some(("ass", "ass")),
        "webvtt" => Some(("webvtt", "vtt")),
        _ => None,
    }
}

fn normalized_subtitle_language(language: Option<&str>) -> String {
    let Some(language) = language.map(str::trim).filter(|value| !value.is_empty()) else {
        return "und".to_string();
    };

    let normalized = language
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else if matches!(ch, '-' | '_') {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .trim_matches('-')
        .to_string();

    if normalized.is_empty() {
        "und".to_string()
    } else {
        normalized
    }
}

fn primary_container(container: &str) -> String {
    container
        .split(',')
        .next()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "mkv".to_string())
}

fn is_mp4_family_container(container: &str) -> bool {
    matches!(container, "mp4" | "m4v" | "mov")
}

fn container_requires_remux(input_container: &str, target_container: &str) -> bool {
    is_mp4_family_container(input_container) && !is_mp4_family_container(target_container)
}

fn plan_filters(
    analysis: &MediaAnalysis,
    encoder: Encoder,
    config: &Config,
    subtitles: &SubtitleStreamPlan,
    hdr_mode: HdrMode,
) -> Vec<FilterStep> {
    let mut filters = Vec::new();

    if analysis.metadata.dynamic_range.is_hdr() && hdr_mode == crate::config::HdrMode::Tonemap {
        filters.push(FilterStep::Tonemap {
            algorithm: config.transcode.tonemap_algorithm,
            peak: config.transcode.tonemap_peak,
            desat: config.transcode.tonemap_desat,
        });
    }

    if let SubtitleStreamPlan::Burn { stream_index } = subtitles {
        filters.push(FilterStep::SubtitleBurn {
            stream_index: *stream_index,
        });
    }

    if encoder.backend() == crate::media::pipeline::EncoderBackend::Vaapi {
        filters.push(FilterStep::Format {
            pixel_format: "nv12".to_string(),
        });
        filters.push(FilterStep::HwUpload);
    }

    if encoder.backend() == crate::media::pipeline::EncoderBackend::Videotoolbox {
        filters.push(FilterStep::Format {
            pixel_format: "yuv420p".to_string(),
        });
    }

    filters
}

fn parse_quality_u8(value: &str, default_value: u8) -> u8 {
    value.parse().unwrap_or(default_value)
}

fn apply_crf_override(rate_control: RateControl, crf_override: Option<i32>) -> RateControl {
    let Some(crf_override) = crf_override else {
        return rate_control;
    };
    let value = crf_override.clamp(0, u8::MAX as i32) as u8;
    match rate_control {
        RateControl::Crf { .. } => RateControl::Crf { value },
        RateControl::Cq { .. } => RateControl::Cq { value },
        RateControl::QsvQuality { .. } => RateControl::QsvQuality { value },
        RateControl::Bitrate { kbps } => RateControl::Bitrate { kbps },
    }
}

fn output_codec_from_profile(value: &str) -> OutputCodec {
    match value.trim().to_ascii_lowercase().as_str() {
        "hevc" | "h265" => OutputCodec::Hevc,
        "h264" | "avc" => OutputCodec::H264,
        _ => OutputCodec::Av1,
    }
}

fn quality_profile_from_profile(value: &str) -> QualityProfile {
    match value.trim().to_ascii_lowercase().as_str() {
        "quality" => QualityProfile::Quality,
        "speed" => QualityProfile::Speed,
        _ => QualityProfile::Balanced,
    }
}

fn hdr_mode_from_profile(value: &str) -> HdrMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "tonemap" => HdrMode::Tonemap,
        _ => HdrMode::Preserve,
    }
}

fn audio_mode_from_profile(value: &str) -> AudioMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "aac" => AudioMode::Aac,
        "aac_stereo" => AudioMode::AacStereo,
        _ => AudioMode::Copy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HdrMode, QualityProfile, TonemapAlgorithm, TranscodeConfig};
    use crate::media::pipeline::{
        AnalysisConfidence, AudioStreamMetadata, DynamicRange, MediaMetadata,
        SubtitleStreamMetadata,
    };
    use crate::system::hardware::{BackendCapability, ProbeSummary};

    fn config() -> Config {
        let mut config = Config::default();
        config.transcode = TranscodeConfig {
            quality_profile: QualityProfile::Balanced,
            hdr_mode: HdrMode::Preserve,
            tonemap_algorithm: TonemapAlgorithm::Hable,
            ..config.transcode
        };
        config
    }

    fn analysis() -> MediaAnalysis {
        MediaAnalysis {
            metadata: MediaMetadata {
                path: "/tmp/in.mkv".into(),
                duration_secs: 120.0,
                codec_name: "hevc".to_string(),
                width: 1920,
                height: 1080,
                bit_depth: Some(10),
                color_primaries: None,
                color_transfer: None,
                color_space: None,
                color_range: None,
                size_bytes: 500 * 1024 * 1024,
                video_bitrate_bps: Some(8_000_000),
                container_bitrate_bps: Some(8_500_000),
                fps: 24.0,
                container: "matroska".to_string(),
                audio_codec: Some("flac".to_string()),
                audio_bitrate_bps: Some(1_500_000),
                audio_channels: Some(6),
                audio_is_heavy: true,
                subtitle_streams: vec![SubtitleStreamMetadata {
                    stream_index: 0,
                    codec_name: "hdmv_pgs_subtitle".to_string(),
                    language: Some("eng".to_string()),
                    title: None,
                    default: true,
                    forced: false,
                    burnable: false,
                }],
                audio_streams: vec![],
                dynamic_range: DynamicRange::Sdr,
            },
            warnings: Vec::new(),
            confidence: AnalysisConfidence::High,
        }
    }

    #[test]
    fn mp4_subtitle_copy_fails_fast() {
        let reason = match plan_subtitles(
            &[SubtitleStreamMetadata {
                stream_index: 0,
                codec_name: "subrip".to_string(),
                language: None,
                title: None,
                default: false,
                forced: false,
                burnable: true,
            }],
            "mp4",
            Path::new("/tmp/out.mp4"),
            SubtitleMode::Copy,
        ) {
            Ok(_) => panic!("expected mp4 subtitle copy planning to fail"),
            Err(reason) => reason,
        };
        assert!(reason.contains("cannot safely copy"));
    }

    #[test]
    fn heavy_audio_prefers_transcode() {
        let mut encoder_caps = crate::media::ffmpeg::EncoderCapabilities::default();
        encoder_caps.audio_encoders.insert("libopus".to_string());
        let plan = plan_audio(Some("flac"), Some(6), true, "mkv", None, &encoder_caps);
        assert!(matches!(
            plan,
            AudioStreamPlan::Transcode {
                codec: AudioCodec::Opus,
                ..
            }
        ));
    }

    #[test]
    fn heavy_audio_falls_back_to_aac_when_libopus_is_unavailable() {
        let encoder_caps = crate::media::ffmpeg::EncoderCapabilities::default();
        let plan = plan_audio(Some("flac"), Some(6), true, "mkv", None, &encoder_caps);
        assert!(matches!(
            plan,
            AudioStreamPlan::Transcode {
                codec: AudioCodec::Aac,
                ..
            }
        ));
    }

    #[test]
    fn vaapi_plan_includes_hwupload_filter() {
        let mut cfg = config();
        cfg.transcode.output_codec = OutputCodec::Hevc;
        let filters = plan_filters(
            &analysis(),
            Encoder::HevcVaapi,
            &cfg,
            &SubtitleStreamPlan::Drop,
            HdrMode::Preserve,
        );
        assert!(matches!(
            filters.as_slice(),
            [FilterStep::Format { .. }, FilterStep::HwUpload]
        ));
    }

    #[test]
    fn burn_prefers_forced_then_default_then_first_burnable() {
        let streams = vec![
            SubtitleStreamMetadata {
                stream_index: 0,
                codec_name: "subrip".to_string(),
                language: Some("eng".to_string()),
                title: None,
                default: false,
                forced: false,
                burnable: true,
            },
            SubtitleStreamMetadata {
                stream_index: 1,
                codec_name: "ass".to_string(),
                language: Some("spa".to_string()),
                title: None,
                default: true,
                forced: false,
                burnable: true,
            },
            SubtitleStreamMetadata {
                stream_index: 2,
                codec_name: "subrip".to_string(),
                language: Some("eng".to_string()),
                title: None,
                default: false,
                forced: true,
                burnable: true,
            },
        ];

        let plan = plan_subtitles(
            &streams,
            "mkv",
            Path::new("/tmp/out.mkv"),
            SubtitleMode::Burn,
        )
        .unwrap_or_else(|err| panic!("failed to build burn plan: {err}"));
        assert!(matches!(plan, SubtitleStreamPlan::Burn { stream_index: 2 }));
    }

    #[test]
    fn burn_fails_without_burnable_text_stream() {
        let reason = match plan_subtitles(
            &[SubtitleStreamMetadata {
                stream_index: 0,
                codec_name: "hdmv_pgs_subtitle".to_string(),
                language: None,
                title: None,
                default: false,
                forced: false,
                burnable: false,
            }],
            "mkv",
            Path::new("/tmp/out.mkv"),
            SubtitleMode::Burn,
        ) {
            Ok(_) => panic!("expected burn planning to fail without a burnable stream"),
            Err(reason) => reason,
        };
        assert!(reason.contains("No burnable"));
    }

    #[test]
    fn extract_plans_sidecar_output() {
        let plan = plan_subtitles(
            &[SubtitleStreamMetadata {
                stream_index: 0,
                codec_name: "subrip".to_string(),
                language: Some("eng".to_string()),
                title: None,
                default: true,
                forced: false,
                burnable: true,
            }],
            "mkv",
            Path::new("/tmp/library/movie-alchemist.mkv"),
            SubtitleMode::Extract,
        )
        .unwrap_or_else(|err| panic!("failed to build extract plan: {err}"));

        match plan {
            SubtitleStreamPlan::Extract { outputs } => {
                assert_eq!(outputs.len(), 1);
                assert_eq!(outputs[0].stream_index, 0);
                assert_eq!(outputs[0].codec, "srt");
                assert_eq!(
                    outputs[0].final_path,
                    Path::new("/tmp/library/movie-alchemist.eng.srt")
                );
                assert_eq!(
                    outputs[0].temp_path,
                    Path::new("/tmp/library/movie-alchemist.eng.srt.alchemist-part")
                );
            }
            _ => panic!("expected extract plan"),
        }
    }

    #[test]
    fn extract_sidecars_append_language_index_for_duplicates() {
        let plan = plan_subtitles(
            &[
                SubtitleStreamMetadata {
                    stream_index: 0,
                    codec_name: "subrip".to_string(),
                    language: Some("eng".to_string()),
                    title: None,
                    default: true,
                    forced: false,
                    burnable: true,
                },
                SubtitleStreamMetadata {
                    stream_index: 1,
                    codec_name: "ass".to_string(),
                    language: Some("eng".to_string()),
                    title: None,
                    default: false,
                    forced: false,
                    burnable: true,
                },
            ],
            "mkv",
            Path::new("/tmp/library/movie-alchemist.mkv"),
            SubtitleMode::Extract,
        )
        .unwrap_or_else(|err| panic!("failed to build extract plan: {err}"));

        match plan {
            SubtitleStreamPlan::Extract { outputs } => {
                assert_eq!(outputs.len(), 2);
                assert_eq!(
                    outputs[0].final_path,
                    Path::new("/tmp/library/movie-alchemist.eng.srt")
                );
                assert_eq!(
                    outputs[1].final_path,
                    Path::new("/tmp/library/movie-alchemist.eng.2.ass")
                );
            }
            _ => panic!("expected extract plan"),
        }
    }

    #[test]
    fn mp4_target_codec_to_mkv_remuxes_instead_of_skipping() {
        let mut source = analysis();
        source.metadata.container = "mp4".to_string();
        let decision = should_transcode(&source, &config(), OutputCodec::Hevc, "mkv");
        assert!(matches!(decision, TranscodeDecision::Remux { .. }));
    }

    #[test]
    fn already_target_codec_in_mkv_still_skips() {
        let mut source = analysis();
        source.metadata.container = "matroska".to_string();
        let decision = should_transcode(&source, &config(), OutputCodec::Hevc, "mkv");
        assert!(matches!(decision, TranscodeDecision::Skip { .. }));
    }

    #[test]
    fn av1_in_mkv_still_skips_instead_of_remuxing() {
        let mut source = analysis();
        source.metadata.codec_name = "av1".to_string();
        source.metadata.container = "matroska".to_string();
        let decision = should_transcode(&source, &config(), OutputCodec::Av1, "mkv");
        assert!(matches!(decision, TranscodeDecision::Skip { .. }));
    }

    #[test]
    fn already_target_codec_reason_is_stable() {
        let decision = should_transcode(&analysis(), &config(), OutputCodec::Hevc, "mkv");
        let TranscodeDecision::Skip { reason } = decision else {
            panic!("expected skip decision");
        };
        let explanation = crate::explanations::decision_from_legacy("skip", &reason);
        assert_eq!(explanation.code, "already_target_codec");
        assert_eq!(
            explanation.measured.get("codec"),
            Some(&serde_json::json!("hevc"))
        );
    }

    #[test]
    fn remux_reason_is_stable() {
        let mut source = analysis();
        source.metadata.container = "mp4".to_string();
        let decision = should_transcode(&source, &config(), OutputCodec::Hevc, "mkv");
        let TranscodeDecision::Remux { reason } = decision else {
            panic!("expected remux decision");
        };
        let explanation = crate::explanations::decision_from_legacy("remux", &reason);
        assert_eq!(explanation.code, "already_target_codec_wrong_container");
        assert_eq!(
            explanation.measured.get("target_extension"),
            Some(&serde_json::json!("mkv"))
        );
    }

    #[test]
    fn bpp_threshold_reason_is_stable() {
        let mut source = analysis();
        source.metadata.codec_name = "mpeg4".to_string();
        source.metadata.bit_depth = Some(8);
        source.metadata.video_bitrate_bps = Some(1_000_000);
        let decision = should_transcode(&source, &config(), OutputCodec::Av1, "mkv");
        let TranscodeDecision::Skip { reason } = decision else {
            panic!("expected skip decision");
        };
        let explanation = crate::explanations::decision_from_legacy("skip", &reason);
        assert_eq!(explanation.code, "bpp_below_threshold");
    }

    #[test]
    fn min_file_size_reason_is_stable() {
        let mut source = analysis();
        source.metadata.codec_name = "mpeg4".to_string();
        source.metadata.bit_depth = Some(8);
        source.metadata.size_bytes = 20 * 1024 * 1024;
        let decision = should_transcode(&source, &config(), OutputCodec::Av1, "mkv");
        let TranscodeDecision::Skip { reason } = decision else {
            panic!("expected skip decision");
        };
        let explanation = crate::explanations::decision_from_legacy("skip", &reason);
        assert_eq!(explanation.code, "below_min_file_size");
    }

    #[test]
    fn incomplete_metadata_reason_is_stable() {
        let mut source = analysis();
        source.metadata.codec_name = "mpeg4".to_string();
        source.metadata.bit_depth = Some(8);
        source.metadata.width = 0;
        let decision = should_transcode(&source, &config(), OutputCodec::Av1, "mkv");
        let TranscodeDecision::Skip { reason } = decision else {
            panic!("expected skip decision");
        };
        let explanation = crate::explanations::decision_from_legacy("skip", &reason);
        assert_eq!(explanation.code, "incomplete_metadata");
    }

    #[tokio::test]
    async fn no_available_encoders_reason_is_stable() {
        let mut cfg = config();
        cfg.hardware.allow_cpu_encoding = false;
        cfg.hardware.allow_cpu_fallback = false;
        cfg.transcode.allow_fallback = false;
        let planner = BasicPlanner::new(Arc::new(cfg), None);

        let plan = planner
            .plan(&analysis(), Path::new("/tmp/out.mkv"), None)
            .await
            .unwrap_or_else(|err| panic!("failed to build no-encoder plan: {err}"));

        let TranscodeDecision::Skip { reason } = plan.decision else {
            panic!("expected skip decision");
        };
        let explanation = crate::explanations::decision_from_legacy("skip", &reason);
        assert_eq!(explanation.code, "no_available_encoders");
    }

    #[tokio::test]
    async fn preferred_codec_unavailable_reason_is_stable() {
        let hw_info = HardwareInfo {
            vendor: Vendor::Intel,
            device_path: Some("/dev/dri/renderD128".to_string()),
            supported_codecs: vec!["hevc".to_string()],
            backends: vec![BackendCapability {
                kind: HardwareBackend::Qsv,
                codec: "hevc".to_string(),
                encoder: "hevc_qsv".to_string(),
                device_path: Some("/dev/dri/renderD128".to_string()),
            }],
            detection_notes: Vec::new(),
            selection_reason: String::new(),
            probe_summary: ProbeSummary::default(),
        };

        let mut cfg = config();
        cfg.hardware.allow_cpu_encoding = false;
        cfg.hardware.allow_cpu_fallback = false;
        cfg.transcode.output_codec = OutputCodec::Av1;
        cfg.transcode.allow_fallback = false;

        let planner = BasicPlanner::new(Arc::new(cfg), Some(hw_info));
        let plan = planner
            .plan(&analysis(), Path::new("/tmp/out.mkv"), None)
            .await
            .unwrap_or_else(|err| panic!("failed to build preferred-codec plan: {err}"));

        let TranscodeDecision::Skip { reason } = plan.decision else {
            panic!("expected skip decision");
        };
        let explanation = crate::explanations::decision_from_legacy("skip", &reason);
        assert_eq!(
            explanation.code,
            "preferred_codec_unavailable_fallback_disabled"
        );
    }

    #[test]
    fn gpu_codec_fallback_beats_cpu_requested_codec() {
        let inventory = EncoderInventory {
            gpu: vec![Encoder::HevcQsv],
            cpu: vec![Encoder::Av1Svt],
        };

        let (encoder, fallback) = select_encoder(OutputCodec::Av1, &inventory, true)
            .unwrap_or_else(|| panic!("expected selected encoder"));
        assert_eq!(encoder, Encoder::HevcQsv);
        assert_eq!(
            fallback.unwrap_or_else(|| panic!("expected fallback")).kind,
            FallbackKind::Codec
        );
    }

    #[test]
    fn encoder_selection_respects_detected_gpu_backend_order() {
        let inventory = EncoderInventory {
            gpu: vec![Encoder::Av1Vaapi, Encoder::Av1Qsv],
            cpu: Vec::new(),
        };

        let (encoder, fallback) = select_encoder(OutputCodec::Av1, &inventory, false)
            .unwrap_or_else(|| panic!("expected selected encoder"));
        assert_eq!(encoder, Encoder::Av1Vaapi);
        assert!(fallback.is_none());
    }

    #[test]
    fn gpu_host_does_not_use_cpu_when_fallback_is_disabled() {
        let inventory = EncoderInventory {
            gpu: vec![Encoder::HevcQsv],
            cpu: vec![Encoder::Av1Svt],
        };

        assert!(select_encoder(OutputCodec::Av1, &inventory, false).is_none());
    }

    #[test]
    fn cpu_only_host_can_use_requested_codec_without_fallback() {
        let inventory = EncoderInventory {
            gpu: Vec::new(),
            cpu: vec![Encoder::Av1Svt],
        };

        let (encoder, fallback) = select_encoder(OutputCodec::Av1, &inventory, false)
            .unwrap_or_else(|| panic!("expected selected encoder"));
        assert_eq!(encoder, Encoder::Av1Svt);
        assert!(fallback.is_none());
    }

    #[test]
    fn audio_stream_rules_strip_commentary_and_keep_main_audio() {
        let rules = crate::config::StreamRules {
            strip_audio_by_title: vec!["commentary".to_string()],
            keep_audio_languages: Vec::new(),
            keep_only_default_audio: false,
        };

        let kept = filter_audio_streams(
            &[
                AudioStreamMetadata {
                    stream_index: 0,
                    codec_name: "aac".to_string(),
                    language: Some("eng".to_string()),
                    title: Some("Director Commentary".to_string()),
                    channels: Some(2),
                    default: false,
                    forced: false,
                },
                AudioStreamMetadata {
                    stream_index: 1,
                    codec_name: "aac".to_string(),
                    language: Some("eng".to_string()),
                    title: Some("Main Audio".to_string()),
                    channels: Some(6),
                    default: true,
                    forced: false,
                },
            ],
            &rules,
        );

        assert_eq!(kept, Some(vec![1]));
    }

    #[test]
    fn audio_stream_rules_fall_back_to_default_when_all_filtered() {
        let rules = crate::config::StreamRules {
            strip_audio_by_title: vec!["commentary".to_string()],
            keep_audio_languages: vec!["jpn".to_string()],
            keep_only_default_audio: false,
        };

        let kept = filter_audio_streams(
            &[
                AudioStreamMetadata {
                    stream_index: 0,
                    codec_name: "aac".to_string(),
                    language: Some("eng".to_string()),
                    title: Some("Commentary".to_string()),
                    channels: Some(2),
                    default: false,
                    forced: false,
                },
                AudioStreamMetadata {
                    stream_index: 1,
                    codec_name: "aac".to_string(),
                    language: Some("eng".to_string()),
                    title: Some("Main Audio".to_string()),
                    channels: Some(6),
                    default: true,
                    forced: false,
                },
            ],
            &rules,
        );

        assert_eq!(kept, Some(vec![1]));
    }

    #[test]
    fn keep_audio_languages_overrides_default_only_rule() {
        let rules = crate::config::StreamRules {
            strip_audio_by_title: Vec::new(),
            keep_audio_languages: vec!["jpn".to_string()],
            keep_only_default_audio: true,
        };

        let kept = filter_audio_streams(
            &[
                AudioStreamMetadata {
                    stream_index: 0,
                    codec_name: "aac".to_string(),
                    language: Some("eng".to_string()),
                    title: Some("English".to_string()),
                    channels: Some(2),
                    default: true,
                    forced: false,
                },
                AudioStreamMetadata {
                    stream_index: 1,
                    codec_name: "aac".to_string(),
                    language: Some("jpn".to_string()),
                    title: Some("Japanese".to_string()),
                    channels: Some(2),
                    default: false,
                    forced: false,
                },
                AudioStreamMetadata {
                    stream_index: 2,
                    codec_name: "aac".to_string(),
                    language: None,
                    title: Some("Unknown".to_string()),
                    channels: Some(2),
                    default: false,
                    forced: false,
                },
            ],
            &rules,
        );

        assert_eq!(kept, Some(vec![1, 2]));
    }
}
