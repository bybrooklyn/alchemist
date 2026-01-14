use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tracing::{debug, error, info};
use walkdir::WalkDir;

use crate::media::pipeline::DiscoveredMedia;

pub struct Scanner {
    pub extensions: Vec<String>,
}

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

    pub fn scan(&self, directories: Vec<PathBuf>) -> Vec<DiscoveredMedia> {
        let entries = directories.into_iter().map(|dir| (dir, true)).collect();
        self.scan_with_recursion(entries)
    }

    pub fn scan_with_recursion(&self, directories: Vec<(PathBuf, bool)>) -> Vec<DiscoveredMedia> {
        let files = Arc::new(Mutex::new(Vec::new()));

        directories.into_par_iter().for_each(|(dir, recursive)| {
            info!("Scanning directory: {:?} (recursive: {})", dir, recursive);
            let mut local_files = Vec::new();
            let walker = if recursive {
                WalkDir::new(dir)
            } else {
                WalkDir::new(dir).max_depth(1)
            };
            for entry in walker.into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                        if self.extensions.contains(&ext.to_lowercase()) {
                            debug!("Found media file: {:?}", entry.path());
                            let mtime = entry
                                .metadata()
                                .map(|m| m.modified().unwrap_or(SystemTime::UNIX_EPOCH))
                                .unwrap_or(SystemTime::UNIX_EPOCH);
                            local_files.push(DiscoveredMedia {
                                path: entry.path().to_path_buf(),
                                mtime,
                            });
                        }
                    }
                }
            }
            match files.lock() {
                Ok(mut guard) => guard.extend(local_files),
                Err(e) => error!("Failed to lock scan results: {}", e),
            }
        });

        let mut final_files = match files.lock() {
            Ok(guard) => guard.clone(),
            Err(e) => {
                error!("Failed to lock scan results for finalize: {}", e);
                Vec::new()
            }
        };
        // Deterministic ordering
        final_files.sort_by(|a, b| a.path.cmp(&b.path));

        info!("Found {} candidate media files", final_files.len());
        final_files
    }
}
