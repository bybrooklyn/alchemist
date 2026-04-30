#![deny(clippy::expect_used, clippy::unwrap_used)]

use alchemist::db::{EventChannels, SystemEvent};
use alchemist::error::Result;
use alchemist::media::pipeline::Planner as _;
use alchemist::system::hardware;
use alchemist::version;
use alchemist::{Agent, Transcoder, config, db, runtime};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::time;

use notify::{RecursiveMode, Watcher};
use tokio::sync::RwLock;
use tokio::sync::broadcast;

#[derive(Parser, Debug)]
#[command(author, version = version::current(), about, long_about = None)]
struct Args {
    /// Reset admin user/password and sessions (forces setup mode)
    #[arg(long)]
    reset_auth: bool,

    /// Enable verbose terminal logging and default DEBUG filtering
    #[arg(long)]
    debug_flags: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Scan directories and enqueue matching work, then exit
    Scan {
        #[arg(value_name = "DIR", required = true)]
        directories: Vec<PathBuf>,
    },
    /// Scan directories, enqueue work, and wait for processing to finish
    Run {
        #[arg(value_name = "DIR", required = true)]
        directories: Vec<PathBuf>,
        /// Don't actually transcode
        #[arg(short, long)]
        dry_run: bool,
    },
    /// Analyze files and report what Alchemist would do without enqueuing jobs
    Plan {
        #[arg(value_name = "DIR", required = true)]
        directories: Vec<PathBuf>,
        /// Emit machine-readable JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Serialize)]
struct CliPlanItem {
    input_path: String,
    output_path: Option<String>,
    profile: Option<String>,
    decision: String,
    reason: String,
    encoder: Option<String>,
    backend: Option<String>,
    rate_control: Option<String>,
    fallback: Option<String>,
    error: Option<String>,
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
    event_channels: &Arc<EventChannels>,
) -> Result<hardware::HardwareInfo> {
    let new_config = config::Config::load(config_path)
        .map_err(|err| alchemist::error::AlchemistError::Config(err.to_string()))?;
    let (detected_hardware, probe_log) = hardware::detect_hardware_with_log(&new_config).await?;
    let new_limit = new_config.transcode.concurrent_jobs;
    alchemist::settings::project_config_to_db(db.as_ref(), &new_config).await?;
    persist_hardware_detection_cache(db.as_ref(), &new_config, &detected_hardware, &probe_log)
        .await;

    {
        let mut config_guard = config_state.write().await;
        *config_guard = new_config;
    }

    hardware_state
        .replace(Some(detected_hardware.clone()))
        .await;
    *hardware_probe_log.write().await = probe_log.clone();
    let _ = event_channels
        .system
        .send(SystemEvent::HardwareStateChanged);
    agent.set_concurrent_jobs(new_limit).await;

    Ok(detected_hardware)
}

fn detection_config_for_mode(config: &config::Config, setup_mode: bool) -> config::Config {
    let mut detection_config = config.clone();
    if setup_mode {
        detection_config.hardware.allow_cpu_fallback = true;
    }
    detection_config
}

async fn persist_hardware_detection_cache(
    db: &db::Db,
    detection_config: &config::Config,
    hardware_info: &hardware::HardwareInfo,
    probe_log: &hardware::HardwareProbeLog,
) {
    match hardware::hardware_detection_cache_key_and_json(detection_config).await {
        Ok((cache_key, fingerprint_json)) => {
            if let Err(err) = db
                .upsert_hardware_detection_cache(
                    &cache_key,
                    &fingerprint_json,
                    hardware_info,
                    probe_log,
                )
                .await
            {
                warn!("Failed to persist hardware detection cache: {err}");
            }
        }
        Err(err) => warn!("Failed to build hardware detection cache key: {err}"),
    }
}

async fn load_cached_hardware_detection(
    db: &db::Db,
    detection_config: &config::Config,
    setup_mode: bool,
) -> (Option<hardware::HardwareInfo>, hardware::HardwareProbeLog) {
    let cache_key = match hardware::hardware_detection_cache_key_and_json(detection_config).await {
        Ok((cache_key, _)) => cache_key,
        Err(err) => {
            warn!("Failed to build hardware detection cache key: {err}");
            return (None, hardware::HardwareProbeLog::default());
        }
    };

    match db.get_hardware_detection_cache(&cache_key).await {
        Ok(Some(entry)) => {
            if !setup_mode
                && entry.hardware_info.vendor == hardware::Vendor::Cpu
                && !detection_config.hardware.allow_cpu_encoding
            {
                warn!("Ignoring cached CPU hardware state because CPU encoding is disabled.");
                return (None, hardware::HardwareProbeLog::default());
            }
            info!(
                target: "startup",
                "Loaded cached hardware detection from {}",
                entry.detected_at
            );
            (Some(entry.hardware_info), entry.probe_log)
        }
        Ok(None) => (None, hardware::HardwareProbeLog::default()),
        Err(err) => {
            warn!("Failed to load hardware detection cache: {err}");
            (None, hardware::HardwareProbeLog::default())
        }
    }
}

async fn detect_and_publish_hardware(
    db: Arc<db::Db>,
    detection_config: config::Config,
    hardware_state: hardware::HardwareState,
    hardware_probe_log: Arc<RwLock<hardware::HardwareProbeLog>>,
    event_channels: Arc<EventChannels>,
) -> Result<hardware::HardwareInfo> {
    let hw_start = Instant::now();
    let (detected_hardware, probe_log) =
        hardware::detect_hardware_with_log(&detection_config).await?;
    info!(
        target: "startup",
        "Hardware detection completed in {} ms",
        hw_start.elapsed().as_millis()
    );
    info!("Selected Hardware: {}", detected_hardware.vendor);
    if let Some(ref path) = detected_hardware.device_path {
        info!("  Device Path: {}", path);
    }

    hardware_state
        .replace(Some(detected_hardware.clone()))
        .await;
    *hardware_probe_log.write().await = probe_log.clone();
    persist_hardware_detection_cache(
        db.as_ref(),
        &detection_config,
        &detected_hardware,
        &probe_log,
    )
    .await;
    alchemist::media::ffmpeg::warm_encoder_cache();
    let _ = event_channels
        .system
        .send(SystemEvent::HardwareStateChanged);

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

fn load_startup_config(config_path: &Path, is_server_mode: bool) -> (config::Config, bool, bool) {
    let config_exists = config_path.exists();
    let (config, setup_mode) = if !config_exists {
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
            warn!("No configuration file found. Using defaults.");
            (config::Config::default(), false)
        }
    } else {
        match config::Config::load(config_path) {
            Ok(c) => (c, false),
            Err(e) => {
                warn!(
                    "Failed to load config file at {:?}: {}. Using defaults.",
                    config_path, e
                );
                if is_server_mode {
                    warn!(
                        "Config load failed in server mode. \
                         Will check for existing users before \
                         entering Setup Mode."
                    );
                    (config::Config::default(), false)
                } else {
                    (config::Config::default(), false)
                }
            }
        }
    };

    (config, setup_mode, config_exists)
}

fn should_enter_setup_mode_for_missing_users(is_server_mode: bool, has_users: bool) -> bool {
    is_server_mode && !has_users
}

async fn run() -> Result<()> {
    let args = Args::parse();
    init_logging(args.debug_flags);
    let is_server_mode = args.command.is_none();

    let boot_start = Instant::now();

    info!(
        target: "startup",
        "Parsed CLI args: command={:?}, reset_auth={}, debug_flags={}",
        args.command,
        args.reset_auth,
        args.debug_flags
    );

    if is_server_mode {
        info!("▄▖▜   ▌      ▘  ▗ ");
        info!("▌▌▐ ▛▘▛▌█▌▛▛▌▌▛▘▜▘");
        info!("▛▌▐▖▙▖▌▌▙▖▌▌▌▌▄▌▐▖");
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
    }

    info!(target: "startup", "Resolved server mode: {}", is_server_mode);

    // 0. Load Configuration
    let config_start = Instant::now();
    let config_path = runtime::config_path();
    let db_path = runtime::db_path();
    let config_mutable = runtime::config_mutable();
    let (config, mut setup_mode, config_exists) = if is_server_mode {
        load_startup_config(config_path.as_path(), true)
    } else {
        if !config_path.exists() {
            error!(
                "Configuration required. Run Alchemist in server mode to complete setup, or create {:?} manually.",
                config_path
            );
            return Err(alchemist::error::AlchemistError::Config(
                "Missing configuration".into(),
            ));
        }
        let config = config::Config::load(config_path.as_path())
            .map_err(|err| alchemist::error::AlchemistError::Config(err.to_string()))?;
        (config, false, true)
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
        match db.get_jobs_by_status(db::JobState::Resuming).await {
            Ok(mut resuming_jobs) => jobs.append(&mut resuming_jobs),
            Err(err) => error!("Failed to load interrupted resuming jobs: {}", err),
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
                let has_resume_session =
                    db.get_resume_session(job.id).await.ok().flatten().is_some();
                if has_resume_session {
                    continue;
                }
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

    // Also clean up any temp files left by cancelled jobs
    // (process was killed before runtime cleanup could run)
    match db.get_jobs_by_status(db::JobState::Cancelled).await {
        Ok(cancelled_jobs) => {
            for job in cancelled_jobs {
                let temp_path = orphaned_temp_output_path(&job.output_path);
                if std::fs::metadata(&temp_path).is_ok() {
                    match std::fs::remove_file(&temp_path) {
                        Ok(_) => warn!(
                            "Removed orphaned temp file \
                             from cancelled job: {}",
                            temp_path.display()
                        ),
                        Err(err) => error!(
                            "Failed to remove cancelled \
                             job temp file {}: {}",
                            temp_path.display(),
                            err
                        ),
                    }
                }
                // Also check for subtitle sidecar temps
                // Pattern: output_path + ".alchemist-part"
                // and output_path + ".N.alchemist-part"
                let sidecar_glob = format!("{}*.alchemist-part", job.output_path);
                // Use glob-style scan: check parent dir
                // for files matching the pattern
                if let Some(parent) = std::path::Path::new(&job.output_path).parent() {
                    if let Ok(entries) = std::fs::read_dir(parent) {
                        for entry in entries.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if name.ends_with(".alchemist-part") {
                                let path = entry.path();
                                match std::fs::remove_file(&path) {
                                    Ok(_) => warn!(
                                        "Removed orphaned \
                                         subtitle sidecar: {}",
                                        path.display()
                                    ),
                                    Err(err) => error!(
                                        "Failed to remove \
                                         sidecar {}: {}",
                                        path.display(),
                                        err
                                    ),
                                }
                            }
                        }
                    }
                }
                drop(sidecar_glob);
            }
        }
        Err(err) => error!(
            "Failed to load cancelled jobs for \
             cleanup: {}",
            err
        ),
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
    let has_users = db.has_users().await?;
    if is_server_mode {
        let users_start = Instant::now();
        info!(
            target: "startup",
            "User check completed (has_users={}) in {} ms",
            has_users,
            users_start.elapsed().as_millis()
        );
        if should_enter_setup_mode_for_missing_users(is_server_mode, has_users) {
            if !setup_mode {
                info!("No users found. Entering Setup Mode (Web UI).");
            }
            setup_mode = true;
        }
    } else if !has_users {
        error!(
            "Setup is not complete. Run Alchemist in server mode to finish creating the first account."
        );
        return Err(alchemist::error::AlchemistError::Config(
            "Setup incomplete".into(),
        ));
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

    // 2. Hardware Detection
    let detection_config = detection_config_for_mode(&config, setup_mode);
    let (initial_hardware_info, initial_probe_log) = if is_server_mode {
        let (cached_hardware, cached_probe_log) =
            load_cached_hardware_detection(db.as_ref(), &detection_config, setup_mode).await;
        match cached_hardware.as_ref() {
            Some(info) => {
                info!(
                    "Using cached hardware while live detection runs: {}",
                    info.vendor
                );
                if let Some(ref path) = info.device_path {
                    info!("  Device Path: {}", path);
                }
            }
            None => {
                info!("Hardware detection pending; starting web server before live probing.");
            }
        }
        (cached_hardware, cached_probe_log)
    } else {
        let hw_start = Instant::now();
        let (detected_hardware, probe_log) =
            hardware::detect_hardware_with_log(&detection_config).await?;
        info!(
            target: "startup",
            "Hardware detection completed in {} ms",
            hw_start.elapsed().as_millis()
        );
        info!("Selected Hardware: {}", detected_hardware.vendor);
        if let Some(ref path) = detected_hardware.device_path {
            info!("  Device Path: {}", path);
        }
        alchemist::media::ffmpeg::warm_encoder_cache();
        (Some(detected_hardware), probe_log)
    };

    // Check CPU encoding policy
    if !setup_mode
        && initial_hardware_info
            .as_ref()
            .is_some_and(|info| info.vendor == hardware::Vendor::Cpu)
    {
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

    let transcoder = Arc::new(Transcoder::new());
    let hardware_state = hardware::HardwareState::new(initial_hardware_info);
    let hardware_probe_log = Arc::new(RwLock::new(initial_probe_log));
    let config = Arc::new(RwLock::new(config));

    if is_server_mode {
        let detection_db = db.clone();
        let detection_config = detection_config.clone();
        let detection_hardware_state = hardware_state.clone();
        let detection_probe_log = hardware_probe_log.clone();
        let detection_events = event_channels.clone();
        tokio::spawn(async move {
            info!("Hardware detection running in background.");
            if let Err(err) = detect_and_publish_hardware(
                detection_db,
                detection_config,
                detection_hardware_state,
                detection_probe_log,
                detection_events,
            )
            .await
            {
                error!("Background hardware detection failed: {err}");
            }
        });
    }

    // Initialize Notification Manager (needs config for allow_local_notifications)
    let notification_manager = Arc::new(alchemist::notifications::NotificationManager::new(
        db.as_ref().clone(),
        config.clone(),
    ));
    notification_manager.start_listener(&event_channels);

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
            event_channels.clone(),
            matches!(args.command, Some(Commands::Run { dry_run: true, .. })),
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
        let file_watcher = Arc::new(alchemist::system::watcher::FileWatcher::new(
            db.clone(),
            Some(agent.clone()),
        ));

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
                scan_agent.analyze_pending_jobs_boot().await;
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
        let event_channels_for_config = event_channels.clone();

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
                                    &event_channels_for_config,
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
        let library_intelligence_cache = Arc::new(tokio::sync::Mutex::new(None));
        let library_health_scan_in_progress = Arc::new(AtomicBool::new(false));
        let server_result = alchemist::server::run_server(alchemist::server::RunServerArgs {
            db,
            config,
            agent,
            transcoder,
            scheduler: scheduler_handle,
            event_channels,
            setup_required: setup_mode,
            config_path: config_path.clone(),
            config_mutable,
            hardware_state,
            hardware_probe_log,
            notification_manager: notification_manager.clone(),
            file_watcher,
            library_scanner,
            library_intelligence_cache,
            library_health_scan_in_progress,
        })
        .await;

        // Background tasks (run_loop, scheduler, watcher,
        // maintenance) have no shutdown signal and run
        // forever. After run_server returns, graceful
        // shutdown is complete — all jobs are drained
        // and FFmpeg processes are cancelled. Exit cleanly.
        match server_result {
            Ok(()) => {
                info!("Server shutdown complete. Exiting.");
                std::process::exit(0);
            }
            Err(e) => {
                error!("Server exited with error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        let command = match args.command.clone() {
            Some(command) => command,
            None => {
                return Err(alchemist::error::AlchemistError::Config(
                    "Missing CLI command".into(),
                ));
            }
        };

        match command {
            Commands::Scan { directories } => {
                agent.scan_and_enqueue(directories).await?;
                info!("Scan complete. Matching files were enqueued.");
            }
            Commands::Run { directories, .. } => {
                agent.scan_and_enqueue(directories).await?;
                wait_for_cli_jobs(db.as_ref()).await?;
                info!("All jobs processed.");
            }
            Commands::Plan { directories, json } => {
                let items =
                    build_cli_plan(db.as_ref(), config.clone(), &hardware_state, directories)
                        .await?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string())
                    );
                } else {
                    print_cli_plan(&items);
                }
            }
        }
    }

    Ok(())
}

async fn wait_for_cli_jobs(db: &db::Db) -> Result<()> {
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
    Ok(())
}

async fn build_cli_plan(
    db: &db::Db,
    config_state: Arc<RwLock<config::Config>>,
    hardware_state: &hardware::HardwareState,
    directories: Vec<PathBuf>,
) -> Result<Vec<CliPlanItem>> {
    let files = tokio::task::spawn_blocking(move || {
        let scanner = alchemist::media::scanner::Scanner::new();
        scanner.scan(directories)
    })
    .await
    .map_err(|err| alchemist::error::AlchemistError::Unknown(format!("scan task failed: {err}")))?;

    let file_settings = match db.get_file_settings().await {
        Ok(settings) => settings,
        Err(err) => {
            error!("Failed to fetch file settings, using defaults: {}", err);
            alchemist::media::pipeline::default_file_settings()
        }
    };
    let config_snapshot = Arc::new(config_state.read().await.clone());
    let hw_info = hardware_state.snapshot().await;
    let planner = alchemist::media::planner::BasicPlanner::new(config_snapshot, hw_info);
    let analyzer = alchemist::media::analyzer::FfmpegAnalyzer;

    let mut items = Vec::new();
    for discovered in files {
        let input_path = discovered.path.clone();
        let input_path_string = input_path.display().to_string();

        if let Some(reason) = alchemist::media::pipeline::skip_reason_for_discovered_path(
            db,
            &input_path,
            &file_settings,
        )
        .await?
        {
            items.push(CliPlanItem {
                input_path: input_path_string,
                output_path: None,
                profile: None,
                decision: "skip".to_string(),
                reason: reason.to_string(),
                encoder: None,
                backend: None,
                rate_control: None,
                fallback: None,
                error: None,
            });
            continue;
        }

        let output_path =
            file_settings.output_path_for_source(&input_path, discovered.source_root.as_deref());
        if output_path.exists() && !file_settings.should_replace_existing_output() {
            items.push(CliPlanItem {
                input_path: input_path_string,
                output_path: Some(output_path.display().to_string()),
                profile: None,
                decision: "skip".to_string(),
                reason: "output exists and replace strategy is keep".to_string(),
                encoder: None,
                backend: None,
                rate_control: None,
                fallback: None,
                error: None,
            });
            continue;
        }

        let analysis = match analyzer.analyze_with_cache(db, &input_path).await {
            Ok(analysis) => analysis,
            Err(err) => {
                items.push(CliPlanItem {
                    input_path: input_path_string,
                    output_path: Some(output_path.display().to_string()),
                    profile: None,
                    decision: "error".to_string(),
                    reason: "analysis failed".to_string(),
                    encoder: None,
                    backend: None,
                    rate_control: None,
                    fallback: None,
                    error: Some(err.to_string()),
                });
                continue;
            }
        };

        let profile = match db.get_profile_for_path(&input_path.to_string_lossy()).await {
            Ok(profile) => profile,
            Err(err) => {
                items.push(CliPlanItem {
                    input_path: input_path_string,
                    output_path: Some(output_path.display().to_string()),
                    profile: None,
                    decision: "error".to_string(),
                    reason: "profile resolution failed".to_string(),
                    encoder: None,
                    backend: None,
                    rate_control: None,
                    fallback: None,
                    error: Some(err.to_string()),
                });
                continue;
            }
        };

        let plan = match planner
            .plan(&analysis, &output_path, profile.as_ref())
            .await
        {
            Ok(plan) => plan,
            Err(err) => {
                items.push(CliPlanItem {
                    input_path: input_path_string,
                    output_path: Some(output_path.display().to_string()),
                    profile: profile.as_ref().map(|p| p.name.clone()),
                    decision: "error".to_string(),
                    reason: "planning failed".to_string(),
                    encoder: None,
                    backend: None,
                    rate_control: None,
                    fallback: None,
                    error: Some(err.to_string()),
                });
                continue;
            }
        };

        let (decision, reason) = match &plan.decision {
            alchemist::media::pipeline::TranscodeDecision::Skip { reason } => {
                ("skip".to_string(), reason.clone())
            }
            alchemist::media::pipeline::TranscodeDecision::Remux { reason } => {
                ("remux".to_string(), reason.clone())
            }
            alchemist::media::pipeline::TranscodeDecision::Transcode { reason } => {
                ("transcode".to_string(), reason.clone())
            }
        };

        items.push(CliPlanItem {
            input_path: input_path_string,
            output_path: Some(output_path.display().to_string()),
            profile: profile.as_ref().map(|p| p.name.clone()),
            decision,
            reason,
            encoder: plan
                .encoder
                .map(|encoder| encoder.ffmpeg_encoder_name().to_string()),
            backend: plan.backend.map(|backend| backend.as_str().to_string()),
            rate_control: plan.rate_control.as_ref().map(format_rate_control),
            fallback: plan
                .fallback
                .as_ref()
                .map(|fallback| fallback.reason.clone()),
            error: None,
        });
    }

    Ok(items)
}

fn format_rate_control(rate_control: &alchemist::media::pipeline::RateControl) -> String {
    match rate_control {
        alchemist::media::pipeline::RateControl::Crf { value } => format!("crf:{value}"),
        alchemist::media::pipeline::RateControl::Cq { value } => format!("cq:{value}"),
        alchemist::media::pipeline::RateControl::QsvQuality { value } => {
            format!("qsv_quality:{value}")
        }
        alchemist::media::pipeline::RateControl::Bitrate { kbps } => format!("bitrate:{kbps}k"),
    }
}

fn print_cli_plan(items: &[CliPlanItem]) {
    for item in items {
        println!("{}", item.input_path);
        println!("  decision: {} — {}", item.decision, item.reason);
        if let Some(output_path) = &item.output_path {
            println!("  output:   {}", output_path);
        }
        if let Some(profile) = &item.profile {
            println!("  profile:  {}", profile);
        }
        if let Some(encoder) = &item.encoder {
            let backend = item.backend.as_deref().unwrap_or("unknown");
            println!("  encoder:  {} ({})", encoder, backend);
        }
        if let Some(rate_control) = &item.rate_control {
            println!("  rate:     {}", rate_control);
        }
        if let Some(fallback) = &item.fallback {
            println!("  fallback: {}", fallback);
        }
        if let Some(error) = &item.error {
            println!("  error:    {}", error);
        }
        println!();
    }
}

fn init_logging(debug_flags: bool) {
    let default_level = if debug_flags {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    let env_filter = EnvFilter::from_default_env().add_directive(default_level.into());

    if debug_flags {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_timer(time())
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .without_time()
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(false)
            .compact()
            .init();
    }
}

#[cfg(test)]
mod logging_tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn debug_flags_arg_parses() {
        let args = Args::try_parse_from(["alchemist", "--debug-flags"])
            .unwrap_or_else(|err| panic!("failed to parse debug flag: {err}"));
        assert!(args.debug_flags);
    }
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
    fn args_reject_removed_cli_flag() {
        assert!(Args::try_parse_from(["alchemist", "--cli"]).is_err());
    }

    #[test]
    fn scan_subcommand_parses() {
        let args = Args::try_parse_from(["alchemist", "scan", "/tmp/media"])
            .unwrap_or_else(|err| panic!("failed to parse scan subcommand: {err}"));
        assert!(matches!(
            args.command,
            Some(Commands::Scan { directories }) if directories == vec![PathBuf::from("/tmp/media")]
        ));
    }

    #[test]
    fn run_subcommand_parses_with_dry_run() {
        let args = Args::try_parse_from(["alchemist", "run", "/tmp/media", "--dry-run"])
            .unwrap_or_else(|err| panic!("failed to parse run subcommand: {err}"));
        assert!(matches!(
            args.command,
            Some(Commands::Run { directories, dry_run }) if directories == vec![PathBuf::from("/tmp/media")] && dry_run
        ));
    }

    #[test]
    fn plan_subcommand_parses_with_json() {
        let args = Args::try_parse_from(["alchemist", "plan", "/tmp/media", "--json"])
            .unwrap_or_else(|err| panic!("failed to parse plan subcommand: {err}"));
        assert!(matches!(
            args.command,
            Some(Commands::Plan { directories, json }) if directories == vec![PathBuf::from("/tmp/media")] && json
        ));
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
    async fn invalid_config_with_existing_users_does_not_reenter_setup_mode()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("alchemist_invalid_config_users");
        let config_path = temp_config_path("alchemist_invalid_config_users");
        std::fs::write(&config_path, "not-valid = [")?;

        let db = db::Db::new(db_path.to_string_lossy().as_ref()).await?;
        db.create_user("admin", "hash").await?;

        let (_config, setup_mode, config_exists) = load_startup_config(config_path.as_path(), true);
        let has_users = db.has_users().await?;
        let final_setup_mode =
            setup_mode || should_enter_setup_mode_for_missing_users(true, has_users);

        assert!(config_exists);
        assert!(has_users);
        assert!(!setup_mode);
        assert!(!final_setup_mode);

        let _ = std::fs::remove_file(config_path);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn invalid_config_without_users_still_enters_setup_mode()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("alchemist_invalid_config_setup");
        let config_path = temp_config_path("alchemist_invalid_config_setup");
        std::fs::write(&config_path, "not-valid = [")?;

        let db = db::Db::new(db_path.to_string_lossy().as_ref()).await?;

        let (_config, setup_mode, config_exists) = load_startup_config(config_path.as_path(), true);
        let has_users = db.has_users().await?;
        let final_setup_mode =
            setup_mode || should_enter_setup_mode_for_missing_users(true, has_users);

        assert!(config_exists);
        assert!(!has_users);
        assert!(!setup_mode);
        assert!(final_setup_mode);

        let _ = std::fs::remove_file(config_path);
        let _ = std::fs::remove_file(db_path);
        Ok(())
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
                event_channels.clone(),
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
            &event_channels,
        )
        .await?;

        assert_eq!(detected.vendor, hardware::Vendor::Cpu);
        assert_eq!(
            hardware_state.snapshot().await.map(|info| info.vendor),
            Some(hardware::Vendor::Cpu)
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
