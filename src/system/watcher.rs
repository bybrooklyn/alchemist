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
use std::sync::atomic::{AtomicBool, Ordering};
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
    agent: Option<Arc<crate::media::processor::Agent>>,
    analysis_pending: Arc<AtomicBool>,
}

impl FileWatcher {
    pub fn new(db: Arc<Db>, agent: Option<Arc<crate::media::processor::Agent>>) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<PendingEvent>();
        let poll_interval = Duration::from_secs(1);
        let db_clone = db.clone();
        let watcher = Self {
            inner: Arc::new(std::sync::Mutex::new(None)),
            tx,
            agent,
            analysis_pending: Arc::new(AtomicBool::new(false)),
        };
        let agent_clone = watcher.agent.clone();
        let analysis_pending_clone = watcher.analysis_pending.clone();

        // Process filesystem events after the target file has stabilized.
        tokio::spawn(async move {
            let mut pending: HashMap<PendingKey, PendingState> = HashMap::new();
            let mut interval = tokio::time::interval_at(
                tokio::time::Instant::now() + poll_interval,
                poll_interval,
            );
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

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

                        // The sweep stats every pending path, and the pending set is
                        // unbounded — a bulk import puts thousands of files in it. Doing
                        // that synchronously here blocked a runtime worker for the whole
                        // sweep, every second. Run it on the blocking pool instead: one
                        // handoff per tick regardless of how many files are pending.
                        let mut owned = std::mem::take(&mut pending);
                        let sweep = tokio::task::spawn_blocking(move || {
                            let mut ready = Vec::new();
                            owned.retain(|key, state| {
                                match state.poll(&key.path) {
                                    PendingPoll::Pending => true,
                                    PendingPoll::Gone => false,
                                    PendingPoll::Ready => {
                                        ready.push(key.clone());
                                        false
                                    }
                                }
                            });
                            (owned, ready)
                        }).await;

                        let ready = match sweep {
                            Ok((remaining, ready)) => {
                                pending = remaining;
                                ready
                            }
                            Err(err) => {
                                error!("Watcher stability sweep failed: {err}");
                                continue;
                            }
                        };

                        for key in ready {
                            if let Ok(metadata) = tokio::fs::metadata(&key.path).await {
                                debug!("Auto-enqueuing stable file: {:?}", key.path);
                                let mtime = metadata.modified().unwrap_or_else(|_| SystemTime::now());
                                let discovered = crate::media::pipeline::DiscoveredMedia {
                                    path: key.path.clone(),
                                    mtime,
                                    source_root: key.source_root.clone(),
                                };
                                match crate::media::pipeline::enqueue_discovered_with_db(&db_clone, discovered).await {
                                    Ok(true) => {
                                        info!("Auto-enqueued: {:?}", key.path);
                                        if let Some(agent) = &agent_clone {
                                            // Only spawn if no analysis pass is
                                            // already queued — coalesces bursts
                                            let already_pending =
                                                analysis_pending_clone
                                                    .swap(true, Ordering::SeqCst);
                                            if !already_pending {
                                                let agent = agent.clone();
                                                let flag = analysis_pending_clone.clone();
                                                tokio::spawn(async move {
                                                    // Clear the flag before starting so
                                                    // new arrivals during analysis still
                                                    // get their own pass
                                                    flag.store(false, Ordering::SeqCst);
                                                    agent.analyze_pending_jobs().await;
                                                });
                                            }
                                        }
                                    }
                                    Ok(false) => debug!("No queue update needed for {:?}", key.path),
                                    Err(e) => error!("Failed to auto-enqueue {:?}: {}", key.path, e),
                                }
                            }
                        }
                    }
                }
            }
        });

        watcher
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
        let watch_roots: Vec<WatchRoot> = directories
            .iter()
            .map(|watch| WatchRoot::new(watch.path.clone()))
            .collect();

        let mut watcher = RecommendedWatcher::new(
            move |res: std::result::Result<Event, notify::Error>| match res {
                Ok(event) => {
                    let Some(hint) = stability_hint_for_event(&event) else {
                        return;
                    };

                    for path in event.paths {
                        if let Some(ext) = path.extension()
                            && extensions.contains(&ext.to_string_lossy().to_lowercase())
                        {
                            // A path with a hidden component below its
                            // watch root (a scratch/resume directory,
                            // another tool's hidden staging dir, or a
                            // dotfile) is never real library media —
                            // skip it before it ever becomes a pending
                            // enqueue candidate. The root itself may
                            // legitimately live under a dot directory,
                            // so only components below it count. Root
                            // resolution matches both the raw and the
                            // canonicalized form, so a symlinked root,
                            // a bind mount, or (on macOS) /var vs
                            // /private/var still resolves instead of
                            // silently skipping the hidden check.
                            let (source_root, hidden) =
                                resolve_watch_root_and_hidden(&path, &watch_roots);
                            if hidden {
                                continue;
                            }
                            let _ = tx_clone.send(PendingEvent {
                                key: PendingKey { path, source_root },
                                hint,
                            });
                        }
                    }
                }
                Err(err) => error!("Watcher event error: {}", err),
            },
            NotifyConfig::default().with_poll_interval(Duration::from_secs(2)),
        )
        .map_err(|e| AlchemistError::Watch(format!("Failed to create watcher: {}", e)))?;

        // Watch all directories. A single bad path must not take down the
        // whole refresh (RG-23): install every watchable directory and
        // report the failures together, so the operator sees the bad path
        // instead of silently keeping a stale watch set.
        let mut failures = Vec::new();
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
                failures.push(format!("{:?}: {e}", watch_path.path));
            }
        }

        info!("File watcher updated for {} directories", directories.len());

        *inner = Some(watcher);
        if failures.is_empty() {
            Ok(())
        } else {
            Err(AlchemistError::Watch(format!(
                "Failed to watch {} directorie(s): {}",
                failures.len(),
                failures.join("; ")
            )))
        }
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

/// A configured watch root, plus its canonicalized form (best effort,
/// `None` if canonicalization fails — e.g. the root doesn't exist yet).
///
/// Event paths reported by the OS watcher don't always share the
/// configured root's exact spelling: a symlinked root, `..` in the
/// configured path, a bind/symlinked mount, or (notably on macOS) `/var`
/// vs the real `/private/var` all mean a literal-component prefix match
/// between the raw event path and the raw root can fail even though the
/// event genuinely belongs to that root.
struct WatchRoot {
    raw: PathBuf,
    canonical: Option<PathBuf>,
}

impl WatchRoot {
    fn new(raw: PathBuf) -> Self {
        let canonical = std::fs::canonicalize(&raw).ok();
        Self { raw, canonical }
    }
}

/// Resolve which configured root `path` falls under, and whether `path`
/// has a hidden component below that root.
///
/// Tries a cheap literal-component prefix match against each root's raw
/// (as-configured) form first, preferring the most specific (longest)
/// match when roots are nested — this is the common case and needs no
/// I/O. If none match, falls back to canonicalizing `path` and matching
/// that against each root's canonical form, so a root and an event path
/// that are really the same filesystem location under different
/// spellings still resolve, and hiddenness is judged against the true,
/// canonical relationship rather than silently skipped because the
/// literal prefix match failed. A raw match returns the raw configured
/// root; a canonical match returns the canonical root, so the event path
/// and returned source root always use the same spelling. This is required
/// by output-root mapping, which derives the relative output path with
/// `path.strip_prefix(source_root)`. If `path` matches no root in either
/// form, this returns `(None, false)` — nothing to relate it to, so nothing
/// to prune, matching the pre-existing behavior for paths outside every
/// configured root.
fn resolve_watch_root_and_hidden(path: &Path, roots: &[WatchRoot]) -> (Option<PathBuf>, bool) {
    let raw_match = roots
        .iter()
        .filter(|root| path.starts_with(&root.raw))
        .max_by_key(|root| root.raw.components().count());
    if let Some(root) = raw_match {
        return (
            Some(root.raw.clone()),
            crate::media::scanner::has_hidden_component(path, &root.raw),
        );
    }

    if let Ok(canonical_path) = std::fs::canonicalize(path) {
        let canonical_match = roots
            .iter()
            .filter(|root| {
                root.canonical
                    .as_deref()
                    .is_some_and(|canonical_root| canonical_path.starts_with(canonical_root))
            })
            .max_by_key(|root| {
                root.canonical
                    .as_ref()
                    .map(|c| c.components().count())
                    .unwrap_or(0)
            });
        if let Some(root) = canonical_match
            && let Some(canonical_root) = &root.canonical
        {
            return (
                Some(canonical_root.clone()),
                crate::media::scanner::has_hidden_component(&canonical_path, canonical_root),
            );
        }
    }

    (None, false)
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

    /// Deterministic, platform-independent regression test for the root-
    /// resolution defect itself: the configured root is the symlink's own
    /// (unresolved) spelling, while the path being resolved is already
    /// the canonical/real path — exactly what macOS FSEvents reports for
    /// a watch registered on a symlinked root (e.g. `/var` -> the real
    /// `/private/var`), and what a literal-prefix match alone cannot
    /// relate to that root.
    #[cfg(unix)]
    #[test]
    fn resolve_watch_root_and_hidden_matches_through_a_symlinked_root() -> anyhow::Result<()> {
        let real_dir = temp_watch_dir("resolve_symlink_real");
        std::fs::create_dir_all(&real_dir)?;
        let symlink_root = temp_watch_dir("resolve_symlink_link");
        std::os::unix::fs::symlink(&real_dir, &symlink_root)?;

        let visible = real_dir.join("movie.mkv");
        std::fs::write(&visible, b"data")?;
        let hidden_dir = real_dir.join(".resume-dir");
        std::fs::create_dir_all(&hidden_dir)?;
        let scratch = hidden_dir.join("segment.mkv");
        std::fs::write(&scratch, b"data")?;

        // As configured, the root is the symlink itself.
        let roots = vec![WatchRoot::new(symlink_root.clone())];

        // Simulate an OS watcher reporting the event path already
        // resolved through the symlink (the canonical, real path).
        let canonical_root = std::fs::canonicalize(&real_dir)?;
        let canonical_visible = std::fs::canonicalize(&visible)?;
        let canonical_scratch = std::fs::canonicalize(&scratch)?;

        let (root, hidden) = resolve_watch_root_and_hidden(&canonical_visible, &roots);
        assert_eq!(
            root.as_deref(),
            Some(canonical_root.as_path()),
            "a canonical event path must be paired with the matching canonical source root"
        );
        assert!(!hidden, "the visible file must not be treated as hidden");

        let output_root = temp_watch_dir("resolve_symlink_output");
        let settings = crate::db::FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "-alchemist".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: Some(output_root.to_string_lossy().to_string()),
        };
        assert_eq!(
            settings.output_path_for_source(&canonical_visible, root.as_deref()),
            output_root.join("movie-alchemist.mkv"),
            "canonical fallback must preserve output-root mapping"
        );

        let (root, hidden) = resolve_watch_root_and_hidden(&canonical_scratch, &roots);
        assert_eq!(root.as_deref(), Some(canonical_root.as_path()));
        assert!(
            hidden,
            "a file inside a hidden directory reached via the real path must still be recognized as hidden"
        );

        let _ = std::fs::remove_file(&symlink_root);
        let _ = std::fs::remove_dir_all(&real_dir);
        Ok(())
    }

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
        let watcher = FileWatcher::new(db.clone(), None);
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

    /// Regression test for the watcher half of the hidden-scratch-directory
    /// bug: a file written inside a dot-prefixed resume-style scratch
    /// directory (Alchemist's own naming, see `resume_temp_dir_for` in
    /// `media::pipeline`) under the watch root must never be enqueued,
    /// while a real, visible file still is.
    #[tokio::test]
    async fn watcher_enqueues_real_media_but_ignores_hidden_scratch_directories()
    -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_watcher_hidden");
        let watch_dir = temp_watch_dir("alchemist_watch_hidden");
        std::fs::create_dir_all(&watch_dir)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let watcher = FileWatcher::new(db.clone(), None);
        watcher.watch(&[WatchPath {
            path: watch_dir.clone(),
            recursive: true,
        }])?;

        let input_path = watch_dir.join("movie.mp4");
        std::fs::write(&input_path, b"source")?;
        wait_for_queued_jobs(db.as_ref(), 1).await?;

        let resume_dir = watch_dir.join(".other-movie.mkv.alchemist.resume-7");
        std::fs::create_dir_all(&resume_dir)?;
        // Give the recursive watcher time to register a watch on the new
        // subdirectory before writing into it, so the file-creation event
        // isn't missed by a startup race unrelated to the hidden-component
        // check under test.
        tokio::time::sleep(Duration::from_millis(500)).await;
        let scratch_segment = resume_dir.join("segment-00001.mkv");
        std::fs::write(&scratch_segment, b"scratch")?;
        tokio::time::sleep(Duration::from_secs(6)).await;

        let queued = db.get_jobs_by_status(crate::db::JobState::Queued).await?;
        assert_eq!(queued.len(), 1, "only the visible file should be queued");
        assert_eq!(
            std::fs::canonicalize(&queued[0].input_path)?,
            std::fs::canonicalize(&input_path)?
        );
        assert!(
            db.get_job_by_input_path(scratch_segment.to_string_lossy().as_ref())
                .await?
                .is_none(),
            "a file inside a hidden scratch directory must never be enqueued"
        );

        watcher.watch(&[])?;
        drop(db);
        cleanup_paths(&[watch_dir, db_path]);
        Ok(())
    }

    /// Regression test for a symlinked watch root: the OS watcher reports
    /// event paths resolved through the symlink (or already canonical),
    /// which a literal-prefix match against the raw, as-configured root
    /// can fail to recognize at all — silently skipping the hidden-
    /// component check along with it. A file inside a hidden resume-style
    /// scratch directory under the symlinked root must still never be
    /// enqueued, while a real, visible file still is and retains output-root
    /// mapping even if the event path is reported canonically.
    #[cfg(unix)]
    #[tokio::test]
    async fn watcher_through_symlinked_root_ignores_hidden_scratch_directories()
    -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_watcher_symlink");
        let real_dir = temp_watch_dir("alchemist_watch_symlink_real");
        let output_root = temp_watch_dir("alchemist_watch_symlink_output");
        std::fs::create_dir_all(&real_dir)?;
        std::fs::create_dir_all(&output_root)?;
        let symlink_root = temp_watch_dir("alchemist_watch_symlink_link");
        std::os::unix::fs::symlink(&real_dir, &symlink_root)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        db.update_file_settings(
            false,
            "mkv",
            "-alchemist",
            "keep",
            Some(output_root.to_string_lossy().as_ref()),
        )
        .await?;
        let watcher = FileWatcher::new(db.clone(), None);
        watcher.watch(&[WatchPath {
            path: symlink_root.clone(),
            recursive: true,
        }])?;

        let input_path = symlink_root.join("movie.mp4");
        std::fs::write(&input_path, b"source")?;
        wait_for_queued_jobs(db.as_ref(), 1).await?;

        let resume_dir = symlink_root.join(".other-movie.mkv.alchemist.resume-7");
        std::fs::create_dir_all(&resume_dir)?;
        // Give the recursive watcher time to register a watch on the new
        // subdirectory before writing into it (see the sibling
        // non-symlinked test above for why).
        tokio::time::sleep(Duration::from_millis(500)).await;
        let scratch_segment = resume_dir.join("segment-00001.mkv");
        std::fs::write(&scratch_segment, b"scratch")?;
        tokio::time::sleep(Duration::from_secs(6)).await;

        // Compare through canonicalize, not raw path equality: the OS
        // watcher may report the event path already resolved through the
        // symlink, so the DB's recorded `input_path` need not match the
        // symlink-relative path byte-for-byte.
        let queued = db.get_jobs_by_status(crate::db::JobState::Queued).await?;
        assert_eq!(
            queued.len(),
            1,
            "only the visible file should be queued, found: {:?}",
            queued.iter().map(|j| &j.input_path).collect::<Vec<_>>()
        );
        assert_eq!(
            std::fs::canonicalize(&queued[0].input_path)?,
            std::fs::canonicalize(&input_path)?
        );
        assert_eq!(
            Path::new(&queued[0].output_path),
            output_root.join("movie-alchemist.mkv"),
            "symlinked-root watcher events must retain output-root mapping"
        );

        watcher.watch(&[])?;
        drop(db);
        cleanup_paths(&[symlink_root, real_dir, output_root, db_path]);
        Ok(())
    }

    #[tokio::test]
    async fn watcher_waits_for_file_to_stabilize_before_queueing() -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_watcher_stability");
        let watch_dir = temp_watch_dir("alchemist_watch_stability");
        std::fs::create_dir_all(&watch_dir)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let watcher = FileWatcher::new(db.clone(), None);
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
        let watcher = FileWatcher::new(db.clone(), None);
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
        let watcher = FileWatcher::new(db.clone(), None);
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

    /// RG-23: one unwatched directory must not take down the refresh — the
    /// good directory stays watched (files queue) while the error names the
    /// bad one.
    #[tokio::test]
    async fn watcher_refresh_tolerates_one_bad_directory() -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_watch_partial");
        let watch_dir = temp_watch_dir("alchemist_watch_partial_good");
        std::fs::create_dir_all(&watch_dir)?;
        let missing = temp_watch_dir("alchemist_watch_partial_missing");

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let watcher = FileWatcher::new(db.clone(), None);
        let result = watcher.watch(&[
            WatchPath {
                path: watch_dir.clone(),
                recursive: false,
            },
            WatchPath {
                path: missing.clone(),
                recursive: false,
            },
        ]);
        assert!(
            result.is_err(),
            "refresh with a missing directory must still report the failure"
        );
        let message = match result {
            Err(err) => err.to_string(),
            Ok(()) => panic!("expected watch() to fail with a missing directory"),
        };
        assert!(
            message.contains(&missing.to_string_lossy().into_owned()),
            "error must name the bad directory: {message}"
        );

        let input_path = watch_dir.join("survivor.mp4");
        std::fs::write(&input_path, b"source")?;
        wait_for_queued_jobs(db.as_ref(), 1).await?;

        watcher.watch(&[])?;
        drop(db);
        cleanup_paths(&[watch_dir, db_path]);
        Ok(())
    }
}
