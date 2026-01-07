use thiserror::Error;

#[derive(Error, Debug)]
pub enum AlchemistError {
    #[cfg(feature = "ssr")]
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

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, AlchemistError>;
