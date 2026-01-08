#[cfg(feature = "ssr")]
use rayon::prelude::*;
use std::path::PathBuf;
#[cfg(feature = "ssr")]
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tracing::{debug, info};
#[cfg(feature = "ssr")]
use walkdir::WalkDir;

#[derive(Clone)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub mtime: SystemTime,
}

#[cfg(feature = "ssr")]
pub struct Scanner {
    pub extensions: Vec<String>,
}

#[cfg(feature = "ssr")]
impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn scan(&self, directories: Vec<PathBuf>) -> Vec<ScannedFile> {
        let files = Arc::new(Mutex::new(Vec::new()));

        directories.into_par_iter().for_each(|dir| {
            info!("Scanning directory: {:?}", dir);
            let mut local_files = Vec::new();
            for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                        if self.extensions.contains(&ext.to_lowercase()) {
                            debug!("Found media file: {:?}", entry.path());
                            let mtime = entry
                                .metadata()
                                .map(|m| m.modified().unwrap_or(SystemTime::UNIX_EPOCH))
                                .unwrap_or(SystemTime::UNIX_EPOCH);
                            local_files.push(ScannedFile {
                                path: entry.path().to_path_buf(),
                                mtime,
                            });
                        }
                    }
                }
            }
            files.lock().unwrap().extend(local_files);
        });

        let mut final_files = files.lock().unwrap().clone();
        // Deterministic ordering
        final_files.sort_by(|a, b| a.path.cmp(&b.path));

        info!("Found {} candidate media files", final_files.len());
        final_files
    }
}
