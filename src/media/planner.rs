use crate::config::{Config, OutputCodec, SubtitleMode};
use crate::error::Result;
use crate::media::pipeline::{
    AudioCodec, AudioStreamPlan, Encoder, FallbackKind, FilterStep, MediaAnalysis, PlannedFallback,
    Planner, RateControl, SidecarOutputPlan, SubtitleStreamMetadata, SubtitleStreamPlan,
    TranscodeDecision, TranscodePlan,
};
use crate::system::hardware::{HardwareBackend, HardwareInfo, Vendor};
use async_trait::async_trait;
use std::collections::HashSet;
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

#[async_trait]
impl Planner for BasicPlanner {
    async fn plan(&self, analysis: &MediaAnalysis, output_path: &Path) -> Result<TranscodePlan> {
        let container = normalize_container(output_path, &analysis.metadata.container);
        let available_encoders =
            build_available_encoders(&self.config, self.hw_info.as_ref(), &self.encoder_caps);

        if available_encoders.is_empty() {
            return Ok(skip_plan(
                "No available encoders for current hardware policy".to_string(),
                container,
                self.config.transcode.output_codec,
                self.config.transcode.allow_fallback,
                self.config.transcode.threads,
            ));
        }

        if !self.config.transcode.allow_fallback
            && !available_encoders
                .has_requested_codec_without_fallback(self.config.transcode.output_codec)
        {
            return Ok(skip_plan(
                format!(
                    "Preferred codec {} unavailable and fallback disabled",
                    self.config.transcode.output_codec.as_str()
                ),
                container,
                self.config.transcode.output_codec,
                self.config.transcode.allow_fallback,
                self.config.transcode.threads,
            ));
        }

        let decision = should_transcode(analysis, &self.config);

        if let TranscodeDecision::Skip { reason } = &decision {
            return Ok(skip_plan(
                reason.clone(),
                container,
                self.config.transcode.output_codec,
                self.config.transcode.allow_fallback,
                self.config.transcode.threads,
            ));
        }

        let Some((encoder, fallback)) = select_encoder(
            self.config.transcode.output_codec,
            &available_encoders,
            self.config.transcode.allow_fallback,
        ) else {
            return Ok(skip_plan(
                "No suitable encoder available".to_string(),
                container,
                self.config.transcode.output_codec,
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
                    self.config.transcode.output_codec,
                    self.config.transcode.allow_fallback,
                    self.config.transcode.threads,
                ))
            }
        };

        let audio = plan_audio(
            analysis.metadata.audio_codec.as_deref(),
            analysis.metadata.audio_channels,
            analysis.metadata.audio_is_heavy,
            &container,
        );
        let filters = plan_filters(analysis, encoder, &self.config, &subtitles);
        let (rate_control, encoder_preset) = encoder_runtime_settings(encoder, &self.config);

        Ok(TranscodePlan {
            decision,
            output_path: None,
            container,
            requested_codec: self.config.transcode.output_codec,
            output_codec: Some(encoder.output_codec()),
            encoder: Some(encoder),
            backend: Some(encoder.backend()),
            rate_control: Some(rate_control),
            encoder_preset,
            threads: self.config.transcode.threads,
            audio,
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

fn should_transcode(analysis: &MediaAnalysis, config: &Config) -> TranscodeDecision {
    let metadata = &analysis.metadata;
    let target_codec = config.transcode.output_codec;
    let target_codec_str = target_codec.as_str();

    if metadata.codec_name.eq_ignore_ascii_case(target_codec_str) && metadata.bit_depth == Some(10)
    {
        return TranscodeDecision::Skip {
            reason: format!("Already {} 10-bit", target_codec_str),
        };
    }

    if target_codec == OutputCodec::H264
        && metadata.codec_name.eq_ignore_ascii_case("h264")
        && metadata.bit_depth.is_some_and(|depth| depth <= 8)
    {
        return TranscodeDecision::Skip {
            reason: "Already H.264".to_string(),
        };
    }

    let estimated_container_bitrate = if metadata.size_bytes > 0 && metadata.duration_secs > 0.0 {
        Some(((metadata.size_bytes as f64 * 8.0) / metadata.duration_secs) as u64)
    } else {
        None
    };
    let video_bitrate_available = metadata.video_bitrate_bps.is_some();
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
            reason: "Incomplete metadata (resolution missing)".to_string(),
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

    let mut threshold = match analysis.confidence {
        crate::media::pipeline::AnalysisConfidence::High => config.transcode.min_bpp_threshold,
        crate::media::pipeline::AnalysisConfidence::Medium => {
            config.transcode.min_bpp_threshold * 0.7
        }
        crate::media::pipeline::AnalysisConfidence::Low => config.transcode.min_bpp_threshold * 0.5,
    };
    if target_codec == OutputCodec::Av1 {
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

    let min_size_bytes = config.transcode.min_file_size_mb * 1024 * 1024;
    if metadata.size_bytes < min_size_bytes {
        return TranscodeDecision::Skip {
            reason: format!(
                "File too small ({}MB < {}MB) to justify transcode overhead",
                metadata.size_bytes / 1024 / 1024,
                config.transcode.min_file_size_mb
            ),
        };
    }

    if metadata.codec_name.eq_ignore_ascii_case("h264") {
        return TranscodeDecision::Transcode {
            reason: "H.264 source prioritized for transcode".to_string(),
        };
    }

    TranscodeDecision::Transcode {
        reason: format!(
            "Ready for {} transcode (Current codec: {}, BPP: {})",
            target_codec_str,
            metadata.codec_name,
            bpp.map(|value| format!("{:.4}", value))
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
        requested_gpu_candidates(target_codec),
        &available_encoders.gpu,
    ) {
        return Some((encoder, None));
    }

    if !available_encoders.gpu.is_empty() {
        if allow_fallback {
            if let Some(encoder) = first_available(
                fallback_gpu_candidates(target_codec),
                &available_encoders.gpu,
            ) {
                return Some((encoder, Some(codec_fallback(target_codec, encoder))));
            }

            if let Some(encoder) = first_available(
                requested_cpu_candidates(target_codec),
                &available_encoders.cpu,
            ) {
                return Some((encoder, Some(cpu_fallback(target_codec, encoder))));
            }

            if let Some(encoder) = first_available(
                fallback_cpu_candidates(target_codec),
                &available_encoders.cpu,
            ) {
                return Some((encoder, Some(cpu_fallback(target_codec, encoder))));
            }
        }

        return None;
    }

    if let Some(encoder) = first_available(
        requested_cpu_candidates(target_codec),
        &available_encoders.cpu,
    ) {
        return Some((encoder, None));
    }

    if allow_fallback {
        if let Some(encoder) = first_available(
            fallback_cpu_candidates(target_codec),
            &available_encoders.cpu,
        ) {
            return Some((encoder, Some(codec_fallback(target_codec, encoder))));
        }
    }

    None
}

fn first_available(candidates: &[Encoder], available_encoders: &[Encoder]) -> Option<Encoder> {
    candidates
        .iter()
        .copied()
        .find(|candidate| available_encoders.contains(candidate))
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

fn encoder_runtime_settings(encoder: Encoder, config: &Config) -> (RateControl, Option<String>) {
    match encoder {
        Encoder::Av1Qsv | Encoder::HevcQsv | Encoder::H264Qsv => (
            RateControl::QsvQuality {
                value: parse_quality_u8(config.transcode.quality_profile.qsv_quality(), 23),
            },
            None,
        ),
        Encoder::Av1Nvenc | Encoder::HevcNvenc | Encoder::H264Nvenc => (
            RateControl::Cq { value: 25 },
            Some(config.transcode.quality_profile.nvenc_preset().to_string()),
        ),
        Encoder::Av1Videotoolbox | Encoder::HevcVideotoolbox | Encoder::H264Videotoolbox => (
            RateControl::Cq {
                value: parse_quality_u8(
                    config.transcode.quality_profile.videotoolbox_quality(),
                    65,
                ),
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
        Encoder::Av1Aom => (RateControl::Crf { value: 32 }, Some("6".to_string())),
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
    }
}

fn plan_audio(
    audio_codec: Option<&str>,
    audio_channels: Option<u32>,
    audio_is_heavy: bool,
    container: &str,
) -> AudioStreamPlan {
    let Some(audio_codec) = audio_codec else {
        return AudioStreamPlan::Copy;
    };

    let compatible = audio_copy_supported(container, audio_codec);
    if !compatible || audio_is_heavy {
        let codec = if container == "mp4" {
            AudioCodec::Aac
        } else {
            AudioCodec::Opus
        };
        return AudioStreamPlan::Transcode {
            codec,
            bitrate_kbps: audio_bitrate_kbps(codec, audio_channels),
        };
    }

    AudioStreamPlan::Copy
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
                Ok(SubtitleStreamPlan::Extract {
                    stream_indices: subtitle_streams
                        .iter()
                        .map(|stream| stream.stream_index)
                        .collect(),
                    sidecar_output: sidecar_output_for(output_path),
                })
            }
        }
    }
}

fn subtitle_copy_supported(container: &str, subtitle_streams: &[SubtitleStreamMetadata]) -> bool {
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

fn sidecar_output_for(output_path: &Path) -> SidecarOutputPlan {
    let parent = output_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let final_path = parent.join(format!("{stem}.subs.mks"));
    let temp_name = format!("{stem}.subs.mks.alchemist-part");
    SidecarOutputPlan {
        final_path,
        temp_path: parent.join(temp_name),
    }
}

fn plan_filters(
    analysis: &MediaAnalysis,
    encoder: Encoder,
    config: &Config,
    subtitles: &SubtitleStreamPlan,
) -> Vec<FilterStep> {
    let mut filters = Vec::new();

    if analysis.metadata.dynamic_range.is_hdr()
        && config.transcode.hdr_mode == crate::config::HdrMode::Tonemap
    {
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

    filters
}

fn parse_quality_u8(value: &str, default_value: u8) -> u8 {
    value.parse().unwrap_or(default_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HdrMode, QualityProfile, TonemapAlgorithm, TranscodeConfig};
    use crate::media::pipeline::{
        AnalysisConfidence, DynamicRange, MediaMetadata, SubtitleStreamMetadata,
    };

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
                dynamic_range: DynamicRange::Sdr,
            },
            warnings: Vec::new(),
            confidence: AnalysisConfidence::High,
        }
    }

    #[test]
    fn mp4_subtitle_copy_fails_fast() {
        let reason = plan_subtitles(
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
        )
        .unwrap_err();
        assert!(reason.contains("cannot safely copy"));
    }

    #[test]
    fn heavy_audio_prefers_transcode() {
        let plan = plan_audio(Some("flac"), Some(6), true, "mkv");
        assert!(matches!(
            plan,
            AudioStreamPlan::Transcode {
                codec: AudioCodec::Opus,
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
        .expect("burn plan");
        assert!(matches!(plan, SubtitleStreamPlan::Burn { stream_index: 2 }));
    }

    #[test]
    fn burn_fails_without_burnable_text_stream() {
        let reason = plan_subtitles(
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
        )
        .unwrap_err();
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
        .expect("extract plan");

        match plan {
            SubtitleStreamPlan::Extract {
                stream_indices,
                sidecar_output,
            } => {
                assert_eq!(stream_indices, vec![0]);
                assert_eq!(
                    sidecar_output.final_path,
                    Path::new("/tmp/library/movie-alchemist.subs.mks")
                );
                assert_eq!(
                    sidecar_output.temp_path,
                    Path::new("/tmp/library/movie-alchemist.subs.mks.alchemist-part")
                );
            }
            _ => panic!("expected extract plan"),
        }
    }

    #[test]
    fn gpu_codec_fallback_beats_cpu_requested_codec() {
        let inventory = EncoderInventory {
            gpu: vec![Encoder::HevcQsv],
            cpu: vec![Encoder::Av1Svt],
        };

        let (encoder, fallback) =
            select_encoder(OutputCodec::Av1, &inventory, true).expect("selected encoder");
        assert_eq!(encoder, Encoder::HevcQsv);
        assert_eq!(fallback.expect("fallback").kind, FallbackKind::Codec);
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

        let (encoder, fallback) =
            select_encoder(OutputCodec::Av1, &inventory, false).expect("selected encoder");
        assert_eq!(encoder, Encoder::Av1Svt);
        assert!(fallback.is_none());
    }
}
