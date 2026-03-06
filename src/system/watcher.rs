//! Filesystem watcher module for auto-enqueuing new media files
//!
//! Uses the `notify` crate to watch configured directories for new files.

use crate::db::Db;
use crate::error::{AlchemistError, Result};
use crate::media::scanner::Scanner;
use notify::{
    event::{AccessKind, AccessMode, CreateKind, DataChange, ModifyKind, RenameMode},
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
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
                        if !pending.is_empty()
                            && last_process.elapsed().as_millis() >= u128::from(debounce_ms)
                        {
                            for path in pending.drain() {
                                if path.exists() {
                                    debug!("Auto-enqueuing new file: {:?}", path);
                                    let mtime = std::fs::metadata(&path)
                                        .and_then(|m| m.modified())
                                        .unwrap_or(std::time::SystemTime::now());
                                    let discovered = crate::media::pipeline::DiscoveredMedia {
                                        path: path.clone(),
                                        mtime,
                                    };
                                    match crate::media::pipeline::enqueue_discovered_with_db(&db_clone, discovered).await {
                                        Ok(true) => info!("Auto-enqueued: {:?}", path),
                                        Ok(false) => debug!("No queue update needed for {:?}", path),
                                        Err(e) => error!("Failed to auto-enqueue {:?}: {}", path, e),
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
            move |res: std::result::Result<Event, notify::Error>| match res {
                Ok(event) => {
                    if !should_enqueue_event(&event) {
                        return;
                    }

                    for path in event.paths {
                        if let Some(ext) = path.extension() {
                            if extensions.contains(&ext.to_string_lossy().to_lowercase()) {
                                let _ = tx_clone.send(path);
                            }
                        }
                    }
                }
                Err(err) => error!("Watcher event error: {}", err),
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

fn should_enqueue_event(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(CreateKind::File)
            | EventKind::Modify(ModifyKind::Data(DataChange::Content | DataChange::Size))
            | EventKind::Modify(ModifyKind::Name(RenameMode::To))
            | EventKind::Access(AccessKind::Close(AccessMode::Any | AccessMode::Write))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use std::path::Path;

    fn temp_db_path(prefix: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("{prefix}_{}.db", rand::random::<u64>()));
        path
    }

    fn temp_watch_dir(prefix: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("{prefix}_{}", rand::random::<u64>()));
        path
    }

    async fn wait_for_queued_jobs(db: &Db, expected: i64) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
        loop {
            let stats = db.get_job_stats().await?;
            if stats.queued == expected {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("timed out waiting for queued jobs: expected {expected}");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    #[test]
    fn should_enqueue_only_stable_file_events() {
        let create = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: Vec::new(),
            attrs: Default::default(),
        };
        assert!(should_enqueue_event(&create));

        let rename_to = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            paths: Vec::new(),
            attrs: Default::default(),
        };
        assert!(should_enqueue_event(&rename_to));

        let broad_modify = Event {
            kind: EventKind::Modify(ModifyKind::Any),
            paths: Vec::new(),
            attrs: Default::default(),
        };
        assert!(!should_enqueue_event(&broad_modify));
    }

    #[tokio::test]
    async fn watcher_enqueues_real_media_but_ignores_generated_outputs() -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_watcher_smoke");
        let watch_dir = temp_watch_dir("alchemist_watch_dir");
        std::fs::create_dir_all(&watch_dir)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let watcher = FileWatcher::new(db.clone());
        watcher.watch(&[WatchPath {
            path: watch_dir.clone(),
            recursive: false,
        }])?;

        let input_path = watch_dir.join("movie.mp4");
        std::fs::write(&input_path, b"source")?;
        wait_for_queued_jobs(db.as_ref(), 1).await?;

        let generated_output = watch_dir.join("movie-alchemist.mkv");
        std::fs::write(&generated_output, b"generated")?;
        tokio::time::sleep(Duration::from_secs(2)).await;

        let queued = db.get_jobs_by_status(crate::db::JobState::Queued).await?;
        assert_eq!(queued.len(), 1);
        assert_eq!(
            std::fs::canonicalize(&queued[0].input_path)?,
            std::fs::canonicalize(&input_path)?
        );
        assert!(db
            .get_job_by_input_path(generated_output.to_string_lossy().as_ref())
            .await?
            .is_none());
        assert!(Path::new(&queued[0].output_path).ends_with("movie-alchemist.mkv"));

        watcher.watch(&[])?;
        drop(db);
        cleanup_paths(&[watch_dir, db_path]);
        Ok(())
    }

    fn cleanup_paths(paths: &[PathBuf]) {
        for path in paths {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
