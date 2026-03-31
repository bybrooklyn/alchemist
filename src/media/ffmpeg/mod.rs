//! FFmpeg wrapper module for Alchemist.
//! Provides typed command generation, capability detection, and progress parsing.

use crate::error::{AlchemistError, Result};
use crate::media::pipeline::{
    AudioCodec, AudioStreamPlan, Encoder, FilterStep, RateControl, SubtitleStreamPlan,
    TranscodePlan,
};
use crate::system::hardware::{CommandRunner, HardwareInfo, SystemCommandRunner};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use tracing::{debug, info, warn};

mod amf;
mod cpu;
mod nvenc;
mod qsv;
mod vaapi;
mod videotoolbox;

#[derive(Debug, Clone, Default)]
pub struct HardwareAccelerators {
    pub available: HashSet<String>,
}

impl HardwareAccelerators {
    pub fn detect() -> Result<Self> {
        Self::detect_with_runner(&SystemCommandRunner)
    }

    pub fn detect_with_runner<R: CommandRunner + ?Sized>(runner: &R) -> Result<Self> {
        let args = vec!["-hide_banner".to_string(), "-hwaccels".to_string()];
        let output = runner.output("ffmpeg", &args).map_err(|e| {
            AlchemistError::FFmpeg(format!("Failed to run ffmpeg -hwaccels: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut available = HashSet::new();
        for line in stdout.lines().skip(1) {
            let accel = line.trim();
            if !accel.is_empty() {
                available.insert(accel.to_string());
            }
        }

        info!("Detected hardware accelerators: {:?}", available);
        Ok(Self { available })
    }
}

#[derive(Debug, Clone, Default)]
pub struct EncoderCapabilities {
    pub video_encoders: HashSet<String>,
    pub audio_encoders: HashSet<String>,
}

impl EncoderCapabilities {
    pub fn detect() -> Result<Self> {
        Self::detect_with_runner(&SystemCommandRunner)
    }

    pub fn detect_with_runner<R: CommandRunner + ?Sized>(runner: &R) -> Result<Self> {
        let args = vec!["-hide_banner".to_string(), "-encoders".to_string()];
        let output = runner.output("ffmpeg", &args).map_err(|e| {
            AlchemistError::FFmpeg(format!("Failed to run ffmpeg -encoders: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut video_encoders = HashSet::new();
        let mut audio_encoders = HashSet::new();

        for line in stdout.lines() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('-') || trimmed.starts_with("Encoders:") {
                continue;
            }

            let mut parts = trimmed.split_whitespace();
            let flags = match parts.next() {
                Some(flags) if flags.len() == 6 => flags,
                _ => continue,
            };
            let encoder_name = match parts.next() {
                Some(name) => name,
                None => continue,
            };

            if flags.starts_with('V') {
                video_encoders.insert(encoder_name.to_string());
            } else if flags.starts_with('A') {
                audio_encoders.insert(encoder_name.to_string());
            }
        }

        debug!(
            "Detected {} video encoders, {} audio encoders",
            video_encoders.len(),
            audio_encoders.len()
        );

        Ok(Self {
            video_encoders,
            audio_encoders,
        })
    }

    pub fn has_video_encoder(&self, name: &str) -> bool {
        self.video_encoders.contains(name)
    }

    pub fn has_libsvtav1(&self) -> bool {
        self.has_video_encoder("libsvtav1")
    }

    pub fn has_libx265(&self) -> bool {
        self.has_video_encoder("libx265")
    }

    pub fn has_libx264(&self) -> bool {
        self.has_video_encoder("libx264")
    }
}

pub struct FFmpegCommandBuilder<'a> {
    input: &'a Path,
    output: &'a Path,
    metadata: &'a crate::media::pipeline::MediaMetadata,
    plan: &'a TranscodePlan,
    hw_info: Option<&'a HardwareInfo>,
}

impl<'a> FFmpegCommandBuilder<'a> {
    pub fn new(
        input: &'a Path,
        output: &'a Path,
        metadata: &'a crate::media::pipeline::MediaMetadata,
        plan: &'a TranscodePlan,
    ) -> Self {
        Self {
            input,
            output,
            metadata,
            plan,
            hw_info: None,
        }
    }

    pub fn with_hardware(mut self, hw_info: Option<&'a HardwareInfo>) -> Self {
        self.hw_info = hw_info;
        self
    }

    pub fn build(self) -> Result<tokio::process::Command> {
        let args = self.build_args()?;
        let mut cmd = tokio::process::Command::new("ffmpeg");
        cmd.args(&args);
        Ok(cmd)
    }

    pub fn build_args(&self) -> Result<Vec<String>> {
        if self.plan.is_remux {
            return Ok(vec![
                "-v".to_string(),
                "error".to_string(),
                "-i".to_string(),
                self.input.display().to_string(),
                "-c".to_string(),
                "copy".to_string(),
                "-map".to_string(),
                "0".to_string(),
                "-y".to_string(),
                self.output.display().to_string(),
            ]);
        }

        let encoder = self
            .plan
            .encoder
            .ok_or_else(|| AlchemistError::Config("Transcode plan missing encoder".into()))?;
        let rate_control = self.plan.rate_control.clone();
        let mut args = vec![
            "-hide_banner".to_string(),
            "-y".to_string(),
            "-nostats".to_string(),
            "-progress".to_string(),
            "pipe:2".to_string(),
            "-i".to_string(),
            self.input.display().to_string(),
            "-map_metadata".to_string(),
            "0".to_string(),
            "-map".to_string(),
            "0:v:0".to_string(),
        ];

        if !matches!(self.plan.audio, AudioStreamPlan::Drop) {
            match &self.plan.audio_stream_indices {
                None => {
                    args.push("-map".to_string());
                    args.push("0:a?".to_string());
                }
                Some(indices) => {
                    for &index in indices {
                        args.push("-map".to_string());
                        args.push(format!("0:a:{index}"));
                    }
                }
            }
        }
        if matches!(self.plan.subtitles, SubtitleStreamPlan::CopyAllCompatible) {
            args.push("-map".to_string());
            args.push("0:s?".to_string());
        }

        match encoder {
            Encoder::Av1Qsv | Encoder::HevcQsv | Encoder::H264Qsv => {
                qsv::append_args(
                    &mut args,
                    encoder,
                    self.hw_info,
                    rate_control,
                    default_quality(&self.plan.rate_control, 23),
                );
            }
            Encoder::Av1Nvenc | Encoder::HevcNvenc | Encoder::H264Nvenc => {
                nvenc::append_args(
                    &mut args,
                    encoder,
                    rate_control,
                    self.plan.encoder_preset.as_deref(),
                );
            }
            Encoder::Av1Vaapi | Encoder::HevcVaapi | Encoder::H264Vaapi => {
                vaapi::append_args(&mut args, encoder, self.hw_info);
            }
            Encoder::Av1Amf | Encoder::HevcAmf | Encoder::H264Amf => {
                amf::append_args(&mut args, encoder);
            }
            Encoder::Av1Videotoolbox | Encoder::HevcVideotoolbox | Encoder::H264Videotoolbox => {
                videotoolbox::append_args(
                    &mut args,
                    encoder,
                    rate_control,
                    default_quality(&self.plan.rate_control, 65),
                );
            }
            Encoder::Av1Svt | Encoder::Av1Aom | Encoder::HevcX265 | Encoder::H264X264 => {
                cpu::append_args(
                    &mut args,
                    encoder,
                    rate_control,
                    self.plan.encoder_preset.as_deref(),
                );
            }
        }

        if let Some(filtergraph) = render_filtergraph(self.input, &self.plan.filters) {
            args.push("-vf".to_string());
            args.push(filtergraph);
        }

        if self.plan.threads > 0 {
            args.push("-threads".to_string());
            args.push(self.plan.threads.to_string());
        }

        apply_audio_plan(&mut args, &self.plan.audio);
        apply_subtitle_plan(&mut args, &self.plan.subtitles);
        apply_color_metadata(&mut args, self.metadata, &self.plan.filters);

        if matches!(self.plan.container.as_str(), "mp4" | "m4v" | "mov") {
            args.push("-movflags".to_string());
            args.push("+faststart".to_string());
        }

        args.push("-f".to_string());
        args.push(output_format_name(&self.plan.container).to_string());
        args.push(self.output.display().to_string());
        Ok(args)
    }

    pub fn build_subtitle_extract_args(&self) -> Result<Option<Vec<String>>> {
        let SubtitleStreamPlan::Extract { outputs } = &self.plan.subtitles else {
            return Ok(None);
        };
        if outputs.is_empty() {
            return Ok(None);
        }

        let mut args = vec![
            "-hide_banner".to_string(),
            "-y".to_string(),
            "-nostats".to_string(),
            "-i".to_string(),
            self.input.display().to_string(),
        ];

        for sidecar_output in outputs {
            args.push("-map".to_string());
            args.push(format!("0:s:{}", sidecar_output.stream_index));
            args.push("-c:s".to_string());
            args.push(sidecar_output.codec.clone());
            args.push("-f".to_string());
            args.push(sidecar_output.codec.clone());
            args.push(sidecar_output.temp_path.display().to_string());
        }

        Ok(Some(args))
    }
}

fn default_quality(rate_control: &Option<RateControl>, fallback: u8) -> u8 {
    match rate_control {
        Some(RateControl::Cq { value }) => *value,
        Some(RateControl::QsvQuality { value }) => *value,
        Some(RateControl::Crf { value }) => *value,
        None => fallback,
    }
}

fn apply_audio_plan(args: &mut Vec<String>, plan: &AudioStreamPlan) {
    match plan {
        AudioStreamPlan::Copy => {
            args.extend(["-c:a".to_string(), "copy".to_string()]);
        }
        AudioStreamPlan::Transcode {
            codec,
            bitrate_kbps,
            channels,
        } => {
            args.extend([
                "-c:a".to_string(),
                codec.ffmpeg_name().to_string(),
                "-b:a".to_string(),
                format!("{bitrate_kbps}k"),
            ]);
            if let Some(channels) = channels {
                args.extend(["-ac".to_string(), channels.to_string()]);
            }
            if matches!(codec, AudioCodec::Aac) {
                args.extend(["-profile:a".to_string(), "aac_low".to_string()]);
            }
        }
        AudioStreamPlan::Drop => {
            args.push("-an".to_string());
        }
    }
}

fn apply_subtitle_plan(args: &mut Vec<String>, plan: &SubtitleStreamPlan) {
    match plan {
        SubtitleStreamPlan::CopyAllCompatible => {
            args.extend(["-c:s".to_string(), "copy".to_string()]);
        }
        SubtitleStreamPlan::Drop
        | SubtitleStreamPlan::Burn { .. }
        | SubtitleStreamPlan::Extract { .. } => {
            args.push("-sn".to_string());
        }
    }
}

fn apply_color_metadata(
    args: &mut Vec<String>,
    metadata: &crate::media::pipeline::MediaMetadata,
    filters: &[FilterStep],
) {
    let tonemapped = filters
        .iter()
        .any(|step| matches!(step, FilterStep::Tonemap { .. }));

    if tonemapped {
        args.extend([
            "-color_primaries".to_string(),
            "bt709".to_string(),
            "-color_trc".to_string(),
            "bt709".to_string(),
            "-colorspace".to_string(),
            "bt709".to_string(),
            "-color_range".to_string(),
            "tv".to_string(),
        ]);
        return;
    }

    if let Some(ref primaries) = metadata.color_primaries {
        args.extend(["-color_primaries".to_string(), primaries.clone()]);
    }
    if let Some(ref transfer) = metadata.color_transfer {
        args.extend(["-color_trc".to_string(), transfer.clone()]);
    }
    if let Some(ref space) = metadata.color_space {
        args.extend(["-colorspace".to_string(), space.clone()]);
    }
    if let Some(ref range) = metadata.color_range {
        args.extend(["-color_range".to_string(), range.clone()]);
    }
}

fn render_filtergraph(input: &Path, filters: &[FilterStep]) -> Option<String> {
    if filters.is_empty() {
        return None;
    }

    let graph = filters
        .iter()
        .map(|step| match step {
            FilterStep::Tonemap {
                algorithm,
                peak,
                desat,
            } => format!(
                "zscale=t=linear:npl={peak},tonemap=tonemap={}:desat={desat},zscale=p=bt709:t=bt709:m=bt709:r=tv,format=yuv420p,setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=tv",
                algorithm.as_str()
            ),
            FilterStep::Format { pixel_format } => format!("format={pixel_format}"),
            FilterStep::SubtitleBurn { stream_index } => format!(
                "subtitles=filename='{}':si={stream_index}",
                escape_filter_path(input)
            ),
            FilterStep::HwUpload => "hwupload".to_string(),
        })
        .collect::<Vec<_>>()
        .join(",");

    Some(graph)
}

fn escape_filter_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}

fn output_format_name(container: &str) -> &str {
    match container {
        "mkv" => "matroska",
        "m4v" => "mp4",
        other => other,
    }
}

#[derive(Debug, Clone, Default)]
pub struct FFmpegProgress {
    pub frame: u64,
    pub fps: f64,
    pub bitrate: String,
    pub total_size: u64,
    pub time: String,
    pub time_seconds: f64,
    pub speed: String,
}

impl FFmpegProgress {
    pub fn parse_line(line: &str) -> Option<Self> {
        if !line.contains("time=") && !line.contains("out_time=") {
            return None;
        }

        let mut progress = Self::default();
        let line = line.replace('=', "= ");
        let parts: Vec<&str> = line.split_whitespace().collect();

        for i in 0..parts.len() {
            match parts[i] {
                "frame=" => {
                    if let Some(val) = parts.get(i + 1) {
                        progress.frame = val.parse().unwrap_or(0);
                    }
                }
                "fps=" => {
                    if let Some(val) = parts.get(i + 1) {
                        progress.fps = val.parse().unwrap_or(0.0);
                    }
                }
                "bitrate=" => {
                    if let Some(val) = parts.get(i + 1) {
                        progress.bitrate = val.to_string();
                    }
                }
                "total_size=" => {
                    if let Some(val) = parts.get(i + 1) {
                        progress.total_size = val.parse().unwrap_or(0);
                    }
                }
                "time=" | "out_time=" => {
                    if let Some(val) = parts.get(i + 1) {
                        progress.time = val.to_string();
                        progress.time_seconds = Self::parse_time(val);
                    }
                }
                "speed=" => {
                    if let Some(val) = parts.get(i + 1) {
                        progress.speed = val.to_string();
                    }
                }
                _ => {}
            }
        }

        if progress.time_seconds > 0.0 || progress.frame > 0 {
            Some(progress)
        } else {
            None
        }
    }

    fn parse_time(s: &str) -> f64 {
        let parts: Vec<&str> = s.split(':').collect();
        let mut total_seconds = 0.0;
        let mut multiplier = 1.0;
        for part in parts.into_iter().rev() {
            total_seconds += part.parse::<f64>().unwrap_or(0.0) * multiplier;
            multiplier *= 60.0;
        }
        total_seconds
    }

    pub fn percentage(&self, total_duration: f64) -> f64 {
        if total_duration <= 0.0 {
            return 0.0;
        }
        (self.time_seconds / total_duration * 100.0).min(100.0)
    }
}

#[derive(Debug, Default)]
pub struct FFmpegProgressState {
    current: FFmpegProgress,
}

impl FFmpegProgressState {
    pub fn ingest_line(&mut self, line: &str) -> Option<FFmpegProgress> {
        if !line.contains(' ') {
            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "frame" => self.current.frame = value.parse().unwrap_or(0),
                    "fps" => self.current.fps = value.parse().unwrap_or(0.0),
                    "bitrate" => self.current.bitrate = value.to_string(),
                    "total_size" => self.current.total_size = value.parse().unwrap_or(0),
                    "out_time" => {
                        self.current.time = value.to_string();
                        self.current.time_seconds = FFmpegProgress::parse_time(value);
                    }
                    "out_time_ms" => {
                        let micros: f64 = value.parse().unwrap_or(0.0);
                        if self.current.time_seconds == 0.0 && micros > 0.0 {
                            self.current.time_seconds = micros / 1_000_000.0;
                        }
                    }
                    "speed" => self.current.speed = value.to_string(),
                    "progress" if matches!(value, "continue" | "end") => {
                        if self.current.time_seconds > 0.0 || self.current.frame > 0 {
                            return Some(self.current.clone());
                        }
                    }
                    _ => {}
                }

                return None;
            }
        }

        if let Some(progress) = FFmpegProgress::parse_line(line) {
            self.current = progress.clone();
            return Some(progress);
        }

        None
    }
}

fn encoder_caps() -> &'static EncoderCapabilities {
    static CAPS: OnceLock<EncoderCapabilities> = OnceLock::new();
    CAPS.get_or_init(|| EncoderCapabilities::detect().unwrap_or_default())
}

pub fn encoder_caps_clone() -> EncoderCapabilities {
    encoder_caps().clone()
}

pub fn warm_encoder_cache() {
    let caps = encoder_caps();
    info!(
        "Encoder capabilities cached: video_encoders={}, audio_encoders={}",
        caps.video_encoders.len(),
        caps.audio_encoders.len()
    );
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScore {
    pub vmaf: Option<f64>,
    pub psnr: Option<f64>,
    pub ssim: Option<f64>,
}

impl QualityScore {
    pub fn compute(original: &Path, encoded: &Path) -> Result<Self> {
        info!("Computing quality metrics for {:?}", encoded);

        let output = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-i")
            .arg(encoded)
            .arg("-i")
            .arg(original)
            .arg("-lavfi")
            .arg("libvmaf=log_fmt=json:log_path=-")
            .arg("-f")
            .arg("null")
            .arg("-")
            .output()
            .map_err(|e| AlchemistError::FFmpeg(format!("Failed to run VMAF: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AlchemistError::QualityCheckFailed(format!(
                "VMAF check failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let vmaf = Self::extract_vmaf_score_json(&stdout)
            .or_else(|| Self::extract_vmaf_score_text(&stdout))
            .or_else(|| Self::extract_vmaf_score_text(&stderr))
            .or_else(|| Self::extract_vmaf_score_json(&stderr));

        if vmaf.is_none() {
            warn!("Could not extract VMAF score from output");
        }

        Ok(Self {
            vmaf,
            psnr: None,
            ssim: None,
        })
    }

    fn extract_vmaf_score_text(output: &str) -> Option<f64> {
        for line in output.lines() {
            if line.contains("VMAF score:") {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 2 {
                    return parts[1].trim().parse().ok();
                }
            }
        }
        None
    }

    fn extract_vmaf_score_json(output: &str) -> Option<f64> {
        let trimmed = output.trim();
        let json_str = if trimmed.starts_with('{') && trimmed.ends_with('}') {
            trimmed
        } else {
            let start = trimmed.find('{')?;
            let end = trimmed.rfind('}')?;
            if end <= start {
                return None;
            }
            &trimmed[start..=end]
        };

        let value: Value = serde_json::from_str(json_str).ok()?;
        let pooled = value.get("pooled_metrics")?;
        let vmaf = pooled.get("vmaf")?;
        vmaf.get("mean")
            .and_then(|v| v.as_f64())
            .or_else(|| vmaf.get("harmonic_mean").and_then(|v| v.as_f64()))
    }

    pub fn is_acceptable(&self, min_vmaf: f64) -> bool {
        self.vmaf.map(|v| v >= min_vmaf).unwrap_or(true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodeStats {
    pub input_size_bytes: u64,
    pub output_size_bytes: u64,
    pub compression_ratio: f64,
    pub encode_time_seconds: f64,
    pub encode_speed: f64,
    pub avg_bitrate_kbps: f64,
    pub quality_score: Option<QualityScore>,
}

impl EncodeStats {
    pub fn new(
        input_size_bytes: u64,
        output_size_bytes: u64,
        encode_time_seconds: f64,
        duration_seconds: f64,
    ) -> Self {
        let compression_ratio = if input_size_bytes > 0 {
            1.0 - (output_size_bytes as f64 / input_size_bytes as f64)
        } else {
            0.0
        };

        let encode_speed = if encode_time_seconds > 0.0 {
            duration_seconds / encode_time_seconds
        } else {
            0.0
        };

        let avg_bitrate_kbps = if duration_seconds > 0.0 {
            (output_size_bytes as f64 * 8.0) / (duration_seconds * 1000.0)
        } else {
            0.0
        };

        Self {
            input_size_bytes,
            output_size_bytes,
            compression_ratio,
            encode_time_seconds,
            encode_speed,
            avg_bitrate_kbps,
            quality_score: None,
        }
    }

    pub fn with_quality(mut self, score: QualityScore) -> Self {
        self.quality_score = Some(score);
        self
    }
}

pub fn verify_ffmpeg() -> Result<String> {
    let output = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map_err(|e| AlchemistError::FFmpeg(format!("FFmpeg not found: {}", e)))?;

    if !output.status.success() {
        return Err(AlchemistError::FFmpeg("FFmpeg returned error".into()));
    }

    let version = String::from_utf8_lossy(&output.stdout);
    let first_line = version.lines().next().unwrap_or("unknown");

    info!("FFmpeg version: {}", first_line);
    Ok(first_line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OutputCodec;
    use crate::media::pipeline::{
        DynamicRange, MediaMetadata, RateControl, SidecarOutputPlan, SubtitleStreamMetadata,
        TranscodeDecision,
    };
    use crate::system::hardware::CommandRunner;
    use std::process::Output;

    fn metadata() -> MediaMetadata {
        MediaMetadata {
            path: "/tmp/input.mkv".into(),
            duration_secs: 120.0,
            codec_name: "hevc".to_string(),
            width: 1920,
            height: 1080,
            bit_depth: Some(10),
            color_primaries: Some("bt2020".to_string()),
            color_transfer: Some("smpte2084".to_string()),
            color_space: Some("bt2020nc".to_string()),
            color_range: Some("tv".to_string()),
            size_bytes: 500 * 1024 * 1024,
            video_bitrate_bps: Some(8_000_000),
            container_bitrate_bps: Some(8_500_000),
            fps: 24.0,
            container: "matroska".to_string(),
            audio_codec: Some("aac".to_string()),
            audio_bitrate_bps: Some(192_000),
            audio_channels: Some(2),
            audio_is_heavy: false,
            subtitle_streams: Vec::new(),
            audio_streams: Vec::new(),
            dynamic_range: DynamicRange::Hdr10,
        }
    }

    fn plan_for(encoder: Encoder) -> TranscodePlan {
        let mut filters = vec![FilterStep::Tonemap {
            algorithm: crate::config::TonemapAlgorithm::Hable,
            peak: 100.0,
            desat: 0.2,
        }];
        if encoder.backend() == crate::media::pipeline::EncoderBackend::Vaapi {
            filters.push(FilterStep::Format {
                pixel_format: "nv12".to_string(),
            });
            filters.push(FilterStep::HwUpload);
        }

        TranscodePlan {
            decision: TranscodeDecision::Transcode {
                reason: "test".to_string(),
            },
            is_remux: false,
            output_path: None,
            container: "mkv".to_string(),
            requested_codec: encoder.output_codec(),
            output_codec: Some(encoder.output_codec()),
            encoder: Some(encoder),
            backend: Some(encoder.backend()),
            rate_control: Some(match encoder {
                Encoder::Av1Qsv | Encoder::HevcQsv | Encoder::H264Qsv => {
                    RateControl::QsvQuality { value: 23 }
                }
                Encoder::Av1Svt | Encoder::Av1Aom | Encoder::HevcX265 | Encoder::H264X264 => {
                    RateControl::Crf { value: 21 }
                }
                _ => RateControl::Cq { value: 25 },
            }),
            encoder_preset: Some(match encoder {
                Encoder::Av1Nvenc | Encoder::HevcNvenc | Encoder::H264Nvenc => "p4".to_string(),
                Encoder::Av1Svt => "8".to_string(),
                Encoder::Av1Aom => "6".to_string(),
                Encoder::HevcX265 | Encoder::H264X264 => "medium".to_string(),
                _ => "".to_string(),
            }),
            threads: 0,
            audio: AudioStreamPlan::Copy,
            audio_stream_indices: None,
            subtitles: SubtitleStreamPlan::CopyAllCompatible,
            filters,
            allow_fallback: true,
            fallback: None,
        }
    }

    fn hw_info(path: &str) -> HardwareInfo {
        HardwareInfo {
            vendor: crate::system::hardware::Vendor::Intel,
            device_path: Some(path.to_string()),
            supported_codecs: vec!["av1".to_string(), "hevc".to_string(), "h264".to_string()],
            backends: vec![crate::system::hardware::BackendCapability {
                kind: crate::system::hardware::HardwareBackend::Qsv,
                codec: "av1".to_string(),
                encoder: "av1_qsv".to_string(),
                device_path: Some(path.to_string()),
            }],
            detection_notes: Vec::new(),
            selection_reason: String::new(),
            probe_summary: crate::system::hardware::ProbeSummary::default(),
        }
    }

    struct FakeRunner {
        stdout: Vec<u8>,
    }

    impl CommandRunner for FakeRunner {
        fn output(&self, _program: &str, _args: &[String]) -> std::io::Result<Output> {
            Ok(Output {
                status: exit_status(true),
                stdout: self.stdout.clone(),
                stderr: Vec::new(),
            })
        }
    }

    fn exit_status(success: bool) -> std::process::ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(if success { 0 } else { 1 } << 8)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(if success { 0 } else { 1 })
        }
    }

    #[test]
    fn test_progress_parsing() {
        let line =
            "frame=  100 fps=25.0 bitrate=1500kbps total_size=1000000 time=00:00:04.00 speed=1.5x";
        let progress = FFmpegProgress::parse_line(line).expect("expected progress parse");

        assert_eq!(progress.frame, 100);
        assert_eq!(progress.fps, 25.0);
        assert_eq!(progress.time, "00:00:04.00");
        assert!((progress.time_seconds - 4.0).abs() < 0.01);
    }

    #[test]
    fn structured_progress_parsing_emits_on_progress_marker() {
        let mut state = FFmpegProgressState::default();
        assert!(state.ingest_line("frame=42").is_none());
        assert!(state.ingest_line("out_time=00:00:01.50").is_none());
        let progress = state
            .ingest_line("progress=continue")
            .expect("expected structured progress");
        assert_eq!(progress.frame, 42);
        assert!((progress.time_seconds - 1.5).abs() < 0.01);
    }

    #[test]
    fn command_args_cover_cpu_backend() {
        let metadata = metadata();
        let plan = plan_for(Encoder::H264X264);
        let builder = FFmpegCommandBuilder::new(
            Path::new("/tmp/in.mkv"),
            Path::new("/tmp/out.mkv"),
            &metadata,
            &plan,
        );
        let args = builder.build_args().expect("args");
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"-progress".to_string()));
    }

    #[test]
    fn command_args_cover_nvenc_backend() {
        let metadata = metadata();
        let plan = plan_for(Encoder::HevcNvenc);
        let builder = FFmpegCommandBuilder::new(
            Path::new("/tmp/in.mkv"),
            Path::new("/tmp/out.mkv"),
            &metadata,
            &plan,
        );
        let args = builder.build_args().expect("args");
        assert!(args.contains(&"hevc_nvenc".to_string()));
        assert!(args.contains(&"p4".to_string()));
    }

    #[test]
    fn command_args_cover_qsv_backend() {
        let metadata = metadata();
        let plan = plan_for(Encoder::Av1Qsv);
        let hw_info = hw_info("/dev/dri/renderD129");
        let builder = FFmpegCommandBuilder::new(
            Path::new("/tmp/in.mkv"),
            Path::new("/tmp/out.mkv"),
            &metadata,
            &plan,
        )
        .with_hardware(Some(&hw_info));
        let args = builder.build_args().expect("args");
        assert!(args.contains(&"av1_qsv".to_string()));
        assert!(args.contains(&"-init_hw_device".to_string()));
    }

    #[test]
    fn command_args_cover_vaapi_backend() {
        let metadata = metadata();
        let mut info = hw_info("/dev/dri/renderD128");
        info.vendor = crate::system::hardware::Vendor::Amd;
        info.backends = vec![crate::system::hardware::BackendCapability {
            kind: crate::system::hardware::HardwareBackend::Vaapi,
            codec: "hevc".to_string(),
            encoder: "hevc_vaapi".to_string(),
            device_path: Some("/dev/dri/renderD128".to_string()),
        }];
        let plan = plan_for(Encoder::HevcVaapi);
        let builder = FFmpegCommandBuilder::new(
            Path::new("/tmp/in.mkv"),
            Path::new("/tmp/out.mkv"),
            &metadata,
            &plan,
        )
        .with_hardware(Some(&info));
        let args = builder.build_args().expect("args");
        assert!(args.contains(&"hevc_vaapi".to_string()));
        assert!(args.iter().any(|arg| arg.contains("format=nv12,hwupload")));
    }

    #[test]
    fn command_args_cover_videotoolbox_backend() {
        let metadata = metadata();
        let plan = plan_for(Encoder::HevcVideotoolbox);
        let builder = FFmpegCommandBuilder::new(
            Path::new("/tmp/in.mkv"),
            Path::new("/tmp/out.mkv"),
            &metadata,
            &plan,
        );
        let args = builder.build_args().expect("args");
        assert!(args.contains(&"hevc_videotoolbox".to_string()));
    }

    #[test]
    fn command_args_cover_amf_backend() {
        let metadata = metadata();
        let plan = plan_for(Encoder::HevcAmf);
        let builder = FFmpegCommandBuilder::new(
            Path::new("/tmp/in.mkv"),
            Path::new("/tmp/out.mkv"),
            &metadata,
            &plan,
        );
        let args = builder.build_args().expect("args");
        assert!(args.contains(&"hevc_amf".to_string()));
    }

    #[test]
    fn mp4_audio_transcode_uses_aac_profile() {
        let mut plan = plan_for(Encoder::H264X264);
        plan.container = "mp4".to_string();
        plan.audio = AudioStreamPlan::Transcode {
            codec: AudioCodec::Aac,
            bitrate_kbps: 192,
            channels: None,
        };
        plan.requested_codec = OutputCodec::H264;
        let metadata = metadata();
        let builder = FFmpegCommandBuilder::new(
            Path::new("/tmp/in.mkv"),
            Path::new("/tmp/out.mp4"),
            &metadata,
            &plan,
        );
        let args = builder.build_args().expect("args");
        assert!(args.contains(&"aac".to_string()));
        assert!(args.contains(&"aac_low".to_string()));
        assert!(args.contains(&"+faststart".to_string()));
    }

    #[test]
    fn subtitle_extract_command_maps_all_selected_streams() {
        let mut metadata = metadata();
        metadata.subtitle_streams = vec![
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
                codec_name: "hdmv_pgs_subtitle".to_string(),
                language: Some("jpn".to_string()),
                title: None,
                default: false,
                forced: false,
                burnable: false,
            },
        ];
        let mut plan = plan_for(Encoder::H264X264);
        plan.subtitles = SubtitleStreamPlan::Extract {
            outputs: vec![
                SidecarOutputPlan {
                    stream_index: 0,
                    codec: "srt".to_string(),
                    final_path: Path::new("/tmp/out.eng.srt").to_path_buf(),
                    temp_path: Path::new("/tmp/out.eng.srt.alchemist-part").to_path_buf(),
                },
                SidecarOutputPlan {
                    stream_index: 1,
                    codec: "ass".to_string(),
                    final_path: Path::new("/tmp/out.jpn.ass").to_path_buf(),
                    temp_path: Path::new("/tmp/out.jpn.ass.alchemist-part").to_path_buf(),
                },
            ],
        };
        let builder = FFmpegCommandBuilder::new(
            Path::new("/tmp/in.mkv"),
            Path::new("/tmp/out.mkv"),
            &metadata,
            &plan,
        );
        let args = builder
            .build_subtitle_extract_args()
            .expect("args")
            .expect("subtitle extract args");
        assert!(args.contains(&"0:s:0".to_string()));
        assert!(args.contains(&"0:s:1".to_string()));
        assert!(args.contains(&"srt".to_string()));
        assert!(args.contains(&"ass".to_string()));
        assert!(args.contains(&"/tmp/out.eng.srt.alchemist-part".to_string()));
        assert!(args.contains(&"/tmp/out.jpn.ass.alchemist-part".to_string()));
    }

    #[test]
    fn remux_command_uses_stream_copy_without_encoder_args() {
        let metadata = metadata();
        let mut plan = plan_for(Encoder::H264X264);
        plan.is_remux = true;
        plan.encoder = None;
        plan.backend = None;
        plan.rate_control = None;
        plan.encoder_preset = None;
        let builder = FFmpegCommandBuilder::new(
            Path::new("/tmp/in.mp4"),
            Path::new("/tmp/out.mkv"),
            &metadata,
            &plan,
        );
        let args = builder.build_args().expect("args");
        assert_eq!(
            args,
            vec![
                "-v".to_string(),
                "error".to_string(),
                "-i".to_string(),
                "/tmp/in.mp4".to_string(),
                "-c".to_string(),
                "copy".to_string(),
                "-map".to_string(),
                "0".to_string(),
                "-y".to_string(),
                "/tmp/out.mkv".to_string(),
            ]
        );
    }

    #[test]
    fn selected_audio_streams_map_only_requested_indices() {
        let metadata = metadata();
        let mut plan = plan_for(Encoder::H264X264);
        plan.audio_stream_indices = Some(vec![0, 2]);
        let builder = FFmpegCommandBuilder::new(
            Path::new("/tmp/in.mkv"),
            Path::new("/tmp/out.mkv"),
            &metadata,
            &plan,
        );
        let args = builder.build_args().expect("args");
        assert!(args.contains(&"0:a:0".to_string()));
        assert!(args.contains(&"0:a:2".to_string()));
        assert!(!args.contains(&"0:a?".to_string()));
    }

    #[test]
    fn encoder_capabilities_detect_with_runner_parses_video_and_audio_encoders() {
        let runner = FakeRunner {
            stdout: b"Encoders:\n V..... libx264 H.264\n A..... aac AAC\n V..... av1_qsv AV1\n"
                .to_vec(),
        };
        let capabilities = EncoderCapabilities::detect_with_runner(&runner).expect("capabilities");
        assert!(capabilities.has_video_encoder("libx264"));
        assert!(capabilities.has_video_encoder("av1_qsv"));
        assert!(capabilities.audio_encoders.contains("aac"));
    }

    #[test]
    fn hardware_accelerators_detect_with_runner_parses_hwaccels() {
        let runner = FakeRunner {
            stdout: b"Hardware acceleration methods:\nvaapi\nqsv\n".to_vec(),
        };
        let accelerators = HardwareAccelerators::detect_with_runner(&runner).expect("hwaccels");
        assert!(accelerators.available.contains("vaapi"));
        assert!(accelerators.available.contains("qsv"));
    }

    #[test]
    fn test_vmaf_score_text_parse() {
        let stderr = "Some log\nVMAF score: 93.2\nMore log";
        let vmaf = QualityScore::extract_vmaf_score_text(stderr).expect("expected vmaf");
        assert!((vmaf - 93.2).abs() < 0.01);
    }

    #[test]
    fn test_vmaf_score_json_parse() {
        let json = r#"{
            "pooled_metrics": {
                "vmaf": {
                    "mean": 87.65,
                    "harmonic_mean": 86.0
                }
            }
        }"#;
        let vmaf = QualityScore::extract_vmaf_score_json(json).expect("expected vmaf");
        assert!((vmaf - 87.65).abs() < 0.01);
    }
}
