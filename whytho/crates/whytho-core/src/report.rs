use std::fmt;

use crate::config::{BackendKind, WhyThoConfig};
use crate::file_ops::FileOperationPlan;
use crate::media::MediaInput;
use crate::presets::Preset;
use crate::probe::ProbeResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportStatus {
    Scaffolded,
    Planned,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanReport {
    pub input: MediaInput,
    pub preset: Preset,
    pub status: ReportStatus,
    pub notes: Vec<String>,
    pub config: WhyThoConfig,
    pub file_plan: FileOperationPlan,
    pub chosen_backend: BackendKind,
    pub probe: Option<ProbeResult>,
}

impl PlanReport {
    pub fn scaffold(input: MediaInput, preset: Preset) -> Self {
        let config = preset.resolve();
        let file_plan = FileOperationPlan::new(input.path(), input.path());
        Self {
            input,
            preset,
            status: ReportStatus::Scaffolded,
            notes: vec![String::from("planning API scaffold only")],
            config,
            file_plan,
            chosen_backend: BackendKind::Cpu,
            probe: None,
        }
    }

    pub fn planned(
        input: MediaInput,
        config: WhyThoConfig,
        file_plan: FileOperationPlan,
        chosen_backend: BackendKind,
        probe: ProbeResult,
        notes: Vec<String>,
    ) -> Self {
        Self {
            preset: config.preset,
            input,
            status: ReportStatus::Planned,
            config,
            file_plan,
            chosen_backend,
            notes,
            probe: Some(probe),
        }
    }
}

impl fmt::Display for PlanReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Plan: {}", self.input.path().display())?;

        if let Some(ref probe) = self.probe {
            writeln!(f)?;
            writeln!(f, "  Source:")?;
            write!(f, "  ")?;
            write!(f, "{probe}")?;
        }

        writeln!(f)?;
        writeln!(f, "  Preset: {}", self.preset)?;
        writeln!(
            f,
            "  Backend: {} ({})",
            self.chosen_backend,
            if self.chosen_backend == BackendKind::Cpu {
                "software"
            } else {
                "hardware"
            }
        )?;
        writeln!(f, "  Video: {}", self.config.default_video_codec)?;
        writeln!(f, "  Audio: {}", self.config.default_audio_codec)?;
        writeln!(f, "  Container: {}", self.config.container)?;
        writeln!(f, "  Chunking: {}", self.config.chunking)?;
        writeln!(
            f,
            "  File op: {} -> {}",
            self.config.file_operation,
            self.file_plan.output.display()
        )?;
        writeln!(f, "  Verification: {}", self.config.verification)?;
        if !self.notes.is_empty() {
            writeln!(f)?;
            writeln!(f, "  Notes:")?;
            for note in &self.notes {
                writeln!(f, "    - {note}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{DetectedContainer, ProbeResult, StreamInfo, StreamKind, VideoStreamInfo};

    #[test]
    fn scaffold_report_display() {
        let report = PlanReport::scaffold(MediaInput::new("test.mkv"), Preset::Av1Balanced);
        let display = format!("{report}");
        assert!(display.contains("test.mkv"));
        assert!(display.contains("av1-balanced"));
        assert!(display.contains("scaffold"));
    }

    #[test]
    fn planned_report_display() {
        use std::collections::HashMap;
        use std::time::Duration;

        let probe = ProbeResult {
            path: "movie.mkv".into(),
            container: DetectedContainer {
                format: crate::media::ContainerFormat::Mkv,
                matroska_version: None,
                writing_app: None,
                muxing_app: None,
                duration: Some(Duration::from_secs(6123)),
            },
            size_bytes: 2_147_483_648,
            streams: vec![StreamInfo {
                index: 0,
                track_number: 1,
                kind: StreamKind::Video(VideoStreamInfo {
                    width: 1920,
                    height: 1080,
                    display_width: None,
                    display_height: None,
                    pixel_format: Some("YUV420".into()),
                    bit_depth: None,
                    frame_rate: Some(23.976),
                    bitrate: None,
                    profile: Some("High".into()),
                    level: Some("4.1".into()),
                    color_space: None,
                    color_range: None,
                    hdr: None,
                }),
                codec_id: "V_MPEG4/ISO/AVC".into(),
                codec: crate::probe::DetectedCodec::H264,
                language: None,
                name: None,
                default: true,
                forced: false,
                enabled: true,
                duration: None,
            }],
            chapters: Vec::new(),
            attachments: Vec::new(),
            tags: HashMap::new(),
        };

        let input = MediaInput::new("movie.mkv");
        let config = Preset::HevcCompatible.resolve();
        let file_plan = FileOperationPlan::new("movie.mkv", "movie.whytho.mkv");
        let report = PlanReport::planned(
            input,
            config,
            file_plan,
            BackendKind::Cpu,
            probe,
            vec!["source video is H.264 — will re-encode to hevc".into()],
        );
        let display = format!("{report}");
        assert!(display.contains("hevc-compatible"));
        assert!(display.contains("Source:"));
        assert!(display.contains("1920x1080"));
        assert!(display.contains("H.264"));
        assert!(display.contains("re-encode"));
    }
}
