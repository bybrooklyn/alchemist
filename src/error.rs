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

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Analyzer error: {0}")]
    Analyzer(String),

    #[error("Job cancelled")]
    Cancelled,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, AlchemistError>;
