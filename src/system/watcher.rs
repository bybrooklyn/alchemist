//! Filesystem watcher module for auto-enqueuing new media files
//!
//! Uses the `notify` crate to watch configured directories for new files.

use crate::db::Db;
use crate::error::{AlchemistError, Result};
use crate::media::scanner::Scanner;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Filesystem watcher that auto-enqueues new media files
pub struct FileWatcher {
    directories: Vec<PathBuf>,
    db: Arc<Db>,
    debounce_ms: u64,
}

impl FileWatcher {
    pub fn new(directories: Vec<PathBuf>, db: Arc<Db>) -> Self {
        Self {
            directories,
            db,
            debounce_ms: 1000, // 1 second debounce
        }
    }

    /// Start watching directories for new files
    pub async fn start(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<PathBuf>(100);
        let scanner = Scanner::new();
        let extensions: HashSet<String> = scanner
            .extensions
            .iter()
            .map(|s| s.to_lowercase())
            .collect();

        // Create the watcher
        let tx_clone = tx.clone();
        let extensions_clone = extensions.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: std::result::Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    for path in event.paths {
                        // Check if it's a media file
                        if let Some(ext) = path.extension() {
                            if extensions_clone.contains(&ext.to_string_lossy().to_lowercase()) {
                                let _ = tx_clone.blocking_send(path);
                            }
                        }
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .map_err(|e| AlchemistError::Watch(format!("Failed to create watcher: {}", e)))?;

        // Watch all directories
        for dir in &self.directories {
            info!("Watching directory: {:?}", dir);
            watcher
                .watch(dir, RecursiveMode::Recursive)
                .map_err(|e| AlchemistError::Watch(format!("Failed to watch {:?}: {}", dir, e)))?;
        }

        info!(
            "File watcher started for {} directories",
            self.directories.len()
        );

        // Debounce and process events
        let db = self.db.clone();
        let debounce_ms = self.debounce_ms;

        tokio::spawn(async move {
            let mut pending: HashSet<PathBuf> = HashSet::new();
            let mut last_process = std::time::Instant::now();

            loop {
                tokio::select! {
                    Some(path) = rx.recv() => {
                        pending.insert(path);
                    }
                    _ = tokio::time::sleep(Duration::from_millis(debounce_ms)) => {
                        if !pending.is_empty() && last_process.elapsed().as_millis() >= debounce_ms as u128 {
                            for path in pending.drain() {
                                if path.exists() {
                                    debug!("Auto-enqueuing new file: {:?}", path);
                                    let mtime = std::fs::metadata(&path)
                                        .and_then(|m| m.modified())
                                        .unwrap_or(std::time::SystemTime::now());

                                    let mut output_path = path.clone();
                                    output_path.set_extension("av1.mkv");

                                    if let Err(e) = db.enqueue_job(&path, &output_path, mtime).await {
                                        error!("Failed to auto-enqueue {:?}: {}", path, e);
                                    } else {
                                        info!("Auto-enqueued: {:?}", path);
                                    }
                                }
                            }
                            last_process = std::time::Instant::now();
                        }
                    }
                }
            }
        });

        // Keep watcher alive
        // In a real implementation, we'd store the watcher handle
        // For now, we leak it intentionally to keep it running
        std::mem::forget(watcher);
        warn!("File watcher task started (watcher handle leaked intentionally)");

        Ok(())
    }
}
