use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
use alchemist::{analyzer, config, db, hardware, orchestrator, scanner};
use alchemist::db::JobState;
use alchemist::server::AlchemistEvent;
use tokio::sync::broadcast;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directories to scan for media files
    #[arg(required = true)]
    directories: Vec<PathBuf>,

    /// Dry run (don't actually transcode)
    #[arg(short, long)]
    dry_run: bool,

    /// Output directory (optional, defaults to same as input with .av1)
    #[arg(short, long)]
    output_dir: Option<PathBuf>,

    /// Run as web server
    #[arg(long)]
    server: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    info!("Alchemist starting...");

    let args = Args::parse();
    
    // 0. Load Configuration
    let config_path = std::path::Path::new("config.toml");
    let config = config::Config::load(config_path).unwrap_or_else(|e| {
        warn!("Failed to load config.toml: {}. Using defaults.", e);
        config::Config::default()
    });

    // 1. Hardware Detection
    let hw_info = match hardware::detect_hardware() {
        Ok(info) => {
            info!("Hardware detected: {}", info.vendor);
            Some(info)
        }
        Err(e) => {
            error!("{}", e);
            if !config.hardware.allow_cpu_fallback && !args.dry_run {
                error!("GPU unavailable. CPU fallback: disabled. Exiting.");
                return Err(e);
            }
            warn!("GPU unavailable. CPU fallback: enabled.");
            None
        }
    };

    // 2. Initialize Database and Broadcast Channel
    let db = db::Db::new("alchemist.db").await?;
    let (tx, _rx) = broadcast::channel(100);
    info!("Database and Broadcast channel initialized.");

    if args.server {
        info!("Starting web server...");
        alchemist::server::run_server(Arc::new(db), Arc::new(config), tx).await?;
        return Ok(());
    }

    // 3. Scan directories and enqueue jobs
    let scanner = scanner::Scanner::new();
    let files = scanner.scan(args.directories);

    if files.is_empty() {
        info!("No media files found to process.");
    } else {
        for scanned_file in files {
            // Basic output path generation - can be refined later
            let mut output_path = scanned_file.path.clone();
            output_path.set_extension("av1.mkv");
            
            if let Err(e) = db.enqueue_job(&scanned_file.path, &output_path, scanned_file.mtime).await {
                error!("Failed to enqueue job for {:?}: {}", scanned_file.path, e);
            }
        }
    }

    // 4. Process Queue
    let orchestrator = Arc::new(orchestrator::Orchestrator::new());
    let db = Arc::new(db);
    let config = Arc::new(config);
    let hw_info = Arc::new(hw_info);
    let tx = Arc::new(tx);
    
    let semaphore = Arc::new(Semaphore::new(config.transcode.concurrent_jobs));
    let mut futures = Vec::new();

    while let Some(job) = db.get_next_job().await? {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        
        let db = db.clone();
        let orchestrator = orchestrator.clone();
        let config = config.clone();
        let hw_info = hw_info.clone();
        let tx = tx.clone();
        let dry_run = args.dry_run;

        let future = tokio::spawn(async move {
            let _permit = permit; // Hold permit until job is done
            
            let file_path = std::path::PathBuf::from(&job.input_path);
            let output_path = std::path::PathBuf::from(&job.output_path);

            info!("--- Processing Job {}: {:?} ---", job.id, file_path.file_name().unwrap_or_default());
            
            // 1. ANALYZING
            let _ = db.update_job_status(job.id, JobState::Analyzing).await;
            let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Analyzing });

            // Preflight Analysis
            match analyzer::Analyzer::probe(&file_path) {
                Ok(metadata) => {
                    let (should_encode, reason) = analyzer::Analyzer::should_transcode(&file_path, &metadata, &config);
                    
                    if should_encode {
                        // 2. ENCODING
                        info!("Decision: ENCODE Job {} - {}", job.id, reason);
                        let _ = db.add_decision(job.id, "encode", &reason).await;
                        let _ = tx.send(AlchemistEvent::Decision { job_id: job.id, action: "encode".to_string(), reason: reason.clone() });
                        let _ = db.update_job_status(job.id, JobState::Encoding).await;
                        let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Encoding });
                        
                        if let Err(e) = orchestrator.transcode_to_av1(&file_path, &output_path, hw_info.as_ref().as_ref(), dry_run, &metadata, Some((job.id, tx.clone()))) {
                            error!("Transcode failed for Job {}: {}", job.id, e);
                            let _ = db.add_decision(job.id, "reject", &e.to_string()).await;
                            let _ = tx.send(AlchemistEvent::Decision { job_id: job.id, action: "reject".to_string(), reason: e.to_string() });
                            let _ = db.update_job_status(job.id, JobState::Failed).await;
                            let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Failed });
                        } else if !dry_run {
                            // Size Reduction Gate
                            let input_size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
                            let output_size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
                            
                            let reduction = 1.0 - (output_size as f64 / input_size as f64);
                            info!("Job {}: Size reduction: {:.2}% ({} -> {})", job.id, reduction * 100.0, input_size, output_size);
                            
                            if reduction < config.transcode.size_reduction_threshold {
                                info!("Job {}: Size reduction gate failed ({:.2}% < {:.0}%). Reverting.", 
                                    job.id, reduction * 100.0, config.transcode.size_reduction_threshold * 100.0);
                                std::fs::remove_file(&output_path).ok();
                                let _ = db.add_decision(job.id, "skip", "Inefficient: <30% reduction").await;
                                let _ = tx.send(AlchemistEvent::Decision { job_id: job.id, action: "skip".to_string(), reason: "Inefficient: <30% reduction".to_string() });
                                let _ = db.update_job_status(job.id, JobState::Skipped).await;
                                let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Skipped });
                            } else {
                                // Integrity Check
                                match analyzer::Analyzer::probe(&output_path) {
                                    Ok(_) => {
                                        info!("Job {}: Size reduction and integrity gate passed. Job completed.", job.id);
                                        let _ = db.update_job_status(job.id, JobState::Completed).await;
                                        let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Completed });
                                    }
                                    Err(e) => {
                                        error!("Job {}: Integrity check failed for {:?}: {}", job.id, output_path, e);
                                        std::fs::remove_file(&output_path).ok();
                                        let _ = db.add_decision(job.id, "reject", &format!("Integrity check failed: {}", e)).await;
                                        let _ = tx.send(AlchemistEvent::Decision { job_id: job.id, action: "reject".to_string(), reason: format!("Integrity check failed: {}", e) });
                                        let _ = db.update_job_status(job.id, JobState::Failed).await;
                                        let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Failed });
                                    }
                                }
                            }
                        } else {
                            let _ = db.update_job_status(job.id, JobState::Completed).await;
                            let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Completed });
                        }
                    } else {
                        // 2. SKIPPED
                        info!("Decision: SKIP Job {} - {}", job.id, reason);
                        let _ = db.add_decision(job.id, "skip", &reason).await;
                        let _ = tx.send(AlchemistEvent::Decision { job_id: job.id, action: "skip".to_string(), reason: reason.clone() });
                        let _ = db.update_job_status(job.id, JobState::Skipped).await;
                        let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Skipped });
                    }
                }
                Err(e) => {
                    error!("Job {}: Failed to probe {:?}: {}", job.id, file_path, e);
                    let _ = db.add_decision(job.id, "reject", &e.to_string()).await;
                    let _ = tx.send(AlchemistEvent::Decision { job_id: job.id, action: "reject".to_string(), reason: e.to_string() });
                    let _ = db.update_job_status(job.id, JobState::Failed).await;
                    let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Failed });
                }
            }
        });
        futures.push(future);
    }

    // Wait for all jobs to finish
    futures::future::join_all(futures).await;

    info!("All jobs processed.");
    Ok(())
}


