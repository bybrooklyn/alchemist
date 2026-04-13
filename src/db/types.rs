use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Analyzing,
    Encoding,
    Remuxing,
    Completed,
    Skipped,
    Failed,
    Cancelled,
    Resuming,
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            JobState::Queued => "queued",
            JobState::Analyzing => "analyzing",
            JobState::Encoding => "encoding",
            JobState::Remuxing => "remuxing",
            JobState::Completed => "completed",
            JobState::Skipped => "skipped",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
            JobState::Resuming => "resuming",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct JobStats {
    pub active: i64,
    pub queued: i64,
    pub completed: i64,
    pub failed: i64,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct DailySummaryStats {
    pub completed: i64,
    pub failed: i64,
    pub skipped: i64,
    pub bytes_saved: i64,
    pub top_failure_reasons: Vec<String>,
    pub top_skip_reasons: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LogEntry {
    pub id: i64,
    pub level: String,
    pub job_id: Option<i64>,
    pub message: String,
    pub created_at: String, // SQLite datetime as string
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Job {
    pub id: i64,
    pub input_path: String,
    pub output_path: String,
    pub status: JobState,
    pub decision_reason: Option<String>,
    pub priority: i32,
    pub progress: f64,
    pub attempt_count: i32,
    pub vmaf_score: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub input_metadata_json: Option<String>,
}

impl Job {
    pub fn input_metadata(&self) -> Option<crate::media::pipeline::MediaMetadata> {
        self.input_metadata_json
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            JobState::Encoding | JobState::Analyzing | JobState::Remuxing | JobState::Resuming
        )
    }

    pub fn can_retry(&self) -> bool {
        matches!(self.status, JobState::Failed | JobState::Cancelled)
    }

    pub fn status_class(&self) -> &'static str {
        match self.status {
            JobState::Completed => "badge-green",
            JobState::Encoding | JobState::Remuxing | JobState::Resuming => "badge-yellow",
            JobState::Analyzing => "badge-blue",
            JobState::Failed | JobState::Cancelled => "badge-red",
            _ => "badge-gray",
        }
    }

    pub fn progress_fixed(&self) -> String {
        format!("{:.1}", self.progress)
    }

    pub fn vmaf_fixed(&self) -> String {
        self.vmaf_score
            .map(|s| format!("{:.1}", s))
            .unwrap_or_else(|| "N/A".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct JobWithHealthIssueRow {
    pub id: i64,
    pub input_path: String,
    pub output_path: String,
    pub status: JobState,
    pub decision_reason: Option<String>,
    pub priority: i32,
    pub progress: f64,
    pub attempt_count: i32,
    pub vmaf_score: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub input_metadata_json: Option<String>,
    pub health_issues: String,
}

impl JobWithHealthIssueRow {
    pub fn into_parts(self) -> (Job, String) {
        (
            Job {
                id: self.id,
                input_path: self.input_path,
                output_path: self.output_path,
                status: self.status,
                decision_reason: self.decision_reason,
                priority: self.priority,
                progress: self.progress,
                attempt_count: self.attempt_count,
                vmaf_score: self.vmaf_score,
                created_at: self.created_at,
                updated_at: self.updated_at,
                input_metadata_json: self.input_metadata_json,
            },
            self.health_issues,
        )
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DuplicateCandidate {
    pub id: i64,
    pub input_path: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct WatchDir {
    pub id: i64,
    pub path: String,
    pub is_recursive: bool,
    pub profile_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct LibraryProfile {
    pub id: i64,
    pub name: String,
    pub preset: String,
    pub codec: String,
    pub quality_profile: String,
    pub hdr_mode: String,
    pub audio_mode: String,
    pub crf_override: Option<i32>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewLibraryProfile {
    pub name: String,
    pub preset: String,
    pub codec: String,
    pub quality_profile: String,
    pub hdr_mode: String,
    pub audio_mode: String,
    pub crf_override: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct JobFilterQuery {
    pub limit: i64,
    pub offset: i64,
    pub statuses: Option<Vec<JobState>>,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub sort_desc: bool,
    pub archived: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct NotificationTarget {
    pub id: i64,
    pub name: String,
    pub target_type: String,
    pub config_json: String,
    pub events: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct ConversionJob {
    pub id: i64,
    pub upload_path: String,
    pub output_path: Option<String>,
    pub mode: String,
    pub settings_json: String,
    pub probe_json: Option<String>,
    pub linked_job_id: Option<i64>,
    pub status: String,
    pub expires_at: String,
    pub downloaded_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct ScheduleWindow {
    pub id: i64,
    pub start_time: String,
    pub end_time: String,
    pub days_of_week: String, // as JSON string
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct FileSettings {
    pub id: i64,
    pub delete_source: bool,
    pub output_extension: String,
    pub output_suffix: String,
    pub replace_strategy: String,
    pub output_root: Option<String>,
}

impl FileSettings {
    pub fn output_path_for(&self, input_path: &Path) -> PathBuf {
        self.output_path_for_source(input_path, None)
    }

    pub fn output_path_for_source(&self, input_path: &Path, source_root: Option<&Path>) -> PathBuf {
        let mut output_path = self.output_base_path(input_path, source_root);
        let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
        let extension = self.output_extension.trim_start_matches('.');
        let suffix = self.output_suffix.as_str();

        let mut filename = String::new();
        filename.push_str(&stem);
        filename.push_str(suffix);
        if !extension.is_empty() {
            filename.push('.');
            filename.push_str(extension);
        }
        if filename.is_empty() {
            filename.push_str("output");
        }
        output_path.set_file_name(filename);

        if output_path == input_path {
            let safe_suffix = if suffix.is_empty() {
                "-alchemist".to_string()
            } else {
                format!("{}-alchemist", suffix)
            };
            let mut safe_name = String::new();
            safe_name.push_str(&stem);
            safe_name.push_str(&safe_suffix);
            if !extension.is_empty() {
                safe_name.push('.');
                safe_name.push_str(extension);
            }
            output_path.set_file_name(safe_name);
        }

        output_path
    }

    fn output_base_path(&self, input_path: &Path, source_root: Option<&Path>) -> PathBuf {
        let Some(output_root) = self
            .output_root
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        else {
            return input_path.to_path_buf();
        };

        let Some(root) = source_root else {
            return input_path.to_path_buf();
        };

        let Ok(relative_path) = input_path.strip_prefix(root) else {
            return input_path.to_path_buf();
        };

        let mut output_path = PathBuf::from(output_root);
        if let Some(parent) = relative_path.parent() {
            output_path.push(parent);
        }
        output_path.push(relative_path.file_name().unwrap_or_default());
        output_path
    }

    pub fn should_replace_existing_output(&self) -> bool {
        let strategy = self.replace_strategy.trim();
        strategy.eq_ignore_ascii_case("replace") || strategy.eq_ignore_ascii_case("overwrite")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct AggregatedStats {
    pub total_jobs: i64,
    pub completed_jobs: i64,
    pub total_input_size: i64,
    pub total_output_size: i64,
    pub avg_vmaf: Option<f64>,
    pub total_encode_time_seconds: f64,
}

impl AggregatedStats {
    pub fn total_savings_gb(&self) -> f64 {
        self.total_input_size.saturating_sub(self.total_output_size) as f64 / 1_073_741_824.0
    }

    pub fn total_input_gb(&self) -> f64 {
        self.total_input_size as f64 / 1_073_741_824.0
    }

    pub fn avg_reduction_percentage(&self) -> f64 {
        if self.total_input_size == 0 {
            0.0
        } else {
            (1.0 - (self.total_output_size as f64 / self.total_input_size as f64)) * 100.0
        }
    }

    pub fn total_time_hours(&self) -> f64 {
        self.total_encode_time_seconds / 3600.0
    }

    pub fn total_savings_fixed(&self) -> String {
        format!("{:.1}", self.total_savings_gb())
    }

    pub fn total_input_fixed(&self) -> String {
        format!("{:.1}", self.total_input_gb())
    }

    pub fn efficiency_fixed(&self) -> String {
        format!("{:.1}", self.avg_reduction_percentage())
    }

    pub fn time_fixed(&self) -> String {
        format!("{:.1}", self.total_time_hours())
    }

    pub fn avg_vmaf_fixed(&self) -> String {
        self.avg_vmaf
            .map(|v| format!("{:.1}", v))
            .unwrap_or_else(|| "N/A".to_string())
    }
}

/// Daily aggregated statistics for time-series charts
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DailyStats {
    pub date: String,
    pub jobs_completed: i64,
    pub bytes_saved: i64,
    pub total_input_bytes: i64,
    pub total_output_bytes: i64,
}

/// Detailed per-job encoding statistics
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct DetailedEncodeStats {
    pub job_id: i64,
    pub input_path: String,
    pub input_size_bytes: i64,
    pub output_size_bytes: i64,
    pub compression_ratio: f64,
    pub encode_time_seconds: f64,
    pub encode_speed: f64,
    pub avg_bitrate_kbps: f64,
    pub vmaf_score: Option<f64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct EncodeAttempt {
    pub id: i64,
    pub job_id: i64,
    pub attempt_number: i32,
    pub started_at: Option<String>,
    pub finished_at: String,
    pub outcome: String,
    pub failure_code: Option<String>,
    pub failure_summary: Option<String>,
    pub input_size_bytes: Option<i64>,
    pub output_size_bytes: Option<i64>,
    pub encode_time_seconds: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct EncodeAttemptInput {
    pub job_id: i64,
    pub attempt_number: i32,
    pub started_at: Option<String>,
    pub outcome: String,
    pub failure_code: Option<String>,
    pub failure_summary: Option<String>,
    pub input_size_bytes: Option<i64>,
    pub output_size_bytes: Option<i64>,
    pub encode_time_seconds: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct EncodeStatsInput {
    pub job_id: i64,
    pub input_size: u64,
    pub output_size: u64,
    pub compression_ratio: f64,
    pub encode_time: f64,
    pub encode_speed: f64,
    pub avg_bitrate: f64,
    pub vmaf_score: Option<f64>,
    pub output_codec: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodecSavings {
    pub codec: String,
    pub bytes_saved: i64,
    pub job_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DailySavings {
    pub date: String,
    pub bytes_saved: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavingsSummary {
    pub total_input_bytes: i64,
    pub total_output_bytes: i64,
    pub total_bytes_saved: i64,
    pub savings_percent: f64,
    pub job_count: i64,
    pub savings_by_codec: Vec<CodecSavings>,
    pub savings_over_time: Vec<DailySavings>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HealthSummary {
    pub total_checked: i64,
    pub issues_found: i64,
    pub last_run: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Decision {
    pub id: i64,
    pub job_id: i64,
    pub action: String, // "encode", "skip", "reject"
    pub reason: String,
    pub reason_code: Option<String>,
    pub reason_payload_json: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct DecisionRecord {
    pub(crate) job_id: i64,
    pub(crate) action: String,
    pub(crate) reason: String,
    pub(crate) reason_payload_json: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct FailureExplanationRecord {
    pub(crate) legacy_summary: Option<String>,
    pub(crate) code: String,
    pub(crate) payload_json: String,
}

// Auth related structs
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Session {
    pub token: String,
    pub user_id: i64,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ApiTokenAccessLevel {
    ReadOnly,
    FullAccess,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct ApiToken {
    pub id: i64,
    pub name: String,
    pub access_level: ApiTokenAccessLevel,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiTokenRecord {
    pub id: i64,
    pub name: String,
    pub token_hash: String,
    pub access_level: ApiTokenAccessLevel,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_output_path_for_suffix() {
        let settings = FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "-alchemist".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: None,
        };
        let input = Path::new("video.mp4");
        let output = settings.output_path_for(input);
        assert_eq!(output, PathBuf::from("video-alchemist.mkv"));
    }

    #[test]
    fn test_output_path_avoids_inplace() {
        let settings = FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: None,
        };
        let input = Path::new("video.mkv");
        let output = settings.output_path_for(input);
        assert_ne!(output, input);
    }

    #[test]
    fn test_output_path_mirrors_source_root_under_output_root() {
        let settings = FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "-alchemist".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: Some("/encoded".to_string()),
        };
        let input = Path::new("/library/movies/action/video.mp4");
        let output = settings.output_path_for_source(input, Some(Path::new("/library")));
        assert_eq!(
            output,
            PathBuf::from("/encoded/movies/action/video-alchemist.mkv")
        );
    }

    #[test]
    fn test_output_path_falls_back_when_source_root_does_not_match() {
        let settings = FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "-alchemist".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: Some("/encoded".to_string()),
        };
        let input = Path::new("/library/movies/video.mp4");
        let output = settings.output_path_for_source(input, Some(Path::new("/other")));
        assert_eq!(output, PathBuf::from("/library/movies/video-alchemist.mkv"));
    }

    #[test]
    fn test_replace_strategy() {
        let mut settings = FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "-alchemist".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: None,
        };
        assert!(!settings.should_replace_existing_output());
        settings.replace_strategy = "replace".to_string();
        assert!(settings.should_replace_existing_output());
    }
}
