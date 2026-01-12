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

#[derive(Clone, Debug)]
pub struct WatchPath {
    pub path: PathBuf,
    pub recursive: bool,
}

/// Filesystem watcher that auto-enqueues new media files
#[derive(Clone)]
pub struct FileWatcher {
    inner: Arc<std::sync::Mutex<Option<RecommendedWatcher>>>,
    tx: mpsc::UnboundedSender<PathBuf>,
}

impl FileWatcher {
    pub fn new(db: Arc<Db>) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();
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
                            let settings = match db_clone.get_file_settings().await {
                                Ok(s) => s,
                                Err(e) => {
                                    error!("Failed to fetch file settings, using defaults: {}", e);
                                    crate::db::FileSettings {
                                        id: 1,
                                        delete_source: false,
                                        output_extension: "mkv".to_string(),
                                        output_suffix: "-alchemist".to_string(),
                                        replace_strategy: "keep".to_string(),
                                    }
                                }
                            };
                            for path in pending.drain() {
                                if path.exists() {
                                    debug!("Auto-enqueuing new file: {:?}", path);
                                    let mtime = std::fs::metadata(&path)
                                        .and_then(|m| m.modified())
                                        .unwrap_or(std::time::SystemTime::now());
                                    let output_path = settings.output_path_for(&path);
                                    if output_path.exists() && !settings.should_replace_existing_output() {
                                        continue;
                                    }

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
    pub fn watch(&self, directories: &[WatchPath]) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| AlchemistError::Watch(format!("Watcher lock poisoned: {}", e)))?;

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
                                let _ = tx_clone.send(path);
                            }
                        }
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .map_err(|e| AlchemistError::Watch(format!("Failed to create watcher: {}", e)))?;

        // Watch all directories
        for watch_path in directories {
            info!(
                "Watching directory: {:?} (recursive: {})",
                watch_path.path, watch_path.recursive
            );
            let mode = if watch_path.recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            if let Err(e) = watcher.watch(&watch_path.path, mode) {
                error!("Failed to watch {:?}: {}", watch_path.path, e);
                // Continue trying others? Or fail?
                // Failing strictly is probably safer to alert user
                return Err(AlchemistError::Watch(format!(
                    "Failed to watch {:?}: {}",
                    watch_path.path, e
                )));
            }
        }

        info!("File watcher updated for {} directories", directories.len());

        *inner = Some(watcher);
        Ok(())
    }
}
