//! FFmpeg wrapper module for Alchemist
//! Provides typed FFmpeg commands, encoder detection, and structured progress parsing.

use crate::config::{CpuPreset, HdrMode, QualityProfile, TonemapAlgorithm};
use crate::error::{AlchemistError, Result};
use crate::system::hardware::{HardwareInfo, Vendor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use tracing::{debug, info, warn};

/// Available hardware accelerators detected from FFmpeg
#[derive(Debug, Clone, Default)]
pub struct HardwareAccelerators {
    pub available: HashSet<String>,
}

impl HardwareAccelerators {
    /// Detect available hardware accelerators via `ffmpeg -hwaccels`
    pub fn detect() -> Result<Self> {
        let output = Command::new("ffmpeg")
            .args(["-hide_banner", "-hwaccels"])
            .output()
            .map_err(|e| {
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

    pub fn has(&self, accel: &str) -> bool {
        self.available.contains(accel)
    }

    pub fn has_qsv(&self) -> bool {
        self.has("qsv")
    }

    pub fn has_cuda(&self) -> bool {
        self.has("cuda")
    }

    pub fn has_vaapi(&self) -> bool {
        self.has("vaapi")
    }

    pub fn has_videotoolbox(&self) -> bool {
        self.has("videotoolbox")
    }
}

/// Available encoders detected from FFmpeg
#[derive(Debug, Clone, Default)]
pub struct EncoderCapabilities {
    pub video_encoders: HashSet<String>,
    pub audio_encoders: HashSet<String>,
}

impl EncoderCapabilities {
    /// Detect available encoders via `ffmpeg -encoders`
    pub fn detect() -> Result<Self> {
        let output = Command::new("ffmpeg")
            .args(["-hide_banner", "-encoders"])
            .output()
            .map_err(|e| {
                AlchemistError::FFmpeg(format!("Failed to run ffmpeg -encoders: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut video_encoders = HashSet::new();
        let mut audio_encoders = HashSet::new();

        for line in stdout.lines() {
            let line = line.trim();
            if line.len() < 8 {
                continue;
            }

            // Format: " V..... encoder_name  Description"
            let flags = &line[..6];
            let rest = line[6..].trim();
            let encoder_name = rest.split_whitespace().next().unwrap_or("");

            if flags.starts_with(" V") {
                video_encoders.insert(encoder_name.to_string());
            } else if flags.starts_with(" A") {
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

    pub fn has_av1_qsv(&self) -> bool {
        self.has_video_encoder("av1_qsv")
    }

    pub fn has_av1_nvenc(&self) -> bool {
        self.has_video_encoder("av1_nvenc")
    }

    pub fn has_av1_vaapi(&self) -> bool {
        self.has_video_encoder("av1_vaapi")
    }

    pub fn has_av1_videotoolbox(&self) -> bool {
        self.has_video_encoder("av1_videotoolbox")
    }

    pub fn has_libsvtav1(&self) -> bool {
        self.has_video_encoder("libsvtav1")
    }

    pub fn has_hevc_videotoolbox(&self) -> bool {
        self.has_video_encoder("hevc_videotoolbox")
    }

    pub fn has_libx265(&self) -> bool {
        self.has_video_encoder("libx265")
    }

    pub fn has_libx264(&self) -> bool {
        self.has_video_encoder("libx264")
    }
}

// QualityProfile moved to config.rs

pub struct FFmpegCommandBuilder<'a> {
    input: &'a Path,
    output: &'a Path,
    hw_info: Option<&'a HardwareInfo>,
    profile: QualityProfile,
    cpu_preset: CpuPreset,
    target_codec: crate::config::OutputCodec,
    threads: usize,
    allow_fallback: bool,
    hdr_mode: HdrMode,
    tonemap_algorithm: TonemapAlgorithm,
    tonemap_peak: f32,
    tonemap_desat: f32,
    metadata: Option<&'a crate::media::pipeline::MediaMetadata>,
}

impl<'a> FFmpegCommandBuilder<'a> {
    pub fn new(input: &'a Path, output: &'a Path) -> Self {
        Self {
            input,
            output,
            hw_info: None,
            profile: QualityProfile::Balanced,
            cpu_preset: CpuPreset::Medium,
            target_codec: crate::config::OutputCodec::Av1,
            threads: 0,
            allow_fallback: true,
            hdr_mode: HdrMode::Preserve,
            tonemap_algorithm: TonemapAlgorithm::Hable,
            tonemap_peak: crate::config::default_tonemap_peak(),
            tonemap_desat: crate::config::default_tonemap_desat(),
            metadata: None,
        }
    }

    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    pub fn with_hardware(mut self, hw_info: Option<&'a HardwareInfo>) -> Self {
        self.hw_info = hw_info;
        self
    }

    pub fn with_profile(mut self, profile: QualityProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn with_cpu_preset(mut self, preset: CpuPreset) -> Self {
        self.cpu_preset = preset;
        self
    }

    pub fn with_codec(mut self, codec: crate::config::OutputCodec) -> Self {
        self.target_codec = codec;
        self
    }

    pub fn with_allow_fallback(mut self, allow_fallback: bool) -> Self {
        self.allow_fallback = allow_fallback;
        self
    }

    pub fn with_hdr_settings(
        mut self,
        hdr_mode: HdrMode,
        tonemap_algorithm: TonemapAlgorithm,
        tonemap_peak: f32,
        tonemap_desat: f32,
        metadata: Option<&'a crate::media::pipeline::MediaMetadata>,
    ) -> Self {
        self.hdr_mode = hdr_mode;
        self.tonemap_algorithm = tonemap_algorithm;
        self.tonemap_peak = tonemap_peak;
        self.tonemap_desat = tonemap_desat;
        self.metadata = metadata;
        self
    }

    pub fn build(self) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new("ffmpeg");
        cmd.arg("-hide_banner").arg("-y").arg("-i").arg(self.input);

        let caps = encoder_caps();
        if let Some(selection) = self.select_encoder(caps) {
            if selection.requested_codec != selection.effective_codec {
                info!(
                    "Requested codec: {} | Effective codec: {} ({}). Reason: {}",
                    selection.requested_codec.as_str(),
                    selection.effective_codec.as_str(),
                    selection.encoder,
                    selection.reason
                );
            } else {
                info!(
                    "Requested codec: {} | Effective codec: {} ({})",
                    selection.requested_codec.as_str(),
                    selection.effective_codec.as_str(),
                    selection.encoder
                );
            }
            self.apply_encoder(&mut cmd, &selection);
        } else {
            warn!(
                "No suitable encoder found for {} (fallbacks {}). Encoding may fail.",
                self.target_codec.as_str(),
                if self.allow_fallback { "allowed" } else { "disabled" }
            );
            self.apply_cpu_params(&mut cmd);
        }

        self.apply_hdr_settings(&mut cmd);

        if self.threads > 0 {
            cmd.arg("-threads").arg(self.threads.to_string());
        }

        cmd.arg("-c:a").arg("copy");
        cmd.arg("-c:s").arg("copy");
        cmd.arg(self.output);

        cmd
    }

    fn apply_encoder(&self, cmd: &mut tokio::process::Command, selection: &EncoderSelection) {
        match selection.encoder.as_str() {
            "av1_qsv" => {
                if let Some(hw) = self.hw_info {
                    if let Some(ref device_path) = hw.device_path {
                        cmd.arg("-init_hw_device")
                            .arg(format!("qsv=qsv:{}", device_path));
                        cmd.arg("-filter_hw_device").arg("qsv");
                    }
                }
                cmd.arg("-c:v").arg("av1_qsv");
                cmd.arg("-global_quality").arg(self.profile.qsv_quality());
                cmd.arg("-look_ahead").arg("1");
            }
            "hevc_qsv" => {
                if let Some(hw) = self.hw_info {
                    if let Some(ref device_path) = hw.device_path {
                        cmd.arg("-init_hw_device")
                            .arg(format!("qsv=qsv:{}", device_path));
                        cmd.arg("-filter_hw_device").arg("qsv");
                    }
                }
                cmd.arg("-c:v").arg("hevc_qsv");
                cmd.arg("-global_quality").arg(self.profile.qsv_quality());
                cmd.arg("-look_ahead").arg("1");
            }
            "av1_nvenc" => {
                cmd.arg("-c:v").arg("av1_nvenc");
                cmd.arg("-preset").arg(self.profile.nvenc_preset());
                cmd.arg("-cq").arg("25");
            }
            "hevc_nvenc" => {
                cmd.arg("-c:v").arg("hevc_nvenc");
                cmd.arg("-preset").arg(self.profile.nvenc_preset());
                cmd.arg("-cq").arg("25");
            }
            "h264_nvenc" => {
                cmd.arg("-c:v").arg("h264_nvenc");
                cmd.arg("-preset").arg(self.profile.nvenc_preset());
                cmd.arg("-cq").arg("23");
            }
            "av1_vaapi" => {
                if let Some(hw) = self.hw_info {
                    if let Some(ref device_path) = hw.device_path {
                        cmd.arg("-vaapi_device").arg(device_path);
                    }
                }
                cmd.arg("-c:v").arg("av1_vaapi");
            }
            "hevc_vaapi" => {
                if let Some(hw) = self.hw_info {
                    if let Some(ref device_path) = hw.device_path {
                        cmd.arg("-vaapi_device").arg(device_path);
                    }
                }
                cmd.arg("-c:v").arg("hevc_vaapi");
            }
            "h264_vaapi" => {
                if let Some(hw) = self.hw_info {
                    if let Some(ref device_path) = hw.device_path {
                        cmd.arg("-vaapi_device").arg(device_path);
                    }
                }
                cmd.arg("-c:v").arg("h264_vaapi");
            }
            "av1_amf" => {
                cmd.arg("-c:v").arg("av1_amf");
            }
            "hevc_amf" => {
                cmd.arg("-c:v").arg("hevc_amf");
            }
            "h264_amf" => {
                cmd.arg("-c:v").arg("h264_amf");
            }
            "av1_videotoolbox" => {
                cmd.arg("-c:v").arg("av1_videotoolbox");
                cmd.arg("-b:v").arg("0");
                cmd.arg("-q:v").arg(self.profile.videotoolbox_quality());
            }
            "hevc_videotoolbox" => {
                cmd.arg("-c:v").arg("hevc_videotoolbox");
                cmd.arg("-b:v").arg("0");
                cmd.arg("-q:v").arg(self.profile.videotoolbox_quality());
                cmd.arg("-tag:v").arg("hvc1");
            }
            "h264_videotoolbox" => {
                cmd.arg("-c:v").arg("h264_videotoolbox");
                cmd.arg("-b:v").arg("0");
                cmd.arg("-q:v").arg("65");
            }
            "libsvtav1" => {
                let (preset_str, crf_str) = self.cpu_preset.params();
                cmd.arg("-c:v").arg("libsvtav1");
                cmd.arg("-preset").arg(preset_str);
                cmd.arg("-crf").arg(crf_str);
            }
            "libaom-av1" => {
                cmd.arg("-c:v").arg("libaom-av1");
                cmd.arg("-crf").arg("32");
                cmd.arg("-cpu-used").arg("6");
            }
            "libx265" => {
                let preset = self.cpu_preset.as_str();
                let crf = match self.cpu_preset {
                    CpuPreset::Slow => "20",
                    CpuPreset::Medium => "24",
                    CpuPreset::Fast => "26",
                    CpuPreset::Faster => "28",
                };
                cmd.arg("-c:v").arg("libx265");
                cmd.arg("-preset").arg(preset);
                cmd.arg("-crf").arg(crf);
                cmd.arg("-tag:v").arg("hvc1");
            }
            "libx264" => {
                let preset = self.cpu_preset.as_str();
                let crf = match self.cpu_preset {
                    CpuPreset::Slow => "18",
                    CpuPreset::Medium => "21",
                    CpuPreset::Fast => "23",
                    CpuPreset::Faster => "25",
                };
                cmd.arg("-c:v").arg("libx264");
                cmd.arg("-preset").arg(preset);
                cmd.arg("-crf").arg(crf);
            }
            _ => self.apply_cpu_params(cmd),
        }
    }

    fn select_encoder(&self, caps: &EncoderCapabilities) -> Option<EncoderSelection> {
        let preferred = self.target_codec;

        let mut candidates: Vec<EncoderCandidate> = Vec::new();
        match preferred {
            crate::config::OutputCodec::Av1 => {
                self.push_av1_candidates(&mut candidates, caps);
                if self.allow_fallback {
                    self.push_hevc_candidates(&mut candidates, caps, "AV1 encoders unavailable");
                    self.push_h264_candidates(&mut candidates, caps, "HEVC encoders unavailable");
                }
            }
            crate::config::OutputCodec::Hevc => {
                self.push_hevc_candidates(&mut candidates, caps, "Preferred HEVC");
                if self.allow_fallback {
                    self.push_h264_candidates(&mut candidates, caps, "HEVC encoders unavailable");
                }
            }
            crate::config::OutputCodec::H264 => {
                self.push_h264_candidates(&mut candidates, caps, "Preferred H.264");
                if self.allow_fallback {
                    self.push_hevc_candidates(&mut candidates, caps, "H.264 encoders unavailable");
                }
            }
        }

        let selection = candidates.into_iter().find(|c| c.available);
        selection.map(|c| EncoderSelection {
            encoder: c.encoder.to_string(),
            requested_codec: preferred,
            effective_codec: c.effective_codec,
            reason: c.reason,
        })
    }

    fn push_av1_candidates(&self, out: &mut Vec<EncoderCandidate>, caps: &EncoderCapabilities) {
        match self.hw_info.map(|h| h.vendor) {
            Some(Vendor::Apple) => out.push(EncoderCandidate::new(
                "av1_videotoolbox",
                crate::config::OutputCodec::Av1,
                caps.has_video_encoder("av1_videotoolbox"),
                "Hardware AV1 (VideoToolbox)",
            )),
            Some(Vendor::Intel) => out.push(EncoderCandidate::new(
                "av1_qsv",
                crate::config::OutputCodec::Av1,
                caps.has_video_encoder("av1_qsv"),
                "Hardware AV1 (QSV)",
            )),
            Some(Vendor::Nvidia) => out.push(EncoderCandidate::new(
                "av1_nvenc",
                crate::config::OutputCodec::Av1,
                caps.has_video_encoder("av1_nvenc"),
                "Hardware AV1 (NVENC)",
            )),
            Some(Vendor::Amd) => out.push(EncoderCandidate::new(
                if cfg!(target_os = "windows") { "av1_amf" } else { "av1_vaapi" },
                crate::config::OutputCodec::Av1,
                caps.has_video_encoder(if cfg!(target_os = "windows") { "av1_amf" } else { "av1_vaapi" }),
                "Hardware AV1 (AMF/VAAPI)",
            )),
            _ => {}
        }
        out.push(EncoderCandidate::new(
            "libsvtav1",
            crate::config::OutputCodec::Av1,
            caps.has_libsvtav1(),
            "CPU AV1 (SVT-AV1)",
        ));
        out.push(EncoderCandidate::new(
            "libaom-av1",
            crate::config::OutputCodec::Av1,
            caps.has_video_encoder("libaom-av1"),
            "CPU AV1 (libaom)",
        ));
    }

    fn push_hevc_candidates(&self, out: &mut Vec<EncoderCandidate>, caps: &EncoderCapabilities, reason: &str) {
        match self.hw_info.map(|h| h.vendor) {
            Some(Vendor::Apple) => out.push(EncoderCandidate::new(
                "hevc_videotoolbox",
                crate::config::OutputCodec::Hevc,
                caps.has_hevc_videotoolbox(),
                reason,
            )),
            Some(Vendor::Intel) => out.push(EncoderCandidate::new(
                "hevc_qsv",
                crate::config::OutputCodec::Hevc,
                caps.has_video_encoder("hevc_qsv"),
                reason,
            )),
            Some(Vendor::Nvidia) => out.push(EncoderCandidate::new(
                "hevc_nvenc",
                crate::config::OutputCodec::Hevc,
                caps.has_video_encoder("hevc_nvenc"),
                reason,
            )),
            Some(Vendor::Amd) => out.push(EncoderCandidate::new(
                if cfg!(target_os = "windows") { "hevc_amf" } else { "hevc_vaapi" },
                crate::config::OutputCodec::Hevc,
                caps.has_video_encoder(if cfg!(target_os = "windows") { "hevc_amf" } else { "hevc_vaapi" }),
                reason,
            )),
            _ => {}
        }
        out.push(EncoderCandidate::new(
            "libx265",
            crate::config::OutputCodec::Hevc,
            caps.has_libx265(),
            reason,
        ));
    }

    fn push_h264_candidates(&self, out: &mut Vec<EncoderCandidate>, caps: &EncoderCapabilities, reason: &str) {
        match self.hw_info.map(|h| h.vendor) {
            Some(Vendor::Apple) => out.push(EncoderCandidate::new(
                "h264_videotoolbox",
                crate::config::OutputCodec::H264,
                caps.has_video_encoder("h264_videotoolbox"),
                reason,
            )),
            Some(Vendor::Intel) => out.push(EncoderCandidate::new(
                "h264_qsv",
                crate::config::OutputCodec::H264,
                caps.has_video_encoder("h264_qsv"),
                reason,
            )),
            Some(Vendor::Nvidia) => out.push(EncoderCandidate::new(
                "h264_nvenc",
                crate::config::OutputCodec::H264,
                caps.has_video_encoder("h264_nvenc"),
                reason,
            )),
            Some(Vendor::Amd) => out.push(EncoderCandidate::new(
                if cfg!(target_os = "windows") { "h264_amf" } else { "h264_vaapi" },
                crate::config::OutputCodec::H264,
                caps.has_video_encoder(if cfg!(target_os = "windows") { "h264_amf" } else { "h264_vaapi" }),
                reason,
            )),
            _ => {}
        }
        out.push(EncoderCandidate::new(
            "libx264",
            crate::config::OutputCodec::H264,
            caps.has_libx264(),
            reason,
        ));
    }

    fn apply_hdr_settings(&self, cmd: &mut tokio::process::Command) {
        let Some(metadata) = self.metadata else {
            return;
        };

        if metadata.is_hdr && self.hdr_mode == HdrMode::Tonemap {
            let filter = format!(
                "zscale=t=linear:npl={},tonemap=tonemap={}:desat={},zscale=t=bt709:m=bt709:r=tv,format=yuv420p",
                self.tonemap_peak,
                self.tonemap_algorithm.as_str(),
                self.tonemap_desat
            );
            cmd.arg("-vf").arg(filter);
            cmd.arg("-color_primaries").arg("bt709");
            cmd.arg("-color_trc").arg("bt709");
            cmd.arg("-colorspace").arg("bt709");
            cmd.arg("-color_range").arg("tv");
            return;
        }

        if let Some(ref primaries) = metadata.color_primaries {
            cmd.arg("-color_primaries").arg(primaries);
        }
        if let Some(ref transfer) = metadata.color_transfer {
            cmd.arg("-color_trc").arg(transfer);
        }
        if let Some(ref space) = metadata.color_space {
            cmd.arg("-colorspace").arg(space);
        }
        if let Some(ref range) = metadata.color_range {
            cmd.arg("-color_range").arg(range);
        }
    }

    fn apply_cpu_params(&self, cmd: &mut tokio::process::Command) {
        let caps = encoder_caps();
        match self.target_codec {
            crate::config::OutputCodec::Av1 => {
                if caps.has_libsvtav1() {
                    let (preset_str, crf_str) = self.cpu_preset.params();
                    cmd.arg("-c:v").arg("libsvtav1");
                    cmd.arg("-preset").arg(preset_str);
                    cmd.arg("-crf").arg(crf_str);
                } else if caps.has_libx265() {
                    warn!("libsvtav1 not available. Falling back to libx265.");
                    let preset = self.cpu_preset.as_str();
                    let crf = match self.cpu_preset {
                        CpuPreset::Slow => "20",
                        CpuPreset::Medium => "24",
                        CpuPreset::Fast => "26",
                        CpuPreset::Faster => "28",
                    };
                    cmd.arg("-c:v").arg("libx265");
                    cmd.arg("-preset").arg(preset);
                    cmd.arg("-crf").arg(crf);
                    cmd.arg("-tag:v").arg("hvc1");
                } else if caps.has_libx264() {
                    warn!("libsvtav1 not available. Falling back to libx264.");
                    let preset = self.cpu_preset.as_str();
                    let crf = match self.cpu_preset {
                        CpuPreset::Slow => "18",
                        CpuPreset::Medium => "21",
                        CpuPreset::Fast => "23",
                        CpuPreset::Faster => "25",
                    };
                    cmd.arg("-c:v").arg("libx264");
                    cmd.arg("-preset").arg(preset);
                    cmd.arg("-crf").arg(crf);
                } else {
                    warn!("No AV1/HEVC/H.264 CPU encoder available. Encoding will likely fail.");
                    let (preset_str, crf_str) = self.cpu_preset.params();
                    cmd.arg("-c:v").arg("libsvtav1");
                    cmd.arg("-preset").arg(preset_str);
                    cmd.arg("-crf").arg(crf_str);
                }
            }
            crate::config::OutputCodec::Hevc => {
                // For HEVC CPU, we use libx265
                // Map presets roughly:
                // slow -> slow
                // medium -> medium
                // fast -> fast
                // faster -> faster
                let preset = self.cpu_preset.as_str();
                // CRF mapping: libsvtav1 24-32 is roughly equivalent to x265 20-28
                // Let's use a simple offset or strict mapping
                let crf = match self.cpu_preset {
                    CpuPreset::Slow => "20",
                    CpuPreset::Medium => "24",
                    CpuPreset::Fast => "26",
                    CpuPreset::Faster => "28",
                };
                cmd.arg("-c:v").arg("libx265");
                cmd.arg("-preset").arg(preset);
                cmd.arg("-crf").arg(crf);
                cmd.arg("-tag:v").arg("hvc1"); // Apple compatibility
            }
            crate::config::OutputCodec::H264 => {
                let preset = self.cpu_preset.as_str();
                let crf = match self.cpu_preset {
                    CpuPreset::Slow => "18",
                    CpuPreset::Medium => "21",
                    CpuPreset::Fast => "23",
                    CpuPreset::Faster => "25",
                };
                cmd.arg("-c:v").arg("libx264");
                cmd.arg("-preset").arg(preset);
                cmd.arg("-crf").arg(crf);
            }
        }
    }
}

/// Parsed FFmpeg progress from stderr
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
    /// Parse a line of FFmpeg stderr for progress info
    pub fn parse_line(line: &str) -> Option<Self> {
        // FFmpeg progress can come in multiple formats.
        // We look for the standard "frame=... fps=... time=..." format.
        if !line.contains("time=") && !line.contains("out_time=") {
            return None;
        }

        let mut progress = Self::default();

        // Clean up the line (remove extra spaces)
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

    /// Parse time string (HH:MM:SS.ms) to seconds
    fn parse_time(s: &str) -> f64 {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return 0.0;
        }

        let hours: f64 = parts[0].parse().unwrap_or(0.0);
        let minutes: f64 = parts[1].parse().unwrap_or(0.0);
        let seconds: f64 = parts[2].parse().unwrap_or(0.0);

        hours * 3600.0 + minutes * 60.0 + seconds
    }

    /// Calculate percentage complete given total duration
    pub fn percentage(&self, total_duration: f64) -> f64 {
        if total_duration <= 0.0 {
            return 0.0;
        }
        (self.time_seconds / total_duration * 100.0).min(100.0)
    }
}

struct EncoderCandidate {
    encoder: &'static str,
    effective_codec: crate::config::OutputCodec,
    available: bool,
    reason: String,
}

impl EncoderCandidate {
    fn new(
        encoder: &'static str,
        effective_codec: crate::config::OutputCodec,
        available: bool,
        reason: &str,
    ) -> Self {
        Self {
            encoder,
            effective_codec,
            available,
            reason: reason.to_string(),
        }
    }
}

struct EncoderSelection {
    encoder: String,
    requested_codec: crate::config::OutputCodec,
    effective_codec: crate::config::OutputCodec,
    reason: String,
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

/// VMAF quality score result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScore {
    pub vmaf: Option<f64>,
    pub psnr: Option<f64>,
    pub ssim: Option<f64>,
}

impl QualityScore {
    /// Run VMAF quality comparison between original and encoded file
    pub fn compute(original: &Path, encoded: &Path) -> Result<Self> {
        info!("Computing quality metrics for {:?}", encoded);

        // Use FFmpeg's libvmaf filter
        let output = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-i",
                encoded.to_str().unwrap_or(""),
                "-i",
                original.to_str().unwrap_or(""),
                "-lavfi",
                "libvmaf=log_fmt=json:log_path=-",
                "-f",
                "null",
                "-",
            ])
            .output()
            .map_err(|e| AlchemistError::FFmpeg(format!("Failed to run VMAF: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AlchemistError::QualityCheckFailed(format!(
                "VMAF check failed: {}",
                stderr
            )));
        }

        // Parse VMAF score from output
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
            psnr: None, // Could add PSNR filter too
            ssim: None, // Could add SSIM filter too
        })
    }

    fn extract_vmaf_score_text(output: &str) -> Option<f64> {
        // Look for "VMAF score:" in the output
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
        vmaf
            .get("mean")
            .and_then(|v| v.as_f64())
            .or_else(|| vmaf.get("harmonic_mean").and_then(|v| v.as_f64()))
    }

    /// Check if quality is acceptable (VMAF >= threshold)
    pub fn is_acceptable(&self, min_vmaf: f64) -> bool {
        self.vmaf.map(|v| v >= min_vmaf).unwrap_or(true)
    }
}

/// Encode statistics for a completed job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodeStats {
    pub input_size_bytes: u64,
    pub output_size_bytes: u64,
    pub compression_ratio: f64,
    pub encode_time_seconds: f64,
    pub encode_speed: f64, // x speed
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

/// Verify FFmpeg is available and return version info
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

    #[test]
    fn test_progress_parsing() {
        let line =
            "frame=  100 fps=25.0 bitrate=1500kbps total_size=1000000 time=00:00:04.00 speed=1.5x";
        let progress = match FFmpegProgress::parse_line(line) {
            Some(progress) => progress,
            None => panic!("Expected progress parse to succeed"),
        };

        assert_eq!(progress.frame, 100);
        assert_eq!(progress.fps, 25.0);
        assert_eq!(progress.time, "00:00:04.00");
        assert!((progress.time_seconds - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_quality_profile_cpu_params() {
        let (preset, crf) = QualityProfile::Quality.cpu_params();
        assert_eq!(preset, "4");
        assert_eq!(crf, "24");
    }

    #[test]
    fn test_vmaf_score_text_parse() {
        let stderr = "Some log\nVMAF score: 93.2\nMore log";
        let vmaf = match QualityScore::extract_vmaf_score_text(stderr) {
            Some(vmaf) => vmaf,
            None => panic!("Expected VMAF score in text"),
        };
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
        let vmaf = match QualityScore::extract_vmaf_score_json(json) {
            Some(vmaf) => vmaf,
            None => panic!("Expected VMAF score in JSON"),
        };
        assert!((vmaf - 87.65).abs() < 0.01);
    }
}
