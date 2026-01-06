use std::path::PathBuf;
use walkdir::WalkDir;
use tracing::{info, debug};

pub struct Scanner {
    extensions: Vec<String>,
}

impl Scanner {
    pub fn new() -> Self {
        Self {
            extensions: vec![
                "mp4".to_string(),
                "mkv".to_string(),
                "mov".to_string(),
                "avi".to_string(),
                "m4v".to_string(),
            ],
        }
    }

    pub fn scan(&self, directories: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for dir in directories {
            info!("Scanning directory: {:?}", dir);
            for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                        if self.extensions.contains(&ext.to_lowercase()) {
                            debug!("Found media file: {:?}", entry.path());
                            files.push(entry.path().to_path_buf());
                        }
                    }
                }
            }
        }
        info!("Found {} candidate media files", files.len());
        files
    }
}
