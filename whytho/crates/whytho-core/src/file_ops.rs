use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::error::WhyThoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileOperationMode {
    PreserveOriginal,
    KeepOriginal,
    ReplaceOriginal,
}

impl Default for FileOperationMode {
    fn default() -> Self {
        Self::PreserveOriginal
    }
}

impl fmt::Display for FileOperationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreserveOriginal => write!(f, "preserve-original"),
            Self::KeepOriginal => write!(f, "keep-original"),
            Self::ReplaceOriginal => write!(f, "replace-original"),
        }
    }
}

impl FromStr for FileOperationMode {
    type Err = WhyThoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "preserve-original" => Ok(Self::PreserveOriginal),
            "keep-original" => Ok(Self::KeepOriginal),
            "replace-original" => Ok(Self::ReplaceOriginal),
            _ => Err(WhyThoError::InvalidValue {
                field: "file-operation".into(),
                value: s.into(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOperationPlan {
    pub mode: FileOperationMode,
    pub input: PathBuf,
    pub output: PathBuf,
    pub purge_partial_on_cancel: bool,
}

impl FileOperationPlan {
    pub fn new(input: impl Into<PathBuf>, output: impl Into<PathBuf>) -> Self {
        Self {
            mode: FileOperationMode::default(),
            input: input.into(),
            output: output.into(),
            purge_partial_on_cancel: true,
        }
    }

    pub fn validate(&self) -> Result<(), WhyThoError> {
        if !self.input.exists() {
            return Err(WhyThoError::InputNotFound {
                path: self.input.clone(),
            });
        }
        if self.mode != FileOperationMode::ReplaceOriginal && self.input == self.output {
            return Err(WhyThoError::OutputConflictsInput {
                path: self.input.clone(),
            });
        }
        Ok(())
    }

    pub fn resolve_output(
        input: &Path,
        mode: FileOperationMode,
        explicit_output: Option<&Path>,
    ) -> PathBuf {
        if let Some(out) = explicit_output {
            return out.to_path_buf();
        }
        match mode {
            FileOperationMode::ReplaceOriginal => input.to_path_buf(),
            _ => {
                let stem = input.file_stem().unwrap_or_default();
                let ext = input.extension().unwrap_or_default();
                let parent = input.parent().unwrap_or(Path::new("."));
                parent.join(format!(
                    "{}.whytho.{}",
                    stem.to_string_lossy(),
                    ext.to_string_lossy()
                ))
            }
        }
    }
}
