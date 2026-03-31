use alchemist::db::EventChannels;
use alchemist::error::Result;
use alchemist::system::hardware;
use alchemist::version;
use alchemist::{Agent, Transcoder, config, db, runtime};
use clap::Parser;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

use notify::{RecursiveMode, Watcher};
use tokio::sync::RwLock;
use tokio::sync::broadcast;

#[derive(Parser, Debug)]
#[command(author, version = version::current(), about, long_about = None)]
struct Args {
    /// Run in CLI mode (process directories and exit)
    #[arg(long)]
    cli: bool,

    /// Directories to scan for media files (CLI mode only)
    #[arg(long, value_name = "DIR")]
    directories: Vec<PathBuf>,

    /// Dry run (don't actually transcode)
    #[arg(short, long)]
    dry_run: bool,

    /// Reset admin user/password and sessions (forces setup mode)
    #[arg(long)]
    reset_auth: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 0. Ensure CWD is set to executable directory (fixes double-click issues on Windows)
    #[cfg(target_os = "windows")]
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            if let Err(e) = std::env::set_current_dir(exe_dir) {
                eprintln!("Failed to set working directory: {}", e);
            }
        }
    }

    match run().await {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Application error: {}", e);
            // On error, if we are in a terminal that might close, pause
            if std::env::var("ALCHEMIST_NO_PAUSE").is_err() {
                println!("\nPress Enter to exit...");
                let mut input = String::new();
                let _ = std::io::stdin().read_line(&mut input);
            }
            Err(e)
        }
    }
}

async fn apply_reloaded_config(
    db: &Arc<db::Db>,
    config_path: &Path,
    config_state: &Arc<RwLock<config::Config>>,
    agent: &Arc<Agent>,
    hardware_state: &hardware::HardwareState,
    hardware_probe_log: &Arc<RwLock<hardware::HardwareProbeLog>>,
) -> Result<hardware::HardwareInfo> {
    let new_config = config::Config::load(config_path)
        .map_err(|err| alchemist::error::AlchemistError::Config(err.to_string()))?;
    let (detected_hardware, probe_log) = hardware::detect_hardware_with_log(&new_config).await?;
    let new_limit = new_config.transcode.concurrent_jobs;
    alchemist::settings::project_config_to_db(db.as_ref(), &new_config).await?;

    {
        let mut config_guard = config_state.write().await;
        *config_guard = new_config;
    }

    hardware_state
        .replace(Some(detected_hardware.clone()))
        .await;
    *hardware_probe_log.write().await = probe_log;
    agent.set_concurrent_jobs(new_limit).await;

    Ok(detected_hardware)
}

fn config_watch_target(config_path: &Path) -> &Path {
    config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn should_reload_config_for_event(event: &notify::Event, config_path: &Path) -> bool {
    if !event.paths.iter().any(|path| path == config_path) {
        return false;
    }

    matches!(
        &event.kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Any
    )
}

fn orphaned_temp_output_path(output_path: &str) -> PathBuf {
    PathBuf::from(format!("{output_path}.alchemist.tmp"))
}

async fn run() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();

    let boot_start = Instant::now();

    // Startup Banner
    info!(
        " ______     __         ______     __  __     ______     __    __     __     ______     ______ "
    );
    info!(
        "/\\  __ \\   /\\ \\       /\\  ___\\   /\\ \\_\\ \\   /\\  ___\\   /\\ \"-./  \\   /\\ \\   /\\  ___\\   /\\__  _\\"
    );
    info!(
        "\\ \\  __ \\  \\ \\ \\____  \\ \\ \\____  \\ \\  __ \\  \\ \\  __\\   \\ \\ \\-./\\ \\  \\ \\ \\  \\ \\___  \\  \\/_/\\ \\/"
    );
    info!(
        " \\ \\_\\ \\_\\  \\ \\_____\\  \\ \\_____\\  \\ \\_\\ \\_\\  \\ \\_____\\  \\ \\_\\ \\ \\_\\  \\ \\_\\  \\/\\_____\\    \\ \\_\\"
    );
    info!(
        "  \\/_/\\/_/   \\/_____/   \\/_____/   \\/_/\\/_/   \\/_____/   \\/_/  \\/_/   \\/_/   \\/_____/     \\/_/"
    );
    info!("");
    info!("");
    let version = alchemist::version::current();
    let build_info = option_env!("BUILD_INFO")
        .or(option_env!("GIT_SHA"))
        .or(option_env!("VERGEN_GIT_SHA"))
        .unwrap_or("unknown");
    info!("Version: {}", version);
    info!("Build: {}", build_info);
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
    info!(
        target: "startup",
        "Parsed CLI args: cli_mode={}, reset_auth={}, dry_run={}, directories={}",
        args.cli,
        args.reset_auth,
        args.dry_run,
        args.directories.len()
    );

    // ... rest of logic remains largely the same, just inside run()
    // Default to server mode unless CLI is explicitly requested.
    let is_server_mode = !args.cli;
    info!(target: "startup", "Resolved server mode: {}", is_server_mode);
    if is_server_mode && !args.directories.is_empty() {
        warn!("Directories were provided without --cli; ignoring CLI inputs.");
    }

    // 0. Load Configuration
    let config_start = Instant::now();
    let config_path = runtime::config_path();
    let db_path = runtime::db_path();
    let config_mutable = runtime::config_mutable();
    let config_exists = config_path.exists();
    let (config, mut setup_mode) = if !config_exists {
        let cwd = std::env::current_dir().ok();
        info!(
            target: "startup",
            "Config file not found at {:?} (cwd={:?})",
            config_path,
            cwd
        );
        if is_server_mode {
            info!("No configuration file found. Entering Setup Mode (Web UI).");
            (config::Config::default(), true)
        } else {
            // CLI mode requires config or explicit args
            warn!("No configuration file found. Using defaults.");
            (config::Config::default(), false)
        }
    } else {
        match config::Config::load(config_path.as_path()) {
            Ok(c) => (c, false),
            Err(e) => {
                warn!(
                    "Failed to load config file at {:?}: {}. Using defaults.",
                    config_path, e
                );
                if is_server_mode {
                    warn!("Config load failed in server mode. Entering Setup Mode (Web UI).");
                    (config::Config::default(), true)
                } else {
                    (config::Config::default(), false)
                }
            }
        }
    };
    info!(
        target: "startup",
        "Config loaded (path={:?}, exists={}, mutable={}, setup_mode={}) in {} ms",
        config_path,
        config_exists,
        config_mutable,
        setup_mode,
        config_start.elapsed().as_millis()
    );

    // 1. Initialize Database
    let db_start = Instant::now();
    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(alchemist::error::AlchemistError::Io)?;
        }
    }
    let db = Arc::new(db::Db::new(db_path.to_string_lossy().as_ref()).await?);
    alchemist::settings::project_config_to_db(db.as_ref(), &config).await?;
    info!(
        target: "startup",
        "Database initialized at {:?} in {} ms",
        db_path,
        db_start.elapsed().as_millis()
    );

    let interrupted_jobs = {
        let mut jobs = Vec::new();
        match db.get_jobs_by_status(db::JobState::Encoding).await {
            Ok(mut encoding_jobs) => jobs.append(&mut encoding_jobs),
            Err(err) => error!("Failed to load interrupted encoding jobs: {}", err),
        }
        match db.get_jobs_by_status(db::JobState::Remuxing).await {
            Ok(mut remuxing_jobs) => jobs.append(&mut remuxing_jobs),
            Err(err) => error!("Failed to load interrupted remuxing jobs: {}", err),
        }
        match db.get_jobs_by_status(db::JobState::Analyzing).await {
            Ok(mut analyzing_jobs) => jobs.append(&mut analyzing_jobs),
            Err(err) => error!("Failed to load interrupted analyzing jobs: {}", err),
        }
        jobs
    };

    match db.reset_interrupted_jobs().await {
        Ok(count) if count > 0 => {
            warn!("{} interrupted jobs reset to queued", count);
            for job in interrupted_jobs {
                let temp_path = orphaned_temp_output_path(&job.output_path);
                if std::fs::metadata(&temp_path).is_ok() {
                    match std::fs::remove_file(&temp_path) {
                        Ok(_) => warn!("Removed orphaned temp file: {}", temp_path.display()),
                        Err(err) => error!(
                            "Failed to remove orphaned temp file {}: {}",
                            temp_path.display(),
                            err
                        ),
                    }
                }
            }
        }
        Ok(_) => {}
        Err(err) => error!("Failed to reset interrupted jobs: {}", err),
    }

    let log_retention_days = config.system.log_retention_days.unwrap_or(30);
    match db.prune_old_logs(log_retention_days).await {
        Ok(count) if count > 0 => info!("Pruned {} old log rows", count),
        Ok(_) => {}
        Err(err) => error!("Failed to prune old logs: {}", err),
    }

    match db.cleanup_expired_sessions().await {
        Ok(count) => debug!("Removed {} expired sessions at startup", count),
        Err(err) => error!("Failed to cleanup expired sessions: {}", err),
    }

    if args.reset_auth {
        db.reset_auth().await?;
        warn!("Auth reset requested. All users and sessions cleared.");
        setup_mode = true;
    }
    if is_server_mode {
        let users_start = Instant::now();
        let has_users = db.has_users().await?;
        info!(
            target: "startup",
            "User check completed (has_users={}) in {} ms",
            has_users,
            users_start.elapsed().as_millis()
        );
        if !has_users {
            if !setup_mode {
                info!("No users found. Entering Setup Mode (Web UI).");
            }
            setup_mode = true;
        }
    }

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

    // 2. Hardware Detection (using async version to avoid blocking runtime)
    let hw_start = Instant::now();
    let mut detection_config = config.clone();
    if setup_mode {
        detection_config.hardware.allow_cpu_fallback = true;
    }
    let (hw_info, initial_probe_log) =
        hardware::detect_hardware_with_log(&detection_config).await?;
    info!(
        target: "startup",
        "Hardware detection completed in {} ms",
        hw_start.elapsed().as_millis()
    );
    info!("");
    info!("Selected Hardware: {}", hw_info.vendor);
    if let Some(ref path) = hw_info.device_path {
        info!("  Device Path: {}", path);
    }
    alchemist::media::ffmpeg::warm_encoder_cache();

    // Check CPU encoding policy
    if !setup_mode && hw_info.vendor == hardware::Vendor::Cpu {
        if !config.hardware.allow_cpu_encoding {
            // In setup mode, we might not have set this yet, so don't error out.
            error!("CPU encoding is disabled in configuration.");
            error!(
                "Set hardware.allow_cpu_encoding = true in {:?} to enable CPU fallback.",
                config_path
            );
            return Err(alchemist::error::AlchemistError::Config(
                "CPU encoding disabled".into(),
            ));
        }
        warn!("Running in CPU-only mode. Transcoding will be slower.");
    }
    info!("");

    // 3. Initialize Broadcast Channels, Orchestrator, and Processor
    let services_start = Instant::now();

    // Create separate event channels by type and volume
    let (jobs_tx, _jobs_rx) = broadcast::channel(1000); // High volume - job events
    let (config_tx, _config_rx) = broadcast::channel(50); // Low volume - config events
    let (system_tx, _system_rx) = broadcast::channel(100); // Medium volume - system events

    let event_channels = Arc::new(EventChannels {
        jobs: jobs_tx,
        config: config_tx,
        system: system_tx,
    });

    // Keep legacy channel for transition compatibility
    let (tx, _rx) = broadcast::channel(100);

    // Initialize Notification Manager
    let notification_manager = Arc::new(alchemist::notifications::NotificationManager::new(
        db.as_ref().clone(),
    ));
    notification_manager.start_listener(tx.subscribe());

    let transcoder = Arc::new(Transcoder::new());
    let hardware_state = hardware::HardwareState::new(Some(hw_info.clone()));
    let hardware_probe_log = Arc::new(RwLock::new(initial_probe_log));
    let config = Arc::new(RwLock::new(config));

    let maintenance_db = db.clone();
    let maintenance_config = config.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60 * 60 * 24));
        interval.tick().await;
        loop {
            interval.tick().await;

            let retention_days = maintenance_config
                .read()
                .await
                .system
                .log_retention_days
                .unwrap_or(30);
            match maintenance_db.prune_old_logs(retention_days).await {
                Ok(count) if count > 0 => info!("Pruned {} old log rows", count),
                Ok(_) => {}
                Err(err) => error!("Failed to prune old logs: {}", err),
            }

            match maintenance_db.cleanup_expired_sessions().await {
                Ok(count) => debug!("Removed {} expired sessions", count),
                Err(err) => error!("Failed to cleanup expired sessions: {}", err),
            }
        }
    });

    let agent = Arc::new(
        Agent::new(
            db.clone(),
            transcoder.clone(),
            config.clone(),
            hardware_state.clone(),
            tx.clone(),
            event_channels.clone(),
            args.dry_run,
        )
        .await,
    );

    info!("Database and services initialized.");
    info!(
        target: "startup",
        "Core services initialized in {} ms",
        services_start.elapsed().as_millis()
    );

    // 3. Start Background Processor Loop
    // In server mode the engine starts paused and waits for an explicit user action.
    if is_server_mode || setup_mode {
        agent.pause();
    }
    let proc = agent.clone();
    tokio::spawn(async move {
        proc.run_loop().await;
    });

    if is_server_mode {
        info!("Starting web server...");

        // Initialize File Watcher
        let file_watcher = Arc::new(alchemist::system::watcher::FileWatcher::new(db.clone()));

        // Initialize Library Scanner (shared between boot task and server)
        let library_scanner = Arc::new(alchemist::system::scanner::LibraryScanner::new(
            db.clone(),
            config.clone(),
        ));

        if !setup_mode {
            let scan_agent = agent.clone();
            let startup_scanner = library_scanner.clone();
            tokio::spawn(async move {
                // Small delay to let the server fully initialize
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                // Trigger a full library scan first
                if let Err(e) = startup_scanner.start_scan().await {
                    error!("Startup scan failed: {e}");
                }

                // Wait for scan to complete (poll until not running)
                loop {
                    let status = startup_scanner.get_status().await;
                    if !status.is_running {
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }

                // Now analyze all queued + failed jobs
                scan_agent.analyze_pending_jobs().await;
            });
        }

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
                    let config_snapshot = config.read().await.clone();
                    match alchemist::system::watcher::resolve_watch_paths(
                        db.as_ref(),
                        &config_snapshot,
                        setup_mode,
                    )
                    .await
                    {
                        Ok(all_dirs) => {
                            info!("Updating file watcher with {} directories", all_dirs.len());
                            if let Err(e) = file_watcher.watch(&all_dirs) {
                                error!("Failed to update file watcher: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to resolve watch dirs: {}", e);
                            if let Err(stop_err) = file_watcher.watch(&[]) {
                                debug!("Watcher stop after resolution failure: {}", stop_err);
                            }
                        }
                    }
                }
            }
        };

        // Initial Watcher Load (async to reduce boot latency)
        let reload_setup_mode = setup_mode;
        let reload_watcher_task = reload_watcher.clone();
        tokio::spawn(async move {
            let watcher_start = Instant::now();
            reload_watcher_task(reload_setup_mode).await;
            info!(
                target: "startup",
                "Initial file watcher load completed in {} ms",
                watcher_start.elapsed().as_millis()
            );
        });

        // Start Scheduler
        let scheduler = alchemist::scheduler::Scheduler::new(db.clone(), agent.clone());
        let scheduler_handle = scheduler.start();

        // Async Config Watcher
        let config_watcher_arc = config.clone();
        let reload_watcher_clone = reload_watcher.clone();
        let agent_for_config = agent.clone();
        let hardware_state_for_config = hardware_state.clone();
        let hardware_probe_log_for_config = hardware_probe_log.clone();
        let config_watch_path = config_path.clone();
        let db_for_config = db.clone();

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
                let watch_target = config_watch_target(config_watch_path.as_path()).to_path_buf();
                if let Err(e) = watcher.watch(watch_target.as_path(), RecursiveMode::NonRecursive) {
                    error!(
                        "Failed to watch config path {:?} via {:?}: {}",
                        config_watch_path, watch_target, e
                    );
                } else {
                    // Prevent watcher from dropping by keeping it in the spawn if needed,
                    // or just spawning the processing loop.
                    // notify watcher works in background thread usually.
                    // We need to keep `watcher` alive.

                    tokio::spawn(async move {
                        // Keep watcher alive by moving it here
                        let _watcher = watcher;

                        while let Some(event) = rx_notify.recv().await {
                            if should_reload_config_for_event(&event, config_watch_path.as_path()) {
                                info!("Config file changed ({:?}). Reloading...", &event.kind);
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                                match apply_reloaded_config(
                                    &db_for_config,
                                    config_watch_path.as_path(),
                                    &config_watcher_arc,
                                    &agent_for_config,
                                    &hardware_state_for_config,
                                    &hardware_probe_log_for_config,
                                )
                                .await
                                {
                                    Ok(detected_hardware) => {
                                        info!("Configuration reloaded successfully.");
                                        info!(
                                            "Runtime hardware reloaded: {}",
                                            detected_hardware.vendor
                                        );
                                        reload_watcher_clone(false).await;
                                    }
                                    Err(e) => {
                                        error!("Failed to reload config: {}", e);
                                    }
                                }
                            }
                        }
                    });
                }
            }
            Err(e) => error!("Failed to create config watcher: {}", e),
        }

        info!(
            target: "startup",
            "Boot sequence completed in {} ms",
            boot_start.elapsed().as_millis()
        );
        alchemist::server::run_server(alchemist::server::RunServerArgs {
            db,
            config,
            agent,
            transcoder,
            scheduler: scheduler_handle,
            event_channels,
            tx,
            setup_required: setup_mode,
            config_path: config_path.clone(),
            config_mutable,
            hardware_state,
            hardware_probe_log,
            notification_manager: notification_manager.clone(),
            file_watcher,
            library_scanner,
        })
        .await?;
    } else {
        // CLI Mode
        if setup_mode {
            error!(
                "Configuration required. Run without --cli to use the web-based setup wizard, or create {:?} manually.",
                config_path
            );

            // CLI early exit - error
            // (Caller will handle pause-on-exit if needed)
            return Err(alchemist::error::AlchemistError::Config(
                "Missing configuration".into(),
            ));
        }

        if args.directories.is_empty() {
            error!("No directories provided. Usage: alchemist --cli --dir <DIR> [--dir <DIR> ...]");
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
                            ["encoding", "analyzing", "remuxing", "resuming"].contains(&k.as_str())
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

#[cfg(test)]
mod version_cli_tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_command_uses_runtime_version_source() {
        let command = Args::command();
        let version = command.get_version().unwrap_or_default();
        assert_eq!(version, version::current());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use notify::{
        Event, EventKind,
        event::{CreateKind, ModifyKind, RenameMode},
    };
    fn temp_db_path(prefix: &str) -> PathBuf {
        let mut db_path = std::env::temp_dir();
        db_path.push(format!("{prefix}_{}.db", rand::random::<u64>()));
        db_path
    }

    fn temp_config_path(prefix: &str) -> PathBuf {
        let mut config_path = std::env::temp_dir();
        config_path.push(format!("{prefix}_{}.toml", rand::random::<u64>()));
        config_path
    }

    #[test]
    fn args_reject_removed_output_dir_flag() {
        assert!(Args::try_parse_from(["alchemist", "--output-dir", "/tmp/out"]).is_err());
    }

    #[test]
    fn config_reload_matches_create_modify_and_rename_events() {
        let config_path = PathBuf::from("/tmp/alchemist-config.toml");

        let create = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![config_path.clone()],
            attrs: Default::default(),
        };
        assert!(should_reload_config_for_event(&create, &config_path));

        let rename = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            paths: vec![
                PathBuf::from("/tmp/alchemist-config.toml.tmp"),
                config_path.clone(),
            ],
            attrs: Default::default(),
        };
        assert!(should_reload_config_for_event(&rename, &config_path));

        let unrelated = Event {
            kind: EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![PathBuf::from("/tmp/other.toml")],
            attrs: Default::default(),
        };
        assert!(!should_reload_config_for_event(&unrelated, &config_path));

        assert_eq!(
            config_watch_target(config_path.as_path()),
            Path::new("/tmp")
        );
    }

    #[tokio::test]
    async fn config_reload_refreshes_runtime_hardware_state()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("alchemist_config_reload");
        let config_path = temp_config_path("alchemist_config_reload");
        let db = Arc::new(db::Db::new(db_path.to_string_lossy().as_ref()).await?);

        let initial_config = config::Config::default();
        initial_config.save(&config_path)?;
        let config_state = Arc::new(RwLock::new(initial_config.clone()));
        let hardware_state = hardware::HardwareState::new(Some(hardware::HardwareInfo {
            vendor: hardware::Vendor::Nvidia,
            device_path: None,
            supported_codecs: vec!["av1".to_string()],
            backends: Vec::new(),
            detection_notes: Vec::new(),
            selection_reason: String::new(),
            probe_summary: hardware::ProbeSummary::default(),
        }));
        let hardware_probe_log = Arc::new(RwLock::new(hardware::HardwareProbeLog::default()));
        let transcoder = Arc::new(Transcoder::new());
        let (tx, _rx) = broadcast::channel(8);
        let (jobs_tx, _) = broadcast::channel(100);
        let (config_tx, _) = broadcast::channel(10);
        let (system_tx, _) = broadcast::channel(10);
        let event_channels = Arc::new(EventChannels {
            jobs: jobs_tx,
            config: config_tx,
            system: system_tx,
        });
        let agent = Arc::new(
            Agent::new(
                db.clone(),
                transcoder,
                config_state.clone(),
                hardware_state.clone(),
                tx,
                event_channels,
                true,
            )
            .await,
        );

        let mut reloaded_config = initial_config;
        reloaded_config.hardware.preferred_vendor = Some("cpu".to_string());
        reloaded_config.hardware.allow_cpu_fallback = true;
        reloaded_config.hardware.allow_cpu_encoding = true;
        reloaded_config.transcode.concurrent_jobs = 2;
        reloaded_config.save(&config_path)?;

        let detected = apply_reloaded_config(
            &db,
            config_path.as_path(),
            &config_state,
            &agent,
            &hardware_state,
            &hardware_probe_log,
        )
        .await?;

        assert_eq!(detected.vendor, hardware::Vendor::Cpu);
        assert_eq!(
            hardware_state.snapshot().await.unwrap().vendor,
            hardware::Vendor::Cpu
        );

        let config_guard = config_state.read().await;
        assert_eq!(
            config_guard.hardware.preferred_vendor.as_deref(),
            Some("cpu")
        );
        assert_eq!(config_guard.transcode.concurrent_jobs, 2);
        drop(config_guard);

        drop(agent);
        drop(db);
        let _ = std::fs::remove_file(config_path);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
