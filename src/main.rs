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
    info!("║                          ALCHEMIST                            ║");
    info!("║                Video Transcoding Automation                   ║");
    info!(
        "║                     Version {}                             ║",
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

    // 0. Load Configuration
    let config_path = std::path::Path::new("config.toml");
    let (config, setup_mode) = if !config_path.exists() {
        if args.server {
            info!("No configuration file found. Entering Setup Mode (Web UI).");
            (config::Config::default(), true)
        } else {
            // CLI mode requires config or explicit args (which are not fully implemented for all settings)
            // For now, let's just warn and use defaults, or error out.
            // But the user specific request is about Docker/Server.
            warn!("No configuration file found. Using defaults.");
            (config::Config::default(), false) // Assuming defaults are safe or dry-run
        }
    } else {
        match config::Config::load(config_path) {
            Ok(c) => (c, false),
            Err(e) => {
                warn!("Failed to load config.toml: {}. Using defaults.", e);
                (config::Config::default(), false)
            }
        }
    };

    if !setup_mode {
        // Log Configuration only if not in setup mode
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
    }
    info!("");

    // 1. Hardware Detection
    let hw_info = hardware::detect_hardware(config.hardware.allow_cpu_fallback)?;
    info!("");
    info!("Selected Hardware: {}", hw_info.vendor);
    if let Some(ref path) = hw_info.device_path {
        info!("  Device Path: {}", path);
    }

    // Check CPU encoding policy
    if !setup_mode && hw_info.vendor == hardware::Vendor::Cpu {
        if !config.hardware.allow_cpu_encoding {
            // In setup mode, we might not have set this yet, so don't error out.
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
    // Only start if NOT in setup mode. If in setup mode, the agent should effectively be paused or idle/empty
    // But since we pass agent to server, we can start it. It just won't have any jobs or directories to scan yet.
    // However, if we want to be strict, we can pause it.
    if setup_mode {
        info!("Setup mode active. Background processor paused.");
        agent.pause();
    } else {
        let proc = agent.clone();
        tokio::spawn(async move {
            proc.run_loop().await;
        });
    }

    if args.server {
        info!("Starting web server...");

        // Start File Watcher if directories are configured and not in setup mode
        if !setup_mode && !config.scanner.directories.is_empty() {
            let watcher_dirs: Vec<PathBuf> = config
                .scanner
                .directories
                .iter()
                .map(PathBuf::from)
                .collect();
            let watcher = alchemist::system::watcher::FileWatcher::new(watcher_dirs, db.clone());
            let watcher_handle = watcher.clone();
            tokio::spawn(async move {
                if let Err(e) = watcher_handle.start().await {
                    error!("File watcher failed: {}", e);
                }
            });
        }

        alchemist::server::run_server(db, config, agent, transcoder, tx, setup_mode).await?;
    } else {
        // CLI Mode
        if setup_mode {
            error!("Configuration required. Run with --server to use the web-based setup wizard, or create config.toml manually.");
            return Err(alchemist::error::AlchemistError::Config(
                "Missing configuration".into(),
            ));
        }

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
            let active = stats
                .as_object()
                .map(|m| {
                    m.iter()
                        .filter(|(k, _)| {
                            ["encoding", "analyzing", "resuming"].contains(&k.as_str())
                        })
                        .map(|(_, v)| v.as_i64().unwrap_or(0))
                        .sum::<i64>()
                })
                .unwrap_or(0);
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
