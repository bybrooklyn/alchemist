use alchemist::error::Result;
use alchemist::{Orchestrator, Processor, config, db, hardware};
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use tokio::sync::broadcast;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directories to scan for media files
    #[arg()]
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
async fn main() -> Result<()> {
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

    // 2. Initialize Database, Broadcast Channel, Orchestrator, and Processor
    let db = Arc::new(db::Db::new("alchemist.db").await?);
    let (tx, _rx) = broadcast::channel(100);
    let orchestrator = Arc::new(Orchestrator::new());
    let config = Arc::new(config);
    let processor = Arc::new(Processor::new(
        db.clone(),
        orchestrator.clone(),
        config.clone(),
        hw_info,
        tx.clone(),
    ));

    info!("Database and services initialized.");

    // 3. Start Background Processor Loop
    let proc = processor.clone();
    tokio::spawn(async move {
        proc.run_loop().await;
    });

    if args.server {
        info!("Starting web server...");
        alchemist::server::run_server(db, config, processor, orchestrator, tx).await?;
    } else {
        // CLI Mode
        if args.directories.is_empty() {
            error!(
                "No directories provided. Usage: alchemist <DIRECTORIES>... or alchemist --server"
            );
            return Err(alchemist::error::AlchemistError::Config(
                "Missing directories for CLI mode".into(),
            ));
        }
        processor.scan_and_enqueue(args.directories).await?;

        // Wait until all jobs are processed
        info!("Waiting for jobs to complete...");
        loop {
            let stats = db.get_stats().await?;
            let processing = stats
                .get("processing")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let queued = stats.get("queued").and_then(|v| v.as_i64()).unwrap_or(0);
            let analyzing = stats.get("analyzing").and_then(|v| v.as_i64()).unwrap_or(0);
            let encoding = stats.get("encoding").and_then(|v| v.as_i64()).unwrap_or(0);

            if processing + queued + analyzing + encoding == 0 {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
        info!("All jobs processed.");
    }

    Ok(())
}
