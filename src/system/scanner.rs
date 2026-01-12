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

            let settings = match db.get_file_settings().await {
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

            let scanner = Scanner::new();
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

                let files = scanner.scan_with_recursion(vec![(path, recursive)]);
                all_scanned.extend(files);
            }

            {
                let mut s = scanner_self.lock().await;
                s.files_found = all_scanned.len();
                s.current_folder = Some("Processing files...".to_string());
            }

            let mut added = 0;
            for file in all_scanned {
                let path_str = file.path.to_string_lossy().to_string();
                let output_path = settings.output_path_for(&file.path);

                if output_path.exists() && !settings.should_replace_existing_output() {
                    continue;
                }

                // Check if already exists
                match db.get_job_by_input_path(&path_str).await {
                    Ok(Some(_)) => continue,
                    Ok(None) => {
                        if let Err(e) = db.enqueue_job(&file.path, &output_path, file.mtime).await {
                            error!("Failed to add job during scan: {}", e);
                        } else {
                            added += 1;
                        }
                    }
                    Err(e) => error!("Database error during scan check: {}", e),
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
