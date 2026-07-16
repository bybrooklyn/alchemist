use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum WhyThoError {
    #[error("input file not found: {path}")]
    InputNotFound { path: PathBuf },

    #[error("output path conflicts with input: {path}")]
    OutputConflictsInput { path: PathBuf },

    #[error("invalid preset: {name}")]
    InvalidPreset { name: String },

    #[error("invalid {field}: {value}")]
    InvalidValue { field: String, value: String },

    #[error("config: {0}")]
    Config(String),

    #[error("config file not found: {path}")]
    ConfigNotFound { path: PathBuf },

    #[error("probe failed for {}: {source}", path.display())]
    ProbeFailed {
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("unsupported container format for {}: {detected}", path.display())]
    UnsupportedContainer { path: PathBuf, detected: String },

    #[error("corrupt stream {track}: {reason}")]
    CorruptStream { track: u32, reason: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),
}
