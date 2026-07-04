use crate::config::Config;
use crate::db::Db;
use crate::error::Result;
use crate::media::scanner::{PruneOptions, Scanner};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScanStatus {
    pub is_running: bool,
    pub files_found: usize,
    pub files_added: usize,
    pub current_folder: Option<String>,
}

pub struct LibraryScanner {
    db: Arc<Db>,
    config: Arc<RwLock<Config>>,
    status: Arc<Mutex<ScanStatus>>,
}

impl LibraryScanner {
    pub fn new(db: Arc<Db>, config: Arc<RwLock<Config>>) -> Self {
        Self {
            db,
            config,
            status: Arc::new(Mutex::new(ScanStatus {
                is_running: false,
                files_found: 0,
                files_added: 0,
                current_folder: None,
            })),
        }
    }

    pub async fn get_status(&self) -> ScanStatus {
        self.status.lock().await.clone()
    }

    pub async fn start_scan(&self) -> Result<()> {
        self.start_scan_with_options(false).await
    }

    /// PERF-3: `force_full = true` bypasses every shortcut. Probe cache rows
    /// under the scan target paths are deleted first so the analyzer can't
    /// reuse stale entries, and aggressive directory pruning is disabled for
    /// the duration of this scan regardless of the scanner config flag.
    /// `last_scanned_at` is still updated after a successful run so the next
    /// non-force scan benefits.
    pub async fn start_scan_with_options(&self, force_full: bool) -> Result<()> {
        let mut status = self.status.lock().await;
        if status.is_running {
            return Ok(());
        }
        status.is_running = true;
        status.files_found = 0;
        status.files_added = 0;
        drop(status);

        let scanner_self = self.status.clone();
        let db = self.db.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            if force_full {
                info!("Starting full library scan (force-full: cache + pruning bypassed)...");
            } else {
                info!("Starting library scan...");
            }

            let watch_dirs = match db.get_watch_dirs().await {
                Ok(dirs) => dirs,
                Err(e) => {
                    error!("Failed to fetch watch directories for scan: {}", e);
                    let mut s = scanner_self.lock().await;
                    s.is_running = false;
                    return;
                }
            };

            let config_dirs = {
                let cfg = config.read().await;
                cfg.scanner.directories.clone()
            };

            let mut scan_targets: HashMap<PathBuf, bool> = HashMap::new();
            for dir in config_dirs {
                scan_targets.insert(PathBuf::from(dir), true);
            }
            for watch_dir in &watch_dirs {
                scan_targets
                    .entry(PathBuf::from(&watch_dir.path))
                    .and_modify(|recursive| *recursive |= watch_dir.is_recursive)
                    .or_insert(watch_dir.is_recursive);
            }

            // Force-full: wipe probe cache rows under each target so the
            // analyzer is forced to re-probe every file from scratch.
            if force_full {
                for path in scan_targets.keys() {
                    if let Some(prefix) = path.to_str() {
                        match db.clear_media_probe_cache_under(prefix).await {
                            Ok(n) if n > 0 => {
                                info!(
                                    "Force-full scan: cleared {} probe cache rows under {}",
                                    n, prefix
                                );
                            }
                            Ok(_) => {}
                            Err(e) => warn!(
                                "Force-full scan: failed to clear probe cache for {}: {}",
                                prefix, e
                            ),
                        }
                    }
                }
            }

            // Build PruneOptions based on config + force_full override.
            let prune_enabled = if force_full {
                false
            } else {
                let cfg = config.read().await;
                cfg.scanner.aggressive_directory_pruning
            };
            let last_scanned_map = if prune_enabled {
                db.get_watch_dir_last_scanned_map()
                    .await
                    .unwrap_or_default()
            } else {
                HashMap::new()
            };
            let mut prune = PruneOptions {
                enabled: prune_enabled,
                last_scanned_by_root: HashMap::new(),
            };
            if prune_enabled {
                for (path, last_scanned) in last_scanned_map {
                    if let Some(ts) = last_scanned {
                        prune.last_scanned_by_root.insert(PathBuf::from(path), ts);
                    }
                }
            }

            let mut all_scanned = Vec::new();
            let mut completed_roots: Vec<PathBuf> = Vec::new();

            for (path, recursive) in &scan_targets {
                if !path.exists() {
                    warn!("Watch directory does not exist: {:?}", path);
                    continue;
                }

                {
                    let mut s = scanner_self.lock().await;
                    s.current_folder = Some(path.to_string_lossy().to_string());
                }

                let scan_target = path.clone();
                let recursive = *recursive;
                let prune_for_walk = prune.clone();
                let files = match tokio::task::spawn_blocking(move || {
                    let scanner = Scanner::new();
                    scanner.scan_with_options(vec![(scan_target, recursive)], &prune_for_walk)
                })
                .await
                {
                    Ok(files) => files,
                    Err(e) => {
                        error!("Scan worker failed for {:?}: {}", path, e);
                        continue;
                    }
                };
                completed_roots.push(path.clone());
                all_scanned.extend(files);
            }

            {
                let mut s = scanner_self.lock().await;
                s.files_found = all_scanned.len();
                s.current_folder = Some("Processing files...".to_string());
            }

            // Fetch file settings once, then resolve files and write them in
            // chunked transactions. This avoids the per-file settings read and
            // the thousands of individual write transactions that previously
            // monopolized the connection pool and stalled the UI during a large
            // library scan. We yield between chunks so interactive requests can
            // use the pool alongside the scan's writer.
            let settings = db.get_file_settings().await.unwrap_or_else(|e| {
                error!(
                    "Failed to fetch file settings during scan, using defaults: {}",
                    e
                );
                crate::media::pipeline::default_file_settings()
            });

            let mut added: usize = 0;
            const ENQUEUE_CHUNK: usize = 500;
            let mut buffer: Vec<crate::db::PreparedEnqueue> = Vec::with_capacity(ENQUEUE_CHUNK);
            for file in all_scanned {
                match crate::media::pipeline::resolve_discovered_for_enqueue(&db, &file, &settings)
                    .await
                {
                    Ok(Some(prepared)) => buffer.push(prepared),
                    Ok(None) => {}
                    Err(e) => error!("Failed to add job during scan: {}", e),
                }

                if buffer.len() >= ENQUEUE_CHUNK {
                    match db.enqueue_jobs_batch(&buffer).await {
                        Ok(changed) => added += changed as usize,
                        Err(e) => error!("Failed to add job batch during scan: {}", e),
                    }
                    buffer.clear();
                    {
                        let mut s = scanner_self.lock().await;
                        s.files_added = added;
                    }
                    tokio::task::yield_now().await;
                }
            }
            if !buffer.is_empty() {
                match db.enqueue_jobs_batch(&buffer).await {
                    Ok(changed) => added += changed as usize,
                    Err(e) => error!("Failed to add job batch during scan: {}", e),
                }
            }
            {
                let mut s = scanner_self.lock().await;
                s.files_added = added;
            }

            // Record completion timestamps only for roots that finished
            // their walk without erroring out — partial scans must not
            // poison the pruning baseline.
            let now_ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            for root in &completed_roots {
                if let Some(path_str) = root.to_str()
                    && let Err(e) = db.update_watch_dir_last_scanned_at(path_str, now_ts).await
                {
                    warn!("Failed to update last_scanned_at for {}: {}", path_str, e);
                }
            }

            let mut s = scanner_self.lock().await;
            s.files_added = added;
            s.is_running = false;
            s.current_folder = None;
            info!("Library scan complete. Added {} new files.", added);
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use std::time::Duration;

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

    #[tokio::test]
    async fn scanner_ignores_generated_outputs_during_full_scan() -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_scanner_test");
        let watch_dir = temp_watch_dir("alchemist_scanner_dir");
        std::fs::create_dir_all(&watch_dir)?;

        let input_path = watch_dir.join("episode.mp4");
        let generated_output = watch_dir.join("bonus-alchemist.mkv");
        std::fs::write(&input_path, b"source")?;
        std::fs::write(&generated_output, b"generated")?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let mut config = Config::default();
        config.scanner.directories = vec![watch_dir.to_string_lossy().to_string()];
        let config = Arc::new(RwLock::new(config));

        let scanner = LibraryScanner::new(db.clone(), config);
        scanner.start_scan().await?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
        loop {
            let status = scanner.get_status().await;
            if !status.is_running {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("scanner did not finish in time");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let queued = db.get_jobs_by_status(crate::db::JobState::Queued).await?;
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].input_path, input_path.to_string_lossy());
        assert!(
            db.get_job_by_input_path(generated_output.to_string_lossy().as_ref())
                .await?
                .is_none()
        );

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
