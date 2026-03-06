use crate::config::Config;
use crate::db::{AlchemistEvent, Job};
use crate::error::Result;
use crate::media::pipeline::{
    Encoder, ExecutionPlan, ExecutionResult, ExecutionStats, Executor, MediaAnalysis,
};
use crate::orchestrator::{TranscodeRequest, Transcoder};
use crate::system::hardware::HardwareInfo;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct FfmpegExecutor {
    transcoder: Arc<Transcoder>,
    config: Arc<Config>,
    hw_info: Option<HardwareInfo>,
    event_tx: Arc<broadcast::Sender<AlchemistEvent>>,
    dry_run: bool,
}

impl FfmpegExecutor {
    pub fn new(
        transcoder: Arc<Transcoder>,
        config: Arc<Config>,
        hw_info: Option<HardwareInfo>,
        event_tx: Arc<broadcast::Sender<AlchemistEvent>>,
        dry_run: bool,
    ) -> Self {
        Self {
            transcoder,
            config,
            hw_info,
            event_tx,
            dry_run,
        }
    }
}

#[async_trait]
impl Executor for FfmpegExecutor {
    async fn execute(
        &self,
        job: &Job,
        plan: &ExecutionPlan,
        analysis: &MediaAnalysis,
    ) -> Result<ExecutionResult> {
        let input_path = PathBuf::from(&job.input_path);
        let output_path = plan
            .output_path
            .as_ref()
            .cloned()
            .unwrap_or_else(|| PathBuf::from(&job.output_path));

        let encoder = match plan.encoder {
            Some(encoder) => encoder,
            None => {
                return Err(crate::error::AlchemistError::Config(
                    "Execution plan missing encoder".into(),
                ))
            }
        };

        if !self.encoder_available(encoder) {
            return Err(crate::error::AlchemistError::EncoderUnavailable(format!(
                "Requested encoder {:?} is not available",
                encoder
            )));
        }

        self.transcoder
            .transcode_media(TranscodeRequest {
                input: &input_path,
                output: &output_path,
                hw_info: self.hw_info.as_ref(),
                quality_profile: self.config.transcode.quality_profile,
                cpu_preset: self.config.hardware.cpu_preset,
                threads: self.config.transcode.threads,
                allow_fallback: self.config.transcode.allow_fallback,
                hdr_mode: self.config.transcode.hdr_mode,
                tonemap_algorithm: self.config.transcode.tonemap_algorithm,
                tonemap_peak: self.config.transcode.tonemap_peak,
                tonemap_desat: self.config.transcode.tonemap_desat,
                dry_run: self.dry_run,
                metadata: &analysis.metadata,
                encoder: plan.encoder,
                rate_control: plan.rate_control.clone(),
                event_target: Some((job.id, self.event_tx.clone())),
            })
            .await?;

        let (fallback_occurred, used_encoder, actual_codec, actual_encoder_name) =
            self.verify_encoder(&output_path, encoder).await;

        Ok(ExecutionResult {
            requested_encoder: encoder,
            used_encoder,
            fallback_occurred,
            actual_output_codec: actual_codec,
            actual_encoder_name,
            stats: ExecutionStats {
                encode_time_secs: 0.0, // Pipeline calculates this
                input_size: 0,
                output_size: 0,
                vmaf: None,
            },
        })
    }
}

impl FfmpegExecutor {
    fn encoder_available(&self, encoder: Encoder) -> bool {
        let caps = crate::media::ffmpeg::encoder_caps_clone();
        match encoder {
            Encoder::Av1Qsv => caps.has_video_encoder("av1_qsv"),
            Encoder::Av1Nvenc => caps.has_video_encoder("av1_nvenc"),
            Encoder::Av1Vaapi => caps.has_video_encoder("av1_vaapi"),
            Encoder::Av1Videotoolbox => caps.has_video_encoder("av1_videotoolbox"),
            Encoder::Av1Amf => caps.has_video_encoder("av1_amf"),
            Encoder::Av1Svt => caps.has_libsvtav1(),
            Encoder::Av1Aom => caps.has_video_encoder("libaom-av1"),
            Encoder::HevcQsv => caps.has_video_encoder("hevc_qsv"),
            Encoder::HevcNvenc => caps.has_video_encoder("hevc_nvenc"),
            Encoder::HevcVaapi => caps.has_video_encoder("hevc_vaapi"),
            Encoder::HevcVideotoolbox => caps.has_video_encoder("hevc_videotoolbox"),
            Encoder::HevcAmf => caps.has_video_encoder("hevc_amf"),
            Encoder::HevcX265 => caps.has_libx265(),
            Encoder::H264Qsv => caps.has_video_encoder("h264_qsv"),
            Encoder::H264Nvenc => caps.has_video_encoder("h264_nvenc"),
            Encoder::H264Vaapi => caps.has_video_encoder("h264_vaapi"),
            Encoder::H264Videotoolbox => caps.has_video_encoder("h264_videotoolbox"),
            Encoder::H264Amf => caps.has_video_encoder("h264_amf"),
            Encoder::H264X264 => caps.has_libx264(),
        }
    }

    async fn verify_encoder(
        &self,
        output_path: &Path,
        requested: Encoder,
    ) -> (
        bool,
        Encoder,
        Option<crate::config::OutputCodec>,
        Option<String>,
    ) {
        let expected_codec = encoder_codec_name(requested);
        let probe = crate::media::analyzer::Analyzer::probe_output_details(output_path)
            .await
            .ok();
        let actual_codec_name = probe
            .as_ref()
            .map(|p| p.codec_name.as_str())
            .unwrap_or_default()
            .to_string();
        let actual_codec = output_codec_from_name(&actual_codec_name);
        let actual_encoder_name = probe.and_then(|p| p.encoder_tag);
        let codec_matches =
            actual_codec_name.is_empty() || actual_codec_name.eq_ignore_ascii_case(expected_codec);
        let encoder_matches = actual_encoder_name
            .as_deref()
            .map(|name| encoder_tag_matches(requested, name))
            .unwrap_or(true);
        let fallback_occurred = !(codec_matches && encoder_matches);

        (
            fallback_occurred,
            requested,
            actual_codec,
            actual_encoder_name,
        )
    }
}

fn encoder_codec_name(encoder: Encoder) -> &'static str {
    match encoder {
        Encoder::Av1Qsv
        | Encoder::Av1Nvenc
        | Encoder::Av1Vaapi
        | Encoder::Av1Videotoolbox
        | Encoder::Av1Amf
        | Encoder::Av1Svt
        | Encoder::Av1Aom => "av1",
        Encoder::HevcQsv
        | Encoder::HevcNvenc
        | Encoder::HevcVaapi
        | Encoder::HevcVideotoolbox
        | Encoder::HevcAmf
        | Encoder::HevcX265 => "hevc",
        Encoder::H264Qsv
        | Encoder::H264Nvenc
        | Encoder::H264Vaapi
        | Encoder::H264Videotoolbox
        | Encoder::H264Amf
        | Encoder::H264X264 => "h264",
    }
}

fn output_codec_from_name(codec: &str) -> Option<crate::config::OutputCodec> {
    if codec.eq_ignore_ascii_case("av1") {
        Some(crate::config::OutputCodec::Av1)
    } else if codec.eq_ignore_ascii_case("hevc") || codec.eq_ignore_ascii_case("h265") {
        Some(crate::config::OutputCodec::Hevc)
    } else if codec.eq_ignore_ascii_case("h264") || codec.eq_ignore_ascii_case("avc") {
        Some(crate::config::OutputCodec::H264)
    } else {
        None
    }
}

fn encoder_tag_matches(requested: Encoder, encoder_tag: &str) -> bool {
    let tag = encoder_tag.to_ascii_lowercase();
    let expected_markers: &[&str] = match requested {
        Encoder::Av1Qsv | Encoder::HevcQsv | Encoder::H264Qsv => &["qsv"],
        Encoder::Av1Nvenc | Encoder::HevcNvenc | Encoder::H264Nvenc => &["nvenc"],
        Encoder::Av1Vaapi | Encoder::HevcVaapi | Encoder::H264Vaapi => &["vaapi"],
        Encoder::Av1Videotoolbox | Encoder::HevcVideotoolbox | Encoder::H264Videotoolbox => {
            &["videotoolbox"]
        }
        Encoder::Av1Amf | Encoder::HevcAmf | Encoder::H264Amf => &["amf"],
        Encoder::Av1Svt => &["svtav1", "svt-av1", "libsvtav1"],
        Encoder::Av1Aom => &["libaom", "aom"],
        Encoder::HevcX265 => &["x265", "libx265"],
        Encoder::H264X264 => &["x264", "libx264"],
    };

    expected_markers.iter().any(|marker| tag.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_codec_mapping_handles_common_aliases() {
        assert_eq!(
            output_codec_from_name("av1"),
            Some(crate::config::OutputCodec::Av1)
        );
        assert_eq!(
            output_codec_from_name("hevc"),
            Some(crate::config::OutputCodec::Hevc)
        );
        assert_eq!(
            output_codec_from_name("h265"),
            Some(crate::config::OutputCodec::Hevc)
        );
        assert_eq!(
            output_codec_from_name("h264"),
            Some(crate::config::OutputCodec::H264)
        );
        assert_eq!(
            output_codec_from_name("avc"),
            Some(crate::config::OutputCodec::H264)
        );
        assert_eq!(output_codec_from_name("vp9"), None);
    }

    #[test]
    fn encoder_tag_matching_detects_mismatch() {
        assert!(encoder_tag_matches(
            Encoder::Av1Nvenc,
            "Lavc61.3.100 av1_nvenc"
        ));
        assert!(!encoder_tag_matches(
            Encoder::Av1Nvenc,
            "Lavc61.3.100 libsvtav1"
        ));
    }
}
