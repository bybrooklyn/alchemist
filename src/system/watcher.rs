//! Filesystem watcher module for auto-enqueuing new media files
//!
//! Uses the `notify` crate to watch configured directories for new files.

use crate::config::Config as AppConfig;
use crate::db::Db;
use crate::error::{AlchemistError, Result};
use crate::media::scanner::Scanner;
use notify::{
    Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode, CreateKind, DataChange, ModifyKind, RenameMode},
};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

#[derive(Clone, Debug)]
pub struct WatchPath {
    pub path: PathBuf,
    pub recursive: bool,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct PendingKey {
    path: PathBuf,
    source_root: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StabilityHint {
    Standard,
    QuickSettle,
}

#[derive(Clone, Debug)]
struct PendingEvent {
    key: PendingKey,
    hint: StabilityHint,
}

#[derive(Clone, Debug, Default)]
struct PendingState {
    last_size: Option<u64>,
    last_mtime: Option<SystemTime>,
    stable_polls: u8,
    quick_settle: bool,
}

enum PendingPoll {
    Pending,
    Ready,
    Gone,
}

impl PendingState {
    fn note_hint(&mut self, hint: StabilityHint) {
        if matches!(hint, StabilityHint::QuickSettle) {
            self.quick_settle = true;
        }
    }

    fn poll(&mut self, path: &Path) -> PendingPoll {
        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return PendingPoll::Gone,
            Err(_) => return PendingPoll::Pending,
        };
        let modified = metadata.modified().ok();
        let size = metadata.len();
        let unchanged = self.last_size == Some(size) && self.last_mtime == modified;

        if unchanged {
            self.stable_polls = self.stable_polls.saturating_add(1);
        } else {
            self.stable_polls = 0;
        }

        self.last_size = Some(size);
        self.last_mtime = modified;

        let required_stable_polls = if self.quick_settle { 1 } else { 2 };
        if self.stable_polls >= required_stable_polls {
            PendingPoll::Ready
        } else {
            PendingPoll::Pending
        }
    }
}

/// Filesystem watcher that auto-enqueues new media files
#[derive(Clone)]
pub struct FileWatcher {
    inner: Arc<std::sync::Mutex<Option<RecommendedWatcher>>>,
    tx: mpsc::UnboundedSender<PendingEvent>,
}

impl FileWatcher {
    pub fn new(db: Arc<Db>) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<PendingEvent>();
        let poll_interval = Duration::from_secs(1);
        let db_clone = db.clone();

        // Process filesystem events after the target file has stabilized.
        tokio::spawn(async move {
            let mut pending: HashMap<PendingKey, PendingState> = HashMap::new();
            let mut interval = tokio::time::interval(poll_interval);

            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        pending
                            .entry(event.key)
                            .or_default()
                            .note_hint(event.hint);
                    }
                    _ = interval.tick() => {
                        if pending.is_empty() {
                            continue;
                        }

                        let mut ready = Vec::new();
                        pending.retain(|key, state| {
                            match state.poll(&key.path) {
                                PendingPoll::Pending => true,
                                PendingPoll::Gone => false,
                                PendingPoll::Ready => {
                                    ready.push(key.clone());
                                    false
                                }
                            }
                        });

                        for key in ready {
                            if key.path.exists() {
                                debug!("Auto-enqueuing stable file: {:?}", key.path);
                                let mtime = std::fs::metadata(&key.path)
                                    .and_then(|m| m.modified())
                                    .unwrap_or(SystemTime::now());
                                let discovered = crate::media::pipeline::DiscoveredMedia {
                                    path: key.path.clone(),
                                    mtime,
                                    source_root: key.source_root.clone(),
                                };
                                match crate::media::pipeline::enqueue_discovered_with_db(&db_clone, discovered).await {
                                    Ok(true) => info!("Auto-enqueued: {:?}", key.path),
                                    Ok(false) => debug!("No queue update needed for {:?}", key.path),
                                    Err(e) => error!("Failed to auto-enqueue {:?}: {}", key.path, e),
                                }
                            }
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
        let watch_roots: Vec<PathBuf> =
            directories.iter().map(|watch| watch.path.clone()).collect();

        let mut watcher = RecommendedWatcher::new(
            move |res: std::result::Result<Event, notify::Error>| match res {
                Ok(event) => {
                    let Some(hint) = stability_hint_for_event(&event) else {
                        return;
                    };

                    for path in event.paths {
                        if let Some(ext) = path.extension() {
                            if extensions.contains(&ext.to_string_lossy().to_lowercase()) {
                                let source_root = resolve_source_root(&path, &watch_roots);
                                let _ = tx_clone.send(PendingEvent {
                                    key: PendingKey { path, source_root },
                                    hint,
                                });
                            }
                        }
                    }
                }
                Err(err) => error!("Watcher event error: {}", err),
            },
            NotifyConfig::default().with_poll_interval(Duration::from_secs(2)),
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

pub async fn resolve_watch_paths(
    db: &Db,
    config: &AppConfig,
    setup_required: bool,
) -> Result<Vec<WatchPath>> {
    if setup_required {
        return Ok(Vec::new());
    }

    let mut watch_dirs: HashMap<PathBuf, bool> = HashMap::new();

    if config.scanner.watch_enabled {
        for dir in &config.scanner.directories {
            watch_dirs.insert(PathBuf::from(dir), true);
        }
    }

    for dir in db.get_watch_dirs().await? {
        watch_dirs
            .entry(PathBuf::from(dir.path))
            .and_modify(|recursive| *recursive |= dir.is_recursive)
            .or_insert(dir.is_recursive);
    }

    let mut all_dirs: Vec<WatchPath> = watch_dirs
        .into_iter()
        .map(|(path, recursive)| WatchPath { path, recursive })
        .collect();
    all_dirs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(all_dirs)
}

pub async fn refresh_from_sources(
    file_watcher: &FileWatcher,
    db: &Db,
    config: &AppConfig,
    setup_required: bool,
) -> Result<()> {
    let dirs = resolve_watch_paths(db, config, setup_required).await?;
    file_watcher.watch(&dirs)
}

fn resolve_source_root(path: &Path, watch_roots: &[PathBuf]) -> Option<PathBuf> {
    watch_roots
        .iter()
        .filter(|root| path.starts_with(root))
        .max_by_key(|root| root.components().count())
        .cloned()
}

fn stability_hint_for_event(event: &Event) -> Option<StabilityHint> {
    match event.kind {
        EventKind::Create(CreateKind::File)
        | EventKind::Modify(ModifyKind::Data(DataChange::Content | DataChange::Size)) => {
            Some(StabilityHint::Standard)
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To))
        | EventKind::Access(AccessKind::Close(AccessMode::Any | AccessMode::Write)) => {
            Some(StabilityHint::QuickSettle)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config as AppConfig;
    use crate::db::Db;
    use std::io::Write;
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
        let deadline = tokio::time::Instant::now() + Duration::from_secs(12);
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
    fn classifies_file_events_by_stability_hint() {
        let create = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: Vec::new(),
            attrs: Default::default(),
        };
        assert_eq!(
            stability_hint_for_event(&create),
            Some(StabilityHint::Standard)
        );

        let rename_to = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            paths: Vec::new(),
            attrs: Default::default(),
        };
        assert_eq!(
            stability_hint_for_event(&rename_to),
            Some(StabilityHint::QuickSettle)
        );

        let broad_modify = Event {
            kind: EventKind::Modify(ModifyKind::Any),
            paths: Vec::new(),
            attrs: Default::default(),
        };
        assert_eq!(stability_hint_for_event(&broad_modify), None);
    }

    #[tokio::test]
    async fn resolve_watch_paths_respects_watch_enabled() -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_watch_resolve");
        let watch_dir = temp_watch_dir("alchemist_watch_toggle");
        let db_dir = temp_watch_dir("alchemist_watch_db");
        std::fs::create_dir_all(&watch_dir)?;
        std::fs::create_dir_all(&db_dir)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        db.add_watch_dir(db_dir.to_string_lossy().as_ref(), false)
            .await?;

        let mut config = AppConfig::default();
        config.scanner.directories = vec![watch_dir.to_string_lossy().to_string()];
        config.scanner.watch_enabled = false;

        let disabled = resolve_watch_paths(db.as_ref(), &config, false).await?;
        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled[0].path, db_dir);

        config.scanner.watch_enabled = true;
        let enabled = resolve_watch_paths(db.as_ref(), &config, false).await?;
        assert_eq!(enabled.len(), 2);
        assert!(enabled.iter().any(|entry| entry.path == watch_dir));

        cleanup_paths(&[watch_dir, db_dir, db_path]);
        Ok(())
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
        assert!(
            db.get_job_by_input_path(generated_output.to_string_lossy().as_ref())
                .await?
                .is_none()
        );
        assert!(Path::new(&queued[0].output_path).ends_with("movie-alchemist.mkv"));

        watcher.watch(&[])?;
        drop(db);
        cleanup_paths(&[watch_dir, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn watcher_waits_for_file_to_stabilize_before_queueing() -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_watcher_stability");
        let watch_dir = temp_watch_dir("alchemist_watch_stability");
        std::fs::create_dir_all(&watch_dir)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let watcher = FileWatcher::new(db.clone());
        watcher.watch(&[WatchPath {
            path: watch_dir.clone(),
            recursive: false,
        }])?;

        let input_path = watch_dir.join("feature.mp4");
        {
            let mut file = std::fs::File::create(&input_path)?;
            file.write_all(b"partial")?;
            file.flush()?;
        }

        // Access close/write events can enable quick-settle on Linux, so
        // keep this assertion comfortably before the earliest ready poll.
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_eq!(db.get_job_stats().await?.queued, 0);

        {
            let mut file = std::fs::OpenOptions::new().append(true).open(&input_path)?;
            file.write_all(b"-final")?;
            file.flush()?;
        }

        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_eq!(db.get_job_stats().await?.queued, 0);

        wait_for_queued_jobs(db.as_ref(), 1).await?;

        watcher.watch(&[])?;
        cleanup_paths(&[watch_dir, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn watcher_deduplicates_repeated_modify_events() -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_watcher_dedupe");
        let watch_dir = temp_watch_dir("alchemist_watch_dedupe");
        std::fs::create_dir_all(&watch_dir)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let watcher = FileWatcher::new(db.clone());
        watcher.watch(&[WatchPath {
            path: watch_dir.clone(),
            recursive: false,
        }])?;

        let input_path = watch_dir.join("episode.mp4");
        std::fs::write(&input_path, b"one")?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        {
            let mut file = std::fs::OpenOptions::new().append(true).open(&input_path)?;
            file.write_all(b"two")?;
            file.flush()?;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        {
            let mut file = std::fs::OpenOptions::new().append(true).open(&input_path)?;
            file.write_all(b"three")?;
            file.flush()?;
        }

        wait_for_queued_jobs(db.as_ref(), 1).await?;
        let queued = db.get_jobs_by_status(crate::db::JobState::Queued).await?;
        assert_eq!(queued.len(), 1);
        assert_eq!(
            std::fs::canonicalize(&queued[0].input_path)?,
            std::fs::canonicalize(&input_path)?
        );

        watcher.watch(&[])?;
        cleanup_paths(&[watch_dir, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn watcher_enqueues_files_renamed_into_place() -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_watcher_rename");
        let watch_dir = temp_watch_dir("alchemist_watch_rename");
        let staging_root = temp_watch_dir("alchemist_watch_staging");
        std::fs::create_dir_all(&watch_dir)?;
        std::fs::create_dir_all(&staging_root)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let watcher = FileWatcher::new(db.clone());
        watcher.watch(&[WatchPath {
            path: watch_dir.clone(),
            recursive: false,
        }])?;

        let staging_path = staging_root.join("movie.tmp");
        let input_path = watch_dir.join("movie.mp4");
        std::fs::write(&staging_path, b"source")?;
        std::fs::rename(&staging_path, &input_path)?;
        watcher.tx.send(PendingEvent {
            key: PendingKey {
                path: input_path.clone(),
                source_root: Some(watch_dir.clone()),
            },
            hint: StabilityHint::QuickSettle,
        })?;

        wait_for_queued_jobs(db.as_ref(), 1).await?;
        let queued = db.get_jobs_by_status(crate::db::JobState::Queued).await?;
        assert_eq!(queued.len(), 1);
        assert_eq!(
            std::fs::canonicalize(&queued[0].input_path)?,
            std::fs::canonicalize(&input_path)?
        );

        watcher.watch(&[])?;
        cleanup_paths(&[watch_dir, staging_root, db_path]);
        Ok(())
    }

    fn cleanup_paths(paths: &[PathBuf]) {
        for path in paths {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
