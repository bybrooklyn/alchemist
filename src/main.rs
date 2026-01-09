use alchemist::error::Result;
use alchemist::system::hardware;
use alchemist::{config, db, Agent, Transcoder};
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use notify::{RecursiveMode, Watcher};
use tokio::sync::broadcast;
use tokio::sync::RwLock;

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
    let config = Arc::new(RwLock::new(config));
    let agent = Arc::new(
        Agent::new(
            db.clone(),
            transcoder.clone(),
            config.clone(),
            Some(hw_info),
            tx.clone(),
            args.dry_run,
        )
        .await,
    );

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
        let watcher_dirs_opt = {
            let config_read = config.read().await;
            if !setup_mode && !config_read.scanner.directories.is_empty() {
                Some(
                    config_read
                        .scanner
                        .directories
                        .iter()
                        .map(PathBuf::from)
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        };

        if let Some(watcher_dirs) = watcher_dirs_opt {
            let watcher = alchemist::system::watcher::FileWatcher::new(watcher_dirs, db.clone());
            let watcher_handle = watcher.clone();
            tokio::spawn(async move {
                if let Err(e) = watcher_handle.start().await {
                    error!("File watcher failed: {}", e);
                }
            });
        }

        // Config Watcher
        let config_watcher_arc = config.clone();
        tokio::task::spawn_blocking(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            // We use recommended_watcher (usually Create/Write/Modify/Remove events)
            let mut watcher = match notify::recommended_watcher(tx) {
                Ok(w) => w,
                Err(e) => {
                    error!("Failed to create config watcher: {}", e);
                    return;
                }
            };

            if let Err(e) = watcher.watch(
                std::path::Path::new("config.toml"),
                RecursiveMode::NonRecursive,
            ) {
                error!("Failed to watch config.toml: {}", e);
                return;
            }

            // Simple debounce by waiting for events
            for res in rx {
                match res {
                    Ok(event) => {
                        // Reload on any event for simplicity, usually Write/Modify
                        // We can filter for event.kind.
                        if let notify::EventKind::Modify(_) = event.kind {
                            info!("Config file changed. Reloading...");
                            // Brief sleep to ensure write complete?
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            match alchemist::config::Config::load(std::path::Path::new(
                                "config.toml",
                            )) {
                                Ok(new_config) => {
                                    // We need to write to the async RwLock from this blocking thread.
                                    // We can use blocking_write() if available or block_on.
                                    // tokio::sync::RwLock can contain a blocking_write feature?
                                    // No, tokio RwLock is async.
                                    // We can spawn a handle back to async world?
                                    // Or just use std::sync::RwLock for config?
                                    // Using `blocking_write` requires `tokio` feature `sync`?
                                    // Actually `config_watcher_arc` is `Arc<tokio::sync::RwLock<Config>>`.
                                    // We can use `futures::executor::block_on` or create a new runtime?
                                    // BETTER: Spawn the loop as a `tokio::spawn`, but use `notify::Event` stream (async config)?
                                    // OR: Use `tokio::sync::RwLock::blocking_write()` method? It exists!
                                    let mut w = config_watcher_arc.blocking_write();
                                    *w = new_config;
                                    info!("Configuration reloaded successfully.");
                                }
                                Err(e) => {
                                    error!("Failed to reload config: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => error!("Config watch error: {:?}", e),
                }
            }
        });

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
