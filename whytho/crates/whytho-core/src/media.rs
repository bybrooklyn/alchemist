use std::path::{Path, PathBuf};

// The codec and container enums now live in the dependency-light `whytho-types` contract crate
// so codec crates can use them without depending on this app-policy crate. Re-exported here to
// preserve the `whytho_core::media::{VideoCodec, AudioCodec, ContainerFormat}` path.
pub use whytho_types::{AudioCodec, ContainerFormat, VideoCodec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInput {
    path: PathBuf,
}

impl MediaInput {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
