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
use tracing::{debug, error, info};

/// Filesystem watcher that auto-enqueues new media files
#[derive(Clone)]
pub struct FileWatcher {
    inner: Arc<std::sync::Mutex<Option<RecommendedWatcher>>>,
    tx: mpsc::Sender<PathBuf>,
}

impl FileWatcher {
    pub fn new(db: Arc<Db>) -> Self {
        let (tx, mut rx) = mpsc::channel::<PathBuf>(100);
        let debounce_ms = 1000;
        let db_clone = db.clone();

        // Spawn key processing loop immediately
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

                                    if let Err(e) = db_clone.enqueue_job(&path, &output_path, mtime).await {
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

        Self {
            inner: Arc::new(std::sync::Mutex::new(None)),
            tx,
        }
    }

    /// Update watched directories
    pub fn watch(&self, directories: &[PathBuf]) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();

        // Stop existing watcher implicitly by dropping it (if we replace it)
        // Or explicitly unwatch? Dropping RecommendedWatcher stops it.

        if directories.is_empty() {
            *inner = None;
            info!("File watcher stopped (no directories configured)");
            return Ok(());
        }

        let scanner = Scanner::new();
        let extensions: HashSet<String> = scanner
            .extensions
            .iter()
            .map(|s| s.to_lowercase())
            .collect();

        // Create the watcher
        let tx_clone = self.tx.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: std::result::Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    for path in event.paths {
                        // Check if it's a media file
                        if let Some(ext) = path.extension() {
                            if extensions.contains(&ext.to_string_lossy().to_lowercase()) {
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
        for dir in directories {
            info!("Watching directory: {:?}", dir);
            if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
                error!("Failed to watch {:?}: {}", dir, e);
                // Continue trying others? Or fail?
                // Failing strictly is probably safer to alert user
                return Err(AlchemistError::Watch(format!(
                    "Failed to watch {:?}: {}",
                    dir, e
                )));
            }
        }

        info!("File watcher updated for {} directories", directories.len());

        *inner = Some(watcher);
        Ok(())
    }
}
