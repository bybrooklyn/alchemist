use crate::error::{AlchemistError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthIssueReport {
    pub category: HealthIssueCategory,
    pub summary: String,
    pub raw_output: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueCategory {
    CorruptFile,
    TruncatedFile,
    MissingKeyframe,
    CorruptAudio,
    CorruptVideo,
    PermissionError,
    Unknown,
}

/// Upper bound on the stored `raw_output`. A badly damaged file can make ffmpeg
/// emit thousands of decode-error lines even under `-v error`, and the whole
/// blob is persisted per health issue; cap it so one corrupt file cannot bloat
/// the database. The categorization below only ever inspects the leading lines.
const MAX_RAW_OUTPUT_CHARS: usize = 4096;

pub fn categorize_health_output(stderr: &str) -> HealthIssueReport {
    let trimmed = stderr.trim();
    let raw_output = if trimmed.chars().count() > MAX_RAW_OUTPUT_CHARS {
        // Deliberately not the word "truncated": the categorizer below matches
        // that string to detect a truncated *media file*, so using it here would
        // misclassify every oversized error dump as TruncatedFile.
        let head: String = trimmed.chars().take(MAX_RAW_OUTPUT_CHARS).collect();
        format!("{head}\n[output clipped]")
    } else {
        trimmed.to_string()
    };
    let normalized = raw_output.to_ascii_lowercase();

    let (category, summary) = if normalized.contains("moov atom not found")
        || normalized.contains("invalid data found when processing input")
    {
        (
            HealthIssueCategory::CorruptFile,
            "File appears to be corrupt or incomplete (missing MP4 header data).".to_string(),
        )
    } else if normalized.contains("truncated") {
        (
            HealthIssueCategory::TruncatedFile,
            "File is truncated — the end of the file is missing or damaged.".to_string(),
        )
    } else if normalized.contains("no keyframe") || normalized.contains("missing keyframe") {
        (
            HealthIssueCategory::MissingKeyframe,
            "No keyframe found — the file may not be seekable or could have playback issues."
                .to_string(),
        )
    } else if raw_output.contains("Error while decoding stream #0:1") {
        (
            HealthIssueCategory::CorruptAudio,
            "The audio track has errors and may sound distorted or cut out during playback."
                .to_string(),
        )
    } else if raw_output.contains("Error while decoding stream #0:0") {
        (
            HealthIssueCategory::CorruptVideo,
            "The video track has errors and may show visual glitches or freeze during playback."
                .to_string(),
        )
    } else if normalized.contains("permission denied") {
        (
            HealthIssueCategory::PermissionError,
            "Alchemist cannot read this file. Check the file permissions.".to_string(),
        )
    } else {
        let summary = raw_output
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("Unknown FFmpeg health check output")
            .chars()
            .take(200)
            .collect::<String>();
        (HealthIssueCategory::Unknown, summary)
    };

    HealthIssueReport {
        category,
        summary,
        raw_output,
    }
}

pub struct HealthChecker;

impl HealthChecker {
    pub async fn check_file(path: &Path) -> Result<Option<HealthIssueReport>> {
        let mut command = Command::new("ffmpeg");
        command
            .kill_on_drop(true)
            .args(["-v", "error", "-i"])
            .arg(path)
            .args(["-c", "copy", "-f", "null", "-"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let child = command.spawn().map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => AlchemistError::FFmpegNotFound,
            _ => AlchemistError::FFmpeg(format!(
                "Failed to start library health check for {}: {}",
                path.display(),
                err
            )),
        })?;

        let output = match timeout(Duration::from_secs(60), child.wait_with_output()).await {
            Ok(result) => result.map_err(AlchemistError::Io)?,
            Err(_) => {
                return Err(AlchemistError::FFmpeg(format!(
                    "Library health check timed out after 60 seconds for {}",
                    path.display()
                )));
            }
        };

        let stderr_output = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if output.status.success() && stderr_output.is_empty() {
            return Ok(None);
        }

        if !stderr_output.is_empty() {
            return Ok(Some(categorize_health_output(&stderr_output)));
        }

        Ok(Some(categorize_health_output(&format!(
            "ffmpeg exited with status {}",
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorize_corrupt_file_output() {
        let report = categorize_health_output("moov atom not found");
        assert_eq!(report.category, HealthIssueCategory::CorruptFile);
    }

    #[test]
    fn categorize_unknown_output_uses_first_line_summary() {
        let report = categorize_health_output("first line\nsecond line");
        assert_eq!(report.category, HealthIssueCategory::Unknown);
        assert_eq!(report.summary, "first line");
    }

    #[test]
    fn oversized_output_is_clipped_without_forcing_truncated_category() {
        let noisy = format!(
            "Error while decoding stream #0:0: garbage\n{}",
            "x".repeat(MAX_RAW_OUTPUT_CHARS * 2)
        );
        let report = categorize_health_output(&noisy);
        assert!(report.raw_output.chars().count() <= MAX_RAW_OUTPUT_CHARS + 32);
        assert!(report.raw_output.ends_with("[output clipped]"));
        // The clip marker must not steal the TruncatedFile classification.
        assert_eq!(report.category, HealthIssueCategory::CorruptVideo);
    }
}
