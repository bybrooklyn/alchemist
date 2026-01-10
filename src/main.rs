use alchemist::error::Result;
use alchemist::system::hardware;
use alchemist::{config, db, Agent, Transcoder};
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
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
    info!(" ______     __         ______     __  __     ______     __    __     __     ______     ______ ");
    info!("/\\  __ \\   /\\ \\       /\\  ___\\   /\\ \\_\\ \\   /\\  ___\\   /\\ \"-./  \\   /\\ \\   /\\  ___\\   /\\__  _\\");
    info!("\\ \\  __ \\  \\ \\ \\____  \\ \\ \\____  \\ \\  __ \\  \\ \\  __\\   \\ \\ \\-./\\ \\  \\ \\ \\  \\ \\___  \\  \\/_/\\ \\/");
    info!(" \\ \\_\\ \\_\\  \\ \\_____\\  \\ \\_____\\  \\ \\_\\ \\_\\  \\ \\_____\\  \\ \\_\\ \\ \\_\\  \\ \\_\\  \\/\\_____\\    \\ \\_\\");
    info!("  \\/_/\\/_/   \\/_____/   \\/_____/   \\/_/\\/_/   \\/_____/   \\/_/  \\/_/   \\/_/   \\/_____/     \\/_/");
    info!("");
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

    // Default to server mode if no arguments are provided (e.g. double-click run)
    // or if explicit --server flag is used
    let is_server_mode = args.server || args.directories.is_empty();

    // 0. Load Configuration
    let config_path = std::path::Path::new("config.toml");
    let (config, setup_mode) = if !config_path.exists() {
        if is_server_mode {
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

    // Initialize Notification Manager
    let notification_manager = Arc::new(alchemist::notifications::NotificationManager::new(
        db.as_ref().clone(),
    ));
    notification_manager.start_listener(tx.subscribe());

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
    // Always start the loop. The agent will be paused if setup_mode is true.
    if setup_mode {
        agent.pause();
    }
    let proc = agent.clone();
    tokio::spawn(async move {
        proc.run_loop().await;
    });

    if is_server_mode {
        info!("Starting web server...");

        // Start Log Persistence Task
        let log_db = db.clone();
        let mut log_rx = tx.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = log_rx.recv().await {
                match event {
                    alchemist::db::AlchemistEvent::Log {
                        level,
                        job_id,
                        message,
                        ..
                    } => {
                        if let Err(e) = log_db.add_log(&level, job_id, &message).await {
                            eprintln!("Failed to persist log: {}", e);
                        }
                    }
                    _ => {}
                }
            }
        });

        // Initialize File Watcher
        let file_watcher = Arc::new(alchemist::system::watcher::FileWatcher::new(db.clone()));

        // Function to reload watcher (Config + DB)
        let reload_watcher = {
            let config = config.clone();
            let db = db.clone();
            let file_watcher = file_watcher.clone();

            move |setup_mode: bool| {
                let config = config.clone();
                let db = db.clone();
                let file_watcher = file_watcher.clone();
                async move {
                    let mut all_dirs = Vec::new();

                    // 1. Config Dirs
                    {
                        let config_read = config.read().await;
                        if !setup_mode {
                            all_dirs
                                .extend(config_read.scanner.directories.iter().map(PathBuf::from));
                        }
                    }

                    // 2. DB Dirs
                    if !setup_mode {
                        match db.get_watch_dirs().await {
                            Ok(dirs) => {
                                all_dirs.extend(dirs.into_iter().map(|d| PathBuf::from(d.path)));
                            }
                            Err(e) => error!("Failed to fetch watch dirs from DB: {}", e),
                        }
                    }

                    // Deduplicate
                    all_dirs.sort();
                    all_dirs.dedup();

                    if !all_dirs.is_empty() {
                        info!("Updating file watcher with {} directories", all_dirs.len());
                        if let Err(e) = file_watcher.watch(&all_dirs) {
                            error!("Failed to update file watcher: {}", e);
                        }
                    } else {
                        // Ensure we clear it if empty?
                        // The file_watcher.watch() handles empty list by stopping watcher.
                        if let Err(e) = file_watcher.watch(&[]) {
                            debug!("Watcher stopped (empty list): {}", e);
                        }
                    }
                }
            }
        };

        // Initial Watcher Load
        reload_watcher(setup_mode).await;

        // Start Scheduler
        let scheduler = alchemist::scheduler::Scheduler::new(db.clone(), agent.clone());
        scheduler.start();

        // Async Config Watcher
        let config_watcher_arc = config.clone();
        let reload_watcher_clone = reload_watcher.clone();

        // Channel for file events
        let (tx_notify, mut rx_notify) = tokio::sync::mpsc::unbounded_channel();

        let tx_notify_clone = tx_notify.clone();
        let watcher_res = notify::recommended_watcher(
            move |res: std::result::Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx_notify_clone.send(event);
                }
            },
        );

        match watcher_res {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(
                    std::path::Path::new("config.toml"),
                    RecursiveMode::NonRecursive,
                ) {
                    error!("Failed to watch config.toml: {}", e);
                } else {
                    // Prevent watcher from dropping by keeping it in the spawn if needed,
                    // or just spawning the processing loop.
                    // notify watcher works in background thread usually.
                    // We need to keep `watcher` alive.

                    tokio::spawn(async move {
                        // Keep watcher alive by moving it here
                        let _watcher = watcher;

                        while let Some(event) = rx_notify.recv().await {
                            if let notify::EventKind::Modify(_) = event.kind {
                                info!("Config file changed. Reloading...");
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                                match alchemist::config::Config::load(std::path::Path::new(
                                    "config.toml",
                                )) {
                                    Ok(new_config) => {
                                        {
                                            let mut w = config_watcher_arc.write().await;
                                            *w = new_config;
                                        }
                                        info!("Configuration reloaded successfully.");

                                        // Trigger watcher update (merges DB + New Config)
                                        reload_watcher_clone(false).await;
                                    }
                                    Err(e) => error!("Failed to reload config: {}", e),
                                }
                            }
                        }
                    });
                }
            }
            Err(e) => error!("Failed to create config watcher: {}", e),
        }

        alchemist::server::run_server(
            db,
            config,
            agent,
            transcoder,
            tx,
            setup_mode,
            notification_manager.clone(),
            file_watcher,
        )
        .await?;
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
