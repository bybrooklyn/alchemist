use thiserror::Error;

#[derive(Error, Debug)]
pub enum AlchemistError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Hardware detection failed: {0}")]
    Hardware(String),

    #[error("FFmpeg execution failed: {0}")]
    FFmpeg(String),

    #[error("FFmpeg not found or not executable")]
    FFmpegNotFound,

    #[error("Encoder not available: {0}")]
    EncoderUnavailable(String),

    #[error("Quality check failed: {0}")]
    QualityCheckFailed(String),

    #[error("Notification failed: {0}")]
    Notification(String),

    #[error("Watch error: {0}")]
    Watch(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Analyzer error: {0}")]
    Analyzer(String),

    #[error("Job cancelled")]
    Cancelled,

    #[error("Job paused")]
    Paused,

    #[error("Query timeout after {0}s: {1}")]
    QueryTimeout(u64, String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl AlchemistError {
    /// Stable, machine-readable code for this error. Every variant maps to a
    /// documented code so logs, API responses, and the UI can link operators to
    /// `docs_url()`. Keep in sync with `docs/content/errors.md`.
    pub fn code(&self) -> &'static str {
        match self {
            AlchemistError::Database(_) => "ERR_DATABASE",
            AlchemistError::Config(_) => "ERR_CONFIG",
            AlchemistError::Hardware(_) => "ERR_HARDWARE",
            AlchemistError::FFmpeg(_) => "ERR_FFMPEG",
            AlchemistError::FFmpegNotFound => "ERR_FFMPEG_NOT_FOUND",
            AlchemistError::EncoderUnavailable(_) => "ERR_ENCODER_UNAVAILABLE",
            AlchemistError::QualityCheckFailed(_) => "ERR_QUALITY_CHECK_FAILED",
            AlchemistError::Notification(_) => "ERR_NOTIFICATION",
            AlchemistError::Watch(_) => "ERR_WATCH",
            AlchemistError::Io(_) => "ERR_IO",
            AlchemistError::Analyzer(_) => "ERR_ANALYZER",
            AlchemistError::Cancelled => "ERR_CANCELLED",
            AlchemistError::Paused => "ERR_PAUSED",
            AlchemistError::QueryTimeout(_, _) => "ERR_QUERY_TIMEOUT",
            AlchemistError::Unknown(_) => "ERR_UNKNOWN",
        }
    }

    /// Canonical documentation link for this error's code.
    pub fn docs_url(&self) -> String {
        crate::explanations::docs_url_for_code(self.code())
    }

    /// Whether this error is worth a bounded automatic retry. Transient classes
    /// (transient IO, full disk that may clear, query timeouts) are retryable;
    /// deterministic classes (config, encoder-open, planner/analyzer faults,
    /// cancellation) are not — retrying them just repeats the same failure.
    pub fn is_retryable(&self) -> bool {
        match self {
            AlchemistError::Io(err) => is_retryable_io(err),
            AlchemistError::QueryTimeout(_, _) => true,
            AlchemistError::Database(_) => true,
            AlchemistError::FFmpeg(detail) => {
                // FFmpeg failures are deterministic unless they are a transient
                // resource condition (disk space that may be freed, OOM that may
                // clear under lower load). Encoder-open failures are never retried
                // here — they are handled by the one-time CPU fallback instead.
                let normalized = detail.to_ascii_lowercase();
                !crate::explanations::is_encoder_open_failure(detail)
                    && (normalized.contains("no space left on device")
                        || normalized.contains("cannot allocate memory")
                        || normalized.contains("resource temporarily unavailable"))
            }
            AlchemistError::Cancelled
            | AlchemistError::Paused
            | AlchemistError::Config(_)
            | AlchemistError::Hardware(_)
            | AlchemistError::FFmpegNotFound
            | AlchemistError::EncoderUnavailable(_)
            | AlchemistError::QualityCheckFailed(_)
            | AlchemistError::Notification(_)
            | AlchemistError::Watch(_)
            | AlchemistError::Analyzer(_)
            | AlchemistError::Unknown(_) => false,
        }
    }
}

fn is_retryable_io(err: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        err.kind(),
        ErrorKind::TimedOut
            | ErrorKind::Interrupted
            | ErrorKind::WouldBlock
            | ErrorKind::BrokenPipe
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
    )
}

pub type Result<T> = std::result::Result<T, AlchemistError>;
