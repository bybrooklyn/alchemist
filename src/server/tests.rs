//! Server tests (all tests kept together for now).

#![cfg(test)]

use super::settings::TranscodeSettingsPayload;
use super::sse::sse_message_stream;
use super::wizard::normalize_setup_directories;
use super::*;
use crate::db::{AlchemistEvent, JobState};
use crate::system::hardware::{HardwareProbeLog, HardwareState};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, header},
};
use chrono::Utc;
use futures::StreamExt;
use http_body_util::BodyExt;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock, broadcast};
use tower::util::ServiceExt;

fn temp_path(prefix: &str, extension: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("{prefix}_{}.{}", rand::random::<u64>(), extension));
    path
}

async fn build_test_app<F>(
    setup_required: bool,
    tx_capacity: usize,
    configure: F,
) -> std::result::Result<(Arc<AppState>, Router, PathBuf, PathBuf), Box<dyn std::error::Error>>
where
    F: FnOnce(&mut crate::config::Config),
{
    use crate::{Agent, Transcoder, db::Db};

    let db_path = temp_path("alchemist_server_test", "db");
    let config_path = temp_path("alchemist_server_test", "toml");

    let mut config_value = crate::config::Config::default();
    configure(&mut config_value);
    config_value.save(&config_path)?;

    let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
    let config = Arc::new(RwLock::new(config_value));
    let hardware_state = HardwareState::new(Some(crate::system::hardware::HardwareInfo {
        vendor: crate::system::hardware::Vendor::Cpu,
        device_path: None,
        supported_codecs: vec!["av1".to_string(), "hevc".to_string(), "h264".to_string()],
        backends: Vec::new(),
        detection_notes: Vec::new(),
    }));
    let hardware_probe_log = Arc::new(RwLock::new(HardwareProbeLog::default()));
    let (tx, _rx) = broadcast::channel(tx_capacity);
    let transcoder = Arc::new(Transcoder::new());

    // Create event channels before Agent
    let (jobs_tx, _) = broadcast::channel(1000);
    let (config_tx, _) = broadcast::channel(50);
    let (system_tx, _) = broadcast::channel(100);
    let event_channels = Arc::new(crate::db::EventChannels {
        jobs: jobs_tx,
        config: config_tx,
        system: system_tx,
    });

    let agent = Arc::new(
        Agent::new(
            db.clone(),
            transcoder.clone(),
            config.clone(),
            hardware_state.clone(),
            tx.clone(),
            event_channels.clone(),
            true,
        )
        .await,
    );
    let scheduler = crate::scheduler::Scheduler::new(db.clone(), agent.clone());
    let file_watcher = Arc::new(crate::system::watcher::FileWatcher::new(db.clone()));

    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let state = Arc::new(AppState {
        db: db.clone(),
        config: config.clone(),
        agent,
        transcoder,
        scheduler: scheduler.handle(),
        event_channels,
        tx,
        setup_required: Arc::new(AtomicBool::new(setup_required)),
        start_time: Instant::now(),
        telemetry_runtime_id: "test-runtime".to_string(),
        notification_manager: Arc::new(crate::notifications::NotificationManager::new(
            db.as_ref().clone(),
        )),
        sys: Mutex::new(sys),
        file_watcher,
        library_scanner: Arc::new(crate::system::scanner::LibraryScanner::new(db, config)),
        config_path: config_path.clone(),
        config_mutable: true,
        hardware_state,
        hardware_probe_log,
        resources_cache: Arc::new(tokio::sync::Mutex::new(None)),
        login_rate_limiter: Mutex::new(HashMap::new()),
        global_rate_limiter: Mutex::new(HashMap::new()),
    });

    Ok((state.clone(), app_router(state), config_path, db_path))
}

async fn create_session(
    db: &crate::db::Db,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let user_id = db.create_user("tester", "hash").await?;
    let token = format!("test-session-{}", rand::random::<u64>());
    db.create_session(user_id, &token, Utc::now() + chrono::Duration::days(1))
        .await?;
    Ok(token)
}

fn auth_request(method: Method, uri: &str, token: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, format!("alchemist_session={token}"))
        .body(body)
        .unwrap()
}

fn auth_json_request(
    method: Method,
    uri: &str,
    token: &str,
    body: serde_json::Value,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, format!("alchemist_session={token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn seed_job(
    db: &crate::db::Db,
    status: JobState,
) -> std::result::Result<(crate::db::Job, PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let input = temp_path("alchemist_job_input", "mkv");
    let output = temp_path("alchemist_job_output", "mkv");
    std::fs::write(&input, b"test")?;

    db.enqueue_job(&input, &output, std::time::SystemTime::UNIX_EPOCH)
        .await?;
    let job = db
        .get_job_by_input_path(input.to_string_lossy().as_ref())
        .await?
        .expect("job");
    if job.status != status {
        db.update_job_status(job.id, status).await?;
    }

    let job = db.get_job_by_id(job.id).await?.expect("job by id");
    Ok((job, input, output))
}

fn cleanup_paths(paths: &[PathBuf]) {
    for path in paths {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(path);
    }
}

fn sample_transcode_payload() -> TranscodeSettingsPayload {
    TranscodeSettingsPayload {
        concurrent_jobs: 1,
        size_reduction_threshold: 0.3,
        min_bpp_threshold: 0.1,
        min_file_size_mb: 50,
        output_codec: crate::config::OutputCodec::Av1,
        quality_profile: crate::config::QualityProfile::Balanced,
        threads: 0,
        allow_fallback: true,
        hdr_mode: crate::config::HdrMode::Preserve,
        tonemap_algorithm: crate::config::TonemapAlgorithm::Hable,
        tonemap_peak: 100.0,
        tonemap_desat: 0.2,
        subtitle_mode: crate::config::SubtitleMode::Copy,
        stream_rules: crate::config::StreamRules::default(),
    }
}

#[test]
fn validate_transcode_payload_rejects_invalid_values() {
    let mut payload = sample_transcode_payload();
    payload.concurrent_jobs = 0;
    assert!(validate_transcode_payload(&payload).is_err());

    let mut payload = sample_transcode_payload();
    payload.size_reduction_threshold = 1.5;
    assert!(validate_transcode_payload(&payload).is_err());

    let mut payload = sample_transcode_payload();
    payload.tonemap_peak = 10.0;
    assert!(validate_transcode_payload(&payload).is_err());

    let mut payload = sample_transcode_payload();
    payload.tonemap_desat = 2.0;
    assert!(validate_transcode_payload(&payload).is_err());
}

#[test]
fn normalize_setup_directories_trims_and_filters() {
    let movies_dir = temp_path("alchemist_setup_movies", "dir");
    let tv_dir = temp_path("alchemist_setup_tv", "dir");
    std::fs::create_dir_all(&movies_dir).unwrap();
    std::fs::create_dir_all(&tv_dir).unwrap();

    let input = vec![
        format!(" {} ", movies_dir.to_string_lossy()),
        "".to_string(),
        "   ".to_string(),
        tv_dir.to_string_lossy().to_string(),
    ];

    let normalized = normalize_setup_directories(&input).expect("normalize");
    assert_eq!(
        normalized,
        vec![
            std::fs::canonicalize(&movies_dir)
                .unwrap()
                .to_string_lossy()
                .to_string(),
            std::fs::canonicalize(&tv_dir)
                .unwrap()
                .to_string_lossy()
                .to_string()
        ]
    );

    cleanup_paths(&[movies_dir, tv_dir]);
}

#[test]
fn config_write_blocked_returns_409() {
    let response = config_write_blocked_response(std::path::Path::new("/tmp/config.toml"));
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[test]
fn config_save_permission_error_maps_to_409() {
    let err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
    let response = config_save_error_to_response(
        std::path::Path::new("/tmp/config.toml"),
        &anyhow::Error::new(err),
    );
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[test]
fn config_save_other_errors_map_to_500() {
    let err = anyhow::anyhow!("something failed");
    let response = config_save_error_to_response(std::path::Path::new("/tmp/config.toml"), &err);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn sse_message_stream_emits_lagged_event_and_recovers() {
    let (tx, rx) = broadcast::channel(1);
    tx.send(AlchemistEvent::Log {
        level: "info".to_string(),
        job_id: None,
        message: "first".to_string(),
    })
    .unwrap();
    tx.send(AlchemistEvent::Log {
        level: "info".to_string(),
        job_id: None,
        message: "second".to_string(),
    })
    .unwrap();
    drop(tx);

    let mut stream = Box::pin(sse_message_stream(rx));
    let first = stream.next().await.unwrap().unwrap();
    assert_eq!(first.event_name, "lagged");
    assert!(first.data.contains("\"skipped\":1"));

    let second = stream.next().await.unwrap().unwrap();
    assert_eq!(second.event_name, "log");
    assert!(second.data.contains("\"second\""));
}

#[tokio::test]
async fn hardware_settings_route_updates_runtime_state()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;

    let response = app
        .clone()
        .oneshot(auth_json_request(
            Method::POST,
            "/api/settings/hardware",
            &token,
            json!({
                "allow_cpu_fallback": true,
                "allow_cpu_encoding": true,
                "cpu_preset": "medium",
                "preferred_vendor": "cpu",
                "device_path": null
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let hardware = state.hardware_state.snapshot().await.unwrap();
    assert_eq!(hardware.vendor, crate::system::hardware::Vendor::Cpu);

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::GET,
            "/api/system/hardware",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("\"vendor\":\"cpu\""));

    let persisted = crate::config::Config::load(config_path.as_path())?;
    assert_eq!(persisted.hardware.preferred_vendor.as_deref(), Some("cpu"));
    assert_eq!(persisted.hardware.device_path, None);

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn engine_mode_endpoint_applies_manual_override_and_persists_mode()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;

    let response = app
        .clone()
        .oneshot(auth_json_request(
            Method::POST,
            "/api/engine/mode",
            &token,
            json!({
                "mode": "throughput",
                "concurrent_jobs_override": 2,
                "threads_override": 3
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let payload: serde_json::Value = serde_json::from_str(&body_text(response).await)?;
    assert_eq!(payload["mode"], "throughput");
    assert_eq!(payload["concurrent_limit"], 2);
    assert_eq!(payload["is_manual_override"], true);

    assert_eq!(
        state.agent.current_mode().await,
        crate::config::EngineMode::Throughput
    );
    assert_eq!(state.agent.concurrent_jobs_limit(), 2);
    assert!(state.agent.is_manual_override());

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::GET,
            "/api/engine/mode",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let payload: serde_json::Value = serde_json::from_str(&body_text(response).await)?;
    assert_eq!(payload["mode"], "throughput");
    assert_eq!(payload["concurrent_limit"], 2);
    assert_eq!(payload["is_manual_override"], true);
    assert!(payload["cpu_count"].as_u64().unwrap_or(0) > 0);

    let persisted = crate::config::Config::load(config_path.as_path())?;
    assert_eq!(
        persisted.system.engine_mode,
        crate::config::EngineMode::Throughput
    );
    assert_eq!(persisted.transcode.threads, 3);

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn engine_status_endpoint_reports_draining_state()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;

    state.agent.pause();
    state.agent.set_scheduler_paused(true);
    state.agent.set_manual_override(true);
    state.agent.drain();

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::GET,
            "/api/engine/status",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let payload: serde_json::Value = serde_json::from_str(&body_text(response).await)?;
    assert_eq!(payload["status"], "draining");
    assert_eq!(payload["manual_paused"], true);
    assert_eq!(payload["scheduler_paused"], true);
    assert_eq!(payload["draining"], true);
    assert_eq!(payload["mode"], "balanced");
    assert_eq!(payload["concurrent_limit"], 1);
    assert_eq!(payload["is_manual_override"], true);

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn hardware_probe_log_route_returns_runtime_log()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;

    *state.hardware_probe_log.write().await = HardwareProbeLog {
        entries: vec![crate::system::hardware::HardwareProbeEntry {
            encoder: "hevc_videotoolbox".to_string(),
            backend: "videotoolbox".to_string(),
            device_path: None,
            success: false,
            stderr: Some("Unknown encoder".to_string()),
        }],
    };

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::GET,
            "/api/system/hardware/probe-log",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let body = body_text(response).await;
    assert!(body.contains("\"encoder\":\"hevc_videotoolbox\""));
    assert!(body.contains("\"stderr\":\"Unknown encoder\""));

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn setup_complete_updates_runtime_hardware_without_mirroring_watch_dirs()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let watch_dir = temp_path("alchemist_setup_watch", "dir");
    std::fs::create_dir_all(&watch_dir)?;

    let (state, app, config_path, db_path) = build_test_app(true, 8, |config| {
        config.hardware.preferred_vendor = Some("cpu".to_string());
    })
    .await?;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/setup/complete")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "username": "admin",
                        "password": "password123",
                        "size_reduction_threshold": 0.3,
                        "min_bpp_threshold": 0.1,
                        "min_file_size_mb": 50,
                        "concurrent_jobs": 1,
                        "directories": [watch_dir.to_string_lossy().to_string()],
                        "allow_cpu_encoding": true,
                        "enable_telemetry": false,
                        "output_codec": "av1",
                        "quality_profile": "balanced"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    assert!(
        !state
            .setup_required
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    assert_eq!(
        state.hardware_state.snapshot().await.unwrap().vendor,
        crate::system::hardware::Vendor::Cpu
    );

    let watch_dirs = state.db.get_watch_dirs().await?;
    assert!(watch_dirs.is_empty());

    let persisted = crate::config::Config::load(config_path.as_path())?;
    assert!(persisted.scanner.watch_enabled);
    assert_eq!(
        persisted.scanner.directories,
        vec![
            std::fs::canonicalize(&watch_dir)?
                .to_string_lossy()
                .to_string()
        ]
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/system/hardware")
                .header(header::COOKIE, set_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("\"vendor\":\"cpu\""));

    cleanup_paths(&[watch_dir, config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn setup_complete_accepts_nested_settings_payload()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let watch_dir = temp_path("alchemist_setup_nested_watch", "dir");
    std::fs::create_dir_all(&watch_dir)?;

    let (state, app, config_path, db_path) = build_test_app(true, 8, |config| {
        config.hardware.preferred_vendor = Some("cpu".to_string());
    })
    .await?;

    let mut settings = crate::config::Config::default();
    settings.transcode.concurrent_jobs = 3;
    settings.scanner.directories = vec![watch_dir.to_string_lossy().to_string()];
    settings.appearance.active_theme_id = Some("midnight".to_string());
    settings.notifications.targets = vec![crate::config::NotificationTargetConfig {
        name: "Discord".to_string(),
        target_type: "discord".to_string(),
        endpoint_url: "https://discord.com/api/webhooks/test".to_string(),
        auth_token: None,
        events: vec!["completed".to_string()],
        enabled: true,
    }];
    settings.schedule.windows = vec![crate::config::ScheduleWindowConfig {
        start_time: "22:00".to_string(),
        end_time: "06:00".to_string(),
        days_of_week: vec![1, 2, 3],
        enabled: true,
    }];

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/setup/complete")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "username": "admin",
                        "password": "password123",
                        "settings": settings,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        !state
            .setup_required
            .load(std::sync::atomic::Ordering::Relaxed)
    );

    let persisted = crate::config::Config::load(config_path.as_path())?;
    assert_eq!(
        persisted.appearance.active_theme_id.as_deref(),
        Some("midnight")
    );
    assert_eq!(persisted.notifications.targets.len(), 1);
    assert_eq!(persisted.schedule.windows.len(), 1);
    assert_eq!(persisted.transcode.concurrent_jobs, 3);
    assert_eq!(state.agent.concurrent_jobs_limit(), 3);

    cleanup_paths(&[watch_dir, config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn setup_complete_rejects_nested_settings_without_library_directories()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (_state, app, config_path, db_path) = build_test_app(true, 8, |_| {}).await?;

    let mut settings = crate::config::Config::default();
    settings.scanner.directories = Vec::new();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/setup/complete")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "username": "admin",
                        "password": "password123",
                        "settings": settings,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_text(response).await;
    assert!(body.contains("At least one library directory must be configured."));

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn fs_endpoints_are_available_during_setup()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let browse_root = temp_path("alchemist_fs_browse", "dir");
    std::fs::create_dir_all(&browse_root)?;
    let media_dir = browse_root.join("movies");
    std::fs::create_dir_all(&media_dir)?;
    std::fs::write(media_dir.join("movie.mkv"), b"video")?;

    let (_state, app, config_path, db_path) = build_test_app(true, 8, |_| {}).await?;

    let browse_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/api/fs/browse?path={}",
                    browse_root.to_string_lossy()
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert_eq!(browse_response.status(), StatusCode::OK);
    let browse_body = body_text(browse_response).await;
    assert!(browse_body.contains("movies"));

    let preview_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/fs/preview")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "directories": [browse_root.to_string_lossy().to_string()]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await?;
    assert_eq!(preview_response.status(), StatusCode::OK);
    let preview_body = body_text(preview_response).await;
    assert!(preview_body.contains("\"total_media_files\":1"));

    cleanup_paths(&[browse_root, config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn transcode_settings_round_trip_subtitle_mode()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::GET,
            "/api/settings/transcode",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("\"subtitle_mode\":\"copy\""));
    assert!(body.contains("\"stream_rules\""));

    let mut payload = sample_transcode_payload();
    payload.subtitle_mode = crate::config::SubtitleMode::None;
    payload.stream_rules = crate::config::StreamRules {
        strip_audio_by_title: vec!["commentary".to_string()],
        keep_audio_languages: vec!["eng".to_string()],
        keep_only_default_audio: false,
    };
    let response = app
        .clone()
        .oneshot(auth_json_request(
            Method::POST,
            "/api/settings/transcode",
            &token,
            serde_json::to_value(&payload)?,
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let persisted = crate::config::Config::load(config_path.as_path())?;
    assert_eq!(
        persisted.transcode.subtitle_mode,
        crate::config::SubtitleMode::None
    );
    assert_eq!(
        persisted.transcode.stream_rules.strip_audio_by_title,
        vec!["commentary".to_string()]
    );
    assert_eq!(
        persisted.transcode.stream_rules.keep_audio_languages,
        vec!["eng".to_string()]
    );

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn system_settings_round_trip_watch_enabled()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |config| {
        config.scanner.watch_enabled = true;
    })
    .await?;
    let token = create_session(state.db.as_ref()).await?;

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::GET,
            "/api/settings/system",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("\"watch_enabled\":true"));

    let response = app
        .clone()
        .oneshot(auth_json_request(
            Method::POST,
            "/api/settings/system",
            &token,
            json!({
                "monitoring_poll_interval": 2.0,
                "enable_telemetry": false,
                "watch_enabled": false
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let persisted = crate::config::Config::load(config_path.as_path())?;
    assert!(!persisted.scanner.watch_enabled);

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn settings_bundle_put_projects_extended_settings_to_db()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;

    let mut payload = crate::config::Config::default();
    payload.appearance.active_theme_id = Some("midnight".to_string());
    payload.scanner.extra_watch_dirs = vec![crate::config::WatchDirConfig {
        path: "/tmp/library".to_string(),
        is_recursive: true,
    }];
    payload.files.output_suffix = "-custom".to_string();
    payload.schedule.windows = vec![crate::config::ScheduleWindowConfig {
        start_time: "22:00".to_string(),
        end_time: "06:00".to_string(),
        days_of_week: vec![1, 2, 3],
        enabled: true,
    }];
    payload.notifications.enabled = true;
    payload.notifications.targets = vec![crate::config::NotificationTargetConfig {
        name: "Discord".to_string(),
        target_type: "discord".to_string(),
        endpoint_url: "https://discord.com/api/webhooks/test".to_string(),
        auth_token: None,
        events: vec!["completed".to_string()],
        enabled: true,
    }];

    let response = app
        .clone()
        .oneshot(auth_json_request(
            Method::PUT,
            "/api/settings/bundle",
            &token,
            serde_json::to_value(&payload)?,
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let watch_dirs = state.db.get_watch_dirs().await?;
    assert_eq!(watch_dirs.len(), 1);
    assert_eq!(watch_dirs[0].path, "/tmp/library");

    let file_settings = state.db.get_file_settings().await?;
    assert_eq!(file_settings.output_suffix, "-custom");

    let schedule = state.db.get_schedule_windows().await?;
    assert_eq!(schedule.len(), 1);

    let notifications = state.db.get_notification_targets().await?;
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].target_type, "discord");

    let theme = state.db.get_preference("active_theme_id").await?;
    assert_eq!(theme.as_deref(), Some("midnight"));

    let persisted = crate::config::Config::load(config_path.as_path())?;
    assert_eq!(
        persisted.appearance.active_theme_id.as_deref(),
        Some("midnight")
    );
    assert_eq!(persisted.files.output_suffix, "-custom");
    assert_eq!(persisted.scanner.extra_watch_dirs.len(), 1);

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn raw_config_put_overwrites_divergent_db_projection()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;

    state.db.add_watch_dir("/tmp/stale", true).await?;

    let mut payload = crate::config::Config::default();
    payload.appearance.active_theme_id = Some("ember".to_string());
    payload.files.output_extension = "mp4".to_string();
    let raw_toml = toml::to_string_pretty(&payload)?;

    let response = app
        .clone()
        .oneshot(auth_json_request(
            Method::PUT,
            "/api/settings/config",
            &token,
            json!({ "raw_toml": raw_toml }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let watch_dirs = state.db.get_watch_dirs().await?;
    assert!(watch_dirs.is_empty());
    let file_settings = state.db.get_file_settings().await?;
    assert_eq!(file_settings.output_extension, "mp4");
    let theme = state.db.get_preference("active_theme_id").await?;
    assert_eq!(theme.as_deref(), Some("ember"));

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn hardware_settings_get_exposes_configured_device_path()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let explicit_path = if cfg!(target_os = "linux") {
        "/dev/dri/renderD128".to_string()
    } else {
        "custom-device".to_string()
    };
    let (state, app, config_path, db_path) = build_test_app(false, 8, |config| {
        config.hardware.device_path = Some(explicit_path.clone());
    })
    .await?;
    let token = create_session(state.db.as_ref()).await?;

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::GET,
            "/api/settings/hardware",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("\"device_path\""));

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn sse_route_emits_lagged_event_and_recovers()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 1, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::GET,
            "/api/events",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    state.tx.send(AlchemistEvent::Log {
        level: "info".to_string(),
        job_id: None,
        message: "first".to_string(),
    })?;
    state.tx.send(AlchemistEvent::Log {
        level: "info".to_string(),
        job_id: None,
        message: "second".to_string(),
    })?;

    let mut body = response.into_body();
    let mut rendered = String::new();

    while rendered.matches("event:").count() < 2 {
        let maybe_frame =
            tokio::time::timeout(tokio::time::Duration::from_secs(2), body.frame()).await?;
        let Some(frame) = maybe_frame else {
            break;
        };
        let frame = frame?;
        if let Ok(data) = frame.into_data() {
            rendered.push_str(&String::from_utf8_lossy(&data));
        }
    }

    assert!(rendered.contains("event: lagged"));
    assert!(rendered.contains("event: log"));
    assert!(rendered.contains("\"second\""));

    cleanup_paths(&[config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn job_detail_route_includes_logs_and_failure_summary()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;
    let (job, input_path, output_path) = seed_job(state.db.as_ref(), JobState::Failed).await?;

    state
        .db
        .add_log("info", Some(job.id), "ffmpeg started")
        .await?;
    state
        .db
        .add_log("error", Some(job.id), "No such file or directory")
        .await?;

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::GET,
            &format!("/api/jobs/{}/details", job.id),
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let payload: serde_json::Value = serde_json::from_str(&body_text(response).await)?;
    assert_eq!(
        payload["job_failure_summary"].as_str(),
        Some("No such file or directory")
    );
    assert_eq!(payload["job_logs"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        payload["job_logs"][1]["message"].as_str(),
        Some("No such file or directory")
    );

    cleanup_paths(&[input_path, output_path, config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn delete_active_job_returns_conflict() -> std::result::Result<(), Box<dyn std::error::Error>>
{
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;
    let (job, input_path, output_path) = seed_job(state.db.as_ref(), JobState::Encoding).await?;

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::POST,
            &format!("/api/jobs/{}/delete", job.id),
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = body_text(response).await;
    assert!(body.contains("\"blocked\""));
    assert!(body.contains(&format!("\"id\":{}", job.id)));

    cleanup_paths(&[input_path, output_path, config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn batch_delete_and_restart_block_active_jobs()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;
    let (active_job, active_input, active_output) =
        seed_job(state.db.as_ref(), JobState::Encoding).await?;
    let (queued_job, queued_input, queued_output) =
        seed_job(state.db.as_ref(), JobState::Queued).await?;

    for action in ["delete", "restart"] {
        let response = app
            .clone()
            .oneshot(auth_json_request(
                Method::POST,
                "/api/jobs/batch",
                &token,
                json!({
                    "action": action,
                    "ids": [active_job.id, queued_job.id]
                }),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = body_text(response).await;
        assert!(body.contains("\"blocked\""));
        assert!(body.contains(&format!("\"id\":{}", active_job.id)));
    }

    cleanup_paths(&[
        active_input,
        active_output,
        queued_input,
        queued_output,
        config_path,
        db_path,
    ]);
    Ok(())
}

#[tokio::test]
async fn clear_completed_archives_jobs_and_preserves_stats()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;
    let (job, input_path, output_path) = seed_job(state.db.as_ref(), JobState::Completed).await?;

    state
        .db
        .save_encode_stats(crate::db::EncodeStatsInput {
            job_id: job.id,
            input_size: 2_000,
            output_size: 1_000,
            compression_ratio: 0.5,
            encode_time: 60.0,
            encode_speed: 1.5,
            avg_bitrate: 900.0,
            vmaf_score: Some(95.0),
            output_codec: Some("av1".to_string()),
        })
        .await?;

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::POST,
            "/api/jobs/clear-completed",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("\"count\":1"));
    assert!(body.contains("Historical stats were preserved"));

    assert!(state.db.get_job_by_id(job.id).await?.is_none());
    let aggregated = state.db.get_aggregated_stats().await?;
    assert_eq!(aggregated.completed_jobs, 1);
    assert_eq!(aggregated.total_input_size, 2_000);
    assert_eq!(aggregated.total_output_size, 1_000);

    cleanup_paths(&[input_path, output_path, config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn cancel_queued_job_updates_status() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;
    let (job, input_path, output_path) = seed_job(state.db.as_ref(), JobState::Queued).await?;

    let response = app
        .clone()
        .oneshot(auth_request(
            Method::POST,
            &format!("/api/jobs/{}/cancel", job.id),
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let updated = state.db.get_job_by_id(job.id).await?.expect("updated job");
    assert_eq!(updated.status, JobState::Cancelled);

    cleanup_paths(&[input_path, output_path, config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn priority_endpoint_updates_job_priority()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;
    let (job, input_path, output_path) = seed_job(state.db.as_ref(), JobState::Queued).await?;

    let response = app
        .clone()
        .oneshot(auth_json_request(
            Method::POST,
            &format!("/api/jobs/{}/priority", job.id),
            &token,
            json!({ "priority": 10 }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("\"priority\":10"));

    let updated = state.db.get_job_by_id(job.id).await?.expect("updated job");
    assert_eq!(updated.priority, 10);

    cleanup_paths(&[input_path, output_path, config_path, db_path]);
    Ok(())
}

#[tokio::test]
async fn watch_dir_paths_are_canonicalized_and_deduplicated()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let watch_root = temp_path("alchemist_watch_root", "dir");
    let watch_dir = watch_root.join("library");
    std::fs::create_dir_all(&watch_dir)?;

    let (state, app, config_path, db_path) = build_test_app(false, 8, |_| {}).await?;
    let token = create_session(state.db.as_ref()).await?;
    let first_path = watch_dir.to_string_lossy().to_string();
    let second_path = watch_root
        .join("library/../library")
        .to_string_lossy()
        .to_string();

    let response = app
        .clone()
        .oneshot(auth_json_request(
            Method::POST,
            "/api/settings/watch-dirs",
            &token,
            json!({ "path": first_path, "is_recursive": true }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(auth_json_request(
            Method::POST,
            "/api/settings/watch-dirs",
            &token,
            json!({ "path": second_path, "is_recursive": true }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let dirs = state.db.get_watch_dirs().await?;
    assert_eq!(dirs.len(), 1);
    assert_eq!(
        dirs[0].path,
        std::fs::canonicalize(&watch_dir)?
            .to_string_lossy()
            .to_string()
    );

    cleanup_paths(&[watch_root, config_path, db_path]);
    Ok(())
}
