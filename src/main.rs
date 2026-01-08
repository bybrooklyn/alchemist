use alchemist::error::Result;
use alchemist::system::hardware;
use alchemist::{config, db, Agent, Transcoder};
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

    // Startup Banner
    info!("╔═══════════════════════════════════════════════════════════════╗");
    info!("║                        ALCHEMIST                             ║");
    info!("║              Video Transcoding Automation                     ║");
    info!(
        "║                   Version {}                            ║",
        env!("CARGO_PKG_VERSION")
    );
    info!("╚═══════════════════════════════════════════════════════════════╝");
    info!("");
    info!("System Information:");
    info!(
        "  OS: {} ({})",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    info!("  CPUs: {}", num_cpus::get());
    info!("");

    let args = Args::parse();

    // 0. Load Configuration (with first-boot wizard)
    let config_path = std::path::Path::new("config.toml");
    let config = if !config_path.exists() && args.server {
        // First boot: Run configuration wizard
        info!("No configuration file found. Starting configuration wizard...");
        info!("");

        match alchemist::wizard::ConfigWizard::run(config_path) {
            Ok(cfg) => {
                info!("");
                info!("Configuration complete! Continuing with server startup...");
                info!("");
                cfg
            }
            Err(e) => {
                error!("Configuration wizard failed: {}", e);
                error!("You can:");
                error!("  1. Run the wizard again: alchemist --server");
                error!("  2. Create config.toml manually");
                error!("  3. Use Python wizard: python setup/configure.py");
                return Err(e);
            }
        }
    } else {
        // Normal boot or CLI mode: Load config or use defaults
        config::Config::load(config_path).unwrap_or_else(|e| {
            warn!("Failed to load config.toml: {}. Using defaults.", e);
            config::Config::default()
        })
    };

    // Log Configuration
    info!("Configuration:");
    info!("  Concurrent Jobs: {}", config.transcode.concurrent_jobs);
    info!(
        "  Size Reduction Threshold: {:.1}%",
        config.transcode.size_reduction_threshold * 100.0
    );
    info!("  Min File Size: {} MB", config.transcode.min_file_size_mb);
    info!(
        "  CPU Fallback: {}",
        if config.hardware.allow_cpu_fallback {
            "Enabled"
        } else {
            "Disabled"
        }
    );
    info!(
        "  CPU Encoding: {}",
        if config.hardware.allow_cpu_encoding {
            "Enabled"
        } else {
            "Disabled"
        }
    );
    info!("  CPU Preset: {}", config.hardware.cpu_preset);
    info!("");

    // 1. Hardware Detection
    let hw_info = hardware::detect_hardware(config.hardware.allow_cpu_fallback)?;
    info!("");
    info!("Selected Hardware: {}", hw_info.vendor);
    if let Some(ref path) = hw_info.device_path {
        info!("  Device Path: {}", path);
    }

    // Check CPU encoding policy
    if hw_info.vendor == hardware::Vendor::Cpu {
        if !config.hardware.allow_cpu_encoding {
            error!("CPU encoding is disabled in configuration.");
            error!("Set hardware.allow_cpu_encoding = true in config.toml to enable CPU fallback.");
            return Err(alchemist::error::AlchemistError::Config(
                "CPU encoding disabled".into(),
            ));
        }
        warn!("Running in CPU-only mode. Transcoding will be slower.");
    }
    info!("");

    // 2. Initialize Database, Broadcast Channel, Orchestrator, and Processor
    let db = Arc::new(db::Db::new("alchemist.db").await?);
    let (tx, _rx) = broadcast::channel(100);
    let transcoder = Arc::new(Transcoder::new());
    let config = Arc::new(config);
    let agent = Arc::new(Agent::new(
        db.clone(),
        transcoder.clone(),
        config.clone(),
        Some(hw_info),
        tx.clone(),
        args.dry_run,
    ));

    info!("Database and services initialized.");

    // 3. Start Background Processor Loop
    let proc = agent.clone();
    tokio::spawn(async move {
        proc.run_loop().await;
    });

    if args.server {
        info!("Starting web server...");
        alchemist::server::run_server(db, config, agent, transcoder, tx).await?;
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
        agent.scan_and_enqueue(args.directories).await?;

        // Wait until all jobs are processed
        info!("Waiting for jobs to complete...");
        loop {
            let stats = db.get_stats().await?;
            let active = stats.as_object().map(|m| m.iter().filter(|(k, _)| ["encoding", "analyzing", "resuming"].contains(&k.as_str())).map(|(_, v)| v.as_i64().unwrap_or(0)).sum::<i64>()).unwrap_or(0);
            let queued = stats.get("queued").and_then(|v| v.as_i64()).unwrap_or(0);

            if active + queued == 0 {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
        info!("All jobs processed.");
    }

    Ok(())
}
