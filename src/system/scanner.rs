use crate::db::{Db, Job, JobState};
use crate::error::Result;
use crate::media::scanner::Scanner;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
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
    status: Arc<Mutex<ScanStatus>>,
}

impl LibraryScanner {
    pub fn new(db: Arc<Db>) -> Self {
        Self {
            db,
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

            let scanner = Scanner::new();
            let mut all_scanned = Vec::new();

            for watch_dir in watch_dirs {
                let path = PathBuf::from(&watch_dir.path);
                if !path.exists() {
                    warn!("Watch directory does not exist: {:?}", path);
                    continue;
                }

                {
                    let mut s = scanner_self.lock().await;
                    s.current_folder = Some(watch_dir.path.clone());
                }

                let files = scanner.scan(vec![path]);
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

                // Check if already exists
                match db.get_job_by_input_path(&path_str).await {
                    Ok(Some(_)) => continue,
                    Ok(None) => {
                        // Add new job
                        let output_path = path_str
                            .replace(".mkv", ".mp4")
                            .replace(".avi", ".mp4")
                            .replace(".mov", ".mp4"); // Dummy output logic for now

                        let job = Job {
                            id: 0,
                            input_path: path_str,
                            output_path,
                            status: JobState::Queued,
                            priority: 0,
                            progress: 0.0,
                            attempt_count: 0,
                            decision_reason: None,
                            vmaf_score: None,
                            created_at: chrono::Utc::now(),
                            updated_at: chrono::Utc::now(),
                        };

                        if let Err(e) = db.add_job(job).await {
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
