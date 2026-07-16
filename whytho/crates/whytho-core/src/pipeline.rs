use std::path::PathBuf;

use crate::config::{BackendKind, WhyThoConfig};
use crate::error::WhyThoError;
use crate::file_ops::FileOperationPlan;
use crate::media::MediaInput;
use crate::probe;
use crate::report::PlanReport;

#[derive(Debug, Clone, PartialEq)]
pub struct PlanRequest {
    pub input: MediaInput,
    pub config: WhyThoConfig,
    pub output_path: Option<PathBuf>,
}

impl PlanRequest {
    pub fn new(input: MediaInput) -> Self {
        Self {
            input,
            config: WhyThoConfig::default(),
            output_path: None,
        }
    }
}

pub fn draft_plan(request: PlanRequest) -> PlanReport {
    PlanReport::scaffold(request.input, request.config.preset)
}

pub fn plan(request: PlanRequest) -> Result<PlanReport, WhyThoError> {
    let probe_result = probe::probe(request.input.path())?;

    let output_path = FileOperationPlan::resolve_output(
        request.input.path(),
        request.config.file_operation,
        request.output_path.as_deref(),
    );

    let file_plan = FileOperationPlan {
        mode: request.config.file_operation,
        input: request.input.path().to_path_buf(),
        output: output_path,
        purge_partial_on_cancel: true,
    };

    file_plan.validate()?;

    let chosen_backend = BackendKind::Cpu;

    let mut notes = Vec::new();

    for stream in &probe_result.streams {
        use crate::media::VideoCodec;
        use crate::probe::DetectedCodec;

        if let crate::probe::StreamKind::Video(_) = &stream.kind {
            let source_codec = match stream.codec {
                DetectedCodec::H264 => Some(VideoCodec::H264),
                DetectedCodec::Hevc => Some(VideoCodec::Hevc),
                DetectedCodec::Av1 => Some(VideoCodec::Av1),
                _ => None,
            };
            if source_codec == Some(request.config.default_video_codec) {
                notes.push(format!(
                    "source video is already {} — re-encode may be unnecessary",
                    request.config.default_video_codec
                ));
            } else if let Some(src) = source_codec {
                notes.push(format!(
                    "source video is {src} — will re-encode to {}",
                    request.config.default_video_codec
                ));
            }
        }
    }

    let audio_tracks: Vec<_> = probe_result
        .streams
        .iter()
        .filter(|s| matches!(s.kind, crate::probe::StreamKind::Audio(_)))
        .collect();
    if audio_tracks.len() > 1 {
        notes.push(format!("{} audio tracks found", audio_tracks.len()));
    }

    let subtitle_tracks: Vec<_> = probe_result
        .streams
        .iter()
        .filter(|s| matches!(s.kind, crate::probe::StreamKind::Subtitle(_)))
        .collect();
    if !subtitle_tracks.is_empty() {
        notes.push(format!(
            "{} subtitle track(s) found — will preserve",
            subtitle_tracks.len()
        ));
    }

    Ok(PlanReport::planned(
        request.input,
        request.config,
        file_plan,
        chosen_backend,
        probe_result,
        notes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::Preset;
    use crate::report::ReportStatus;

    #[test]
    fn draft_plan_returns_scaffold_report() {
        let report = draft_plan(PlanRequest::new(MediaInput::new("movie.mkv")));
        assert_eq!(report.status, ReportStatus::Scaffolded);
        assert_eq!(report.preset, Preset::Av1Balanced);
        assert_eq!(report.input.path().to_string_lossy(), "movie.mkv");
    }

    #[test]
    fn plan_with_missing_input_returns_error() {
        let request = PlanRequest {
            input: MediaInput::new("nonexistent.mkv"),
            config: WhyThoConfig::default(),
            output_path: Some(PathBuf::from("out.mkv")),
        };
        let result = plan(request);
        assert!(result.is_err());
    }

    #[test]
    fn plan_with_unsupported_extension_returns_error() {
        let request = PlanRequest {
            input: MediaInput::new("Cargo.toml"),
            config: WhyThoConfig::default(),
            output_path: Some(PathBuf::from("out.mkv")),
        };
        let result = plan(request);
        assert!(result.is_err());
    }
}
