use crate::config::Config;
use crate::db::Db;
use crate::error::Result;
use crate::media::scanner::Scanner;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
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
            info!("🚀 Starting full library scan...");

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
            for watch_dir in watch_dirs {
                scan_targets
                    .entry(PathBuf::from(&watch_dir.path))
                    .and_modify(|recursive| *recursive |= watch_dir.is_recursive)
                    .or_insert(watch_dir.is_recursive);
            }

            let mut all_scanned = Vec::new();

            for (path, recursive) in scan_targets {
                if !path.exists() {
                    warn!("Watch directory does not exist: {:?}", path);
                    continue;
                }

                {
                    let mut s = scanner_self.lock().await;
                    s.current_folder = Some(path.to_string_lossy().to_string());
                }

                let scan_target = path.clone();
                let files = match tokio::task::spawn_blocking(move || {
                    let scanner = Scanner::new();
                    scanner.scan_with_recursion(vec![(scan_target, recursive)])
                })
                .await
                {
                    Ok(files) => files,
                    Err(e) => {
                        error!("Scan worker failed for {:?}: {}", path, e);
                        continue;
                    }
                };
                all_scanned.extend(files);
            }

            {
                let mut s = scanner_self.lock().await;
                s.files_found = all_scanned.len();
                s.current_folder = Some("Processing files...".to_string());
            }

            let mut added = 0;
            for file in all_scanned {
                match crate::media::pipeline::enqueue_discovered_with_db(&db, file).await {
                    Ok(changed) => {
                        if changed {
                            added += 1;
                        }
                    }
                    Err(e) => {
                        error!("Failed to add job during scan: {}", e);
                    }
                }

                if added % 10 == 0 {
                    let mut s = scanner_self.lock().await;
                    s.files_added = added;
                }
            }

            let mut s = scanner_self.lock().await;
            s.files_added = added;
            s.is_running = false;
            s.current_folder = None;
            info!("✅ Library scan complete. Added {} new files.", added);
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
        assert!(db
            .get_job_by_input_path(generated_output.to_string_lossy().as_ref())
            .await?
            .is_none());

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
