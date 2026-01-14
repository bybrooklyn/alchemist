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

        let (fallback_occurred, used_encoder) =
            self.verify_encoder(&output_path, encoder).await;

        Ok(ExecutionResult {
            used_encoder,
            fallback_occurred,
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

    async fn verify_encoder(&self, output_path: &Path, requested: Encoder) -> (bool, Encoder) {
        let expected_codec = encoder_codec_name(requested);
        let actual_codec = crate::media::analyzer::Analyzer::probe_video_codec(output_path)
            .await
            .unwrap_or_default();

        if actual_codec.is_empty() || actual_codec.eq_ignore_ascii_case(expected_codec) {
            (false, requested)
        } else {
            (true, requested)
        }
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
