//! FFmpeg integration tests for Alchemist
//!
//! These tests verify the FFmpeg pipeline works correctly end-to-end.
//! They require FFmpeg and FFprobe to be available on the system.

use alchemist::config::{Config, OutputCodec, SubtitleMode};
use alchemist::db::{Db, JobState};
use alchemist::media::analyzer::FfmpegAnalyzer;
use alchemist::media::pipeline::{Analyzer, Pipeline};
use alchemist::orchestrator::Transcoder;
use alchemist::system::hardware::{HardwareInfo, HardwareState, Vendor};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{RwLock, broadcast};

/// Check if FFmpeg is available on the system
fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Check if FFprobe is available on the system
fn ffprobe_available() -> bool {
    Command::new("ffprobe")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Check if both FFmpeg and FFprobe are available
fn ffmpeg_ready() -> bool {
    ffmpeg_available() && ffprobe_available()
}

/// Get the path to test fixtures
fn fixtures_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fixtures");
    path
}

/// Create a temporary directory for test outputs
fn temp_output_dir(test_name: &str) -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "alchemist_test_{}_{}",
        test_name,
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// Clean up temporary directory
fn cleanup_temp_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

/// Create a test database
async fn create_test_db() -> Result<(Arc<Db>, PathBuf)> {
    let mut db_path = std::env::temp_dir();
    db_path.push(format!("alchemist_test_{}.db", rand::random::<u64>()));

    let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
    Ok((db, db_path))
}

/// Build a test pipeline with custom configuration
async fn build_test_pipeline<F>(configure: F) -> Result<(Arc<Db>, Pipeline, PathBuf)>
where
    F: FnOnce(&mut Config),
{
    let (db, db_path) = create_test_db().await?;

    let mut config = Config::default();
    // Set sensible defaults for testing
    config.transcode.output_codec = OutputCodec::H264;
    config.transcode.min_file_size_mb = 0;
    config.transcode.min_bpp_threshold = 0.0;
    config.transcode.size_reduction_threshold = -1.0;
    config.quality.enable_vmaf = false;
    config.hardware.allow_cpu_encoding = true;
    config.hardware.allow_cpu_fallback = true;

    // Apply custom configuration
    configure(&mut config);

    // Create event channels for the pipeline
    let (jobs_tx, _) = broadcast::channel(100);
    let (config_tx, _) = broadcast::channel(10);
    let (system_tx, _) = broadcast::channel(10);
    let event_channels = Arc::new(alchemist::db::EventChannels {
        jobs: jobs_tx,
        config: config_tx,
        system: system_tx,
    });

    let pipeline = Pipeline::new(
        db.clone(),
        Arc::new(Transcoder::new()),
        Arc::new(RwLock::new(config)),
        HardwareState::new(Some(HardwareInfo {
            vendor: Vendor::Cpu,
            device_path: None,
            supported_codecs: vec!["av1".to_string(), "hevc".to_string(), "h264".to_string()],
            backends: Vec::new(),
            detection_notes: Vec::new(),
            selection_reason: String::new(),
            probe_summary: alchemist::system::hardware::ProbeSummary::default(),
        })),
        event_channels,
        false,
    );

    Ok((db, pipeline, db_path))
}

/// Enqueue and process a transcode job
async fn enqueue_and_process(
    db: &Db,
    pipeline: &Pipeline,
    input: &Path,
    output: &Path,
) -> Result<JobState> {
    db.enqueue_job(input, output, SystemTime::UNIX_EPOCH)
        .await?;

    let job = db
        .get_job_by_input_path(input.to_string_lossy().as_ref())
        .await?
        .context("job missing")?;

    if let Err(failure) = pipeline.process_job(job.clone()).await {
        let logs = db.get_logs(50, 0).await.unwrap_or_default();
        let details = logs
            .into_iter()
            .filter(|entry| entry.job_id == Some(job.id))
            .map(|entry| entry.message)
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::bail!("job failed with {:?}\n{}", failure, details);
    }

    let updated_job = db
        .get_job_by_id(job.id)
        .await?
        .context("updated job missing")?;

    Ok(updated_job.status)
}

/// Get stream count by type from FFprobe
fn get_stream_count(path: &Path, stream_type: &str) -> Result<usize> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            stream_type,
            "-show_entries",
            "stream=index",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .context("ffprobe failed")?;

    if !output.status.success() {
        anyhow::bail!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let count = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();

    Ok(count)
}

/// Get codec name for a specific stream type
fn get_codec_name(path: &Path, stream_type: &str) -> Result<Option<String>> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            stream_type,
            "-show_entries",
            "stream=codec_name",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .context("ffprobe failed")?;

    if !output.status.success() {
        anyhow::bail!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let codec = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|codec| !codec.is_empty());

    Ok(codec)
}

/// Verify that a file exists and has the expected codec
fn verify_output_codec(path: &Path, expected_codec: &str) -> Result<()> {
    assert!(
        path.exists(),
        "Output file should exist: {}",
        path.display()
    );

    let actual_codec = get_codec_name(path, "v:0")?.context("No video codec found in output")?;

    assert_eq!(
        actual_codec, expected_codec,
        "Expected codec {}, got {}",
        expected_codec, actual_codec
    );
    Ok(())
}

#[tokio::test]
async fn test_h264_to_hevc_cpu_transcode() -> Result<()> {
    if !ffmpeg_ready() {
        println!("Skipping test: FFmpeg not available");
        return Ok(());
    }

    let fixtures = fixtures_path();
    let input = fixtures.join("test_h264.mp4");

    if !input.exists() {
        println!("Skipping test: Fixture file not found: {}", input.display());
        return Ok(());
    }

    let temp_dir = temp_output_dir("h264_to_hevc")?;
    let output = temp_dir.join("output_hevc.mp4");

    let (db, pipeline, db_path) = build_test_pipeline(|config| {
        config.transcode.output_codec = OutputCodec::Hevc;
    })
    .await?;

    let state = enqueue_and_process(db.as_ref(), &pipeline, &input, &output).await?;

    assert_eq!(
        state,
        JobState::Completed,
        "Job should complete successfully"
    );
    verify_output_codec(&output, "hevc")?;

    // Cleanup
    let _ = std::fs::remove_file(db_path);
    cleanup_temp_dir(&temp_dir);

    Ok(())
}

#[tokio::test]
async fn test_basic_video_analysis() -> Result<()> {
    if !ffprobe_available() {
        println!("Skipping test: FFprobe not available");
        return Ok(());
    }

    let fixtures = fixtures_path();
    let input = fixtures.join("test_h264.mp4");

    if !input.exists() {
        println!("Skipping test: Fixture file not found: {}", input.display());
        return Ok(());
    }

    let analyzer = FfmpegAnalyzer;
    let analysis = analyzer.analyze(&input).await?;

    // Verify basic analysis results
    assert_eq!(analysis.metadata.width, 320, "Expected width 320");
    assert_eq!(analysis.metadata.height, 240, "Expected height 240");
    assert!(
        !analysis.metadata.codec_name.is_empty(),
        "Video codec should be detected"
    );
    assert!(
        analysis.metadata.duration_secs > 0.0,
        "Duration should be greater than 0"
    );

    // Verify streams - we can check if there are subtitle/audio streams in metadata
    // For video-only files, audio codec should be None and subtitle streams empty
    assert!(
        analysis.metadata.audio_codec.is_none(),
        "Should not have audio codec in video-only file"
    );
    assert!(
        analysis.metadata.subtitle_streams.is_empty(),
        "Should not have subtitle streams in video-only file"
    );

    Ok(())
}

#[tokio::test]
async fn test_audio_stream_handling() -> Result<()> {
    if !ffmpeg_ready() {
        println!("Skipping test: FFmpeg not available");
        return Ok(());
    }

    let fixtures = fixtures_path();
    let input = fixtures.join("test_h264_with_audio.mp4");

    if !input.exists() {
        println!("Skipping test: Fixture file not found: {}", input.display());
        return Ok(());
    }

    let temp_dir = temp_output_dir("audio_handling")?;
    let output = temp_dir.join("output_with_audio.mp4");

    let (db, pipeline, db_path) = build_test_pipeline(|config| {
        // Force a transcode so this test covers audio handling
        // instead of the planner's same-codec skip path.
        config.transcode.output_codec = OutputCodec::Hevc;
    })
    .await?;

    let state = enqueue_and_process(db.as_ref(), &pipeline, &input, &output).await?;

    assert_eq!(
        state,
        JobState::Completed,
        "Job should complete successfully"
    );

    // Verify video and audio streams
    let video_count = get_stream_count(&output, "v")?;
    let audio_count = get_stream_count(&output, "a")?;

    assert_eq!(video_count, 1, "Should have one video stream");
    assert_eq!(audio_count, 1, "Should have one audio stream");

    // Cleanup
    let _ = std::fs::remove_file(db_path);
    cleanup_temp_dir(&temp_dir);

    Ok(())
}

#[tokio::test]
async fn test_subtitle_extraction() -> Result<()> {
    if !ffmpeg_ready() {
        println!("Skipping test: FFmpeg not available");
        return Ok(());
    }

    let fixtures = fixtures_path();
    let input = fixtures.join("test_h264_with_subtitles.mkv");

    if !input.exists() {
        println!("Skipping test: Fixture file not found: {}", input.display());
        return Ok(());
    }

    let temp_dir = temp_output_dir("subtitle_extraction")?;
    let output = temp_dir.join("output_no_subs.mkv");

    let (db, pipeline, db_path) = build_test_pipeline(|config| {
        config.transcode.subtitle_mode = SubtitleMode::Extract;
        // Force a transcode so subtitle extraction is exercised
        // instead of skipping the already-H.264 fixture.
        config.transcode.output_codec = OutputCodec::Hevc;
    })
    .await?;

    let state = enqueue_and_process(db.as_ref(), &pipeline, &input, &output).await?;

    assert_eq!(
        state,
        JobState::Completed,
        "Job should complete successfully"
    );

    // Verify main output has no subtitle streams
    let subtitle_count = get_stream_count(&output, "s")?;
    assert_eq!(
        subtitle_count, 0,
        "Main output should have no subtitle streams"
    );

    // Check for sidecar subtitle files (basic check)
    let sidecar_files: Vec<_> = std::fs::read_dir(&temp_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("srt"))
        .collect();

    // Should have extracted at least one subtitle file
    assert!(
        !sidecar_files.is_empty(),
        "Should have extracted subtitle files"
    );

    // Cleanup
    let _ = std::fs::remove_file(db_path);
    cleanup_temp_dir(&temp_dir);

    Ok(())
}

#[tokio::test]
async fn test_multiple_input_formats() -> Result<()> {
    if !ffmpeg_ready() {
        println!("Skipping test: FFmpeg not available");
        return Ok(());
    }

    let fixtures = fixtures_path();
    let test_files = vec![("test_h264.mp4", "h264"), ("test_hevc.mp4", "hevc")];

    for (filename, expected_input_codec) in test_files {
        let input = fixtures.join(filename);

        if !input.exists() {
            println!("Skipping {}: Fixture file not found", filename);
            continue;
        }

        // Verify input codec first
        let input_codec =
            get_codec_name(&input, "v:0")?.context("No video codec found in input")?;
        assert_eq!(
            input_codec, expected_input_codec,
            "Expected input codec {}",
            expected_input_codec
        );

        let temp_dir = temp_output_dir(&format!("multi_format_{}", expected_input_codec))?;
        let output = temp_dir.join("output.mp4");
        let target_codec = match expected_input_codec {
            "h264" => OutputCodec::Hevc,
            "hevc" => OutputCodec::H264,
            other => anyhow::bail!("Unexpected fixture codec: {}", other),
        };
        let expected_output_codec = match target_codec {
            OutputCodec::Hevc => "hevc",
            OutputCodec::H264 => "h264",
            OutputCodec::Av1 => "av1",
        };

        let (db, pipeline, db_path) = build_test_pipeline(|config| {
            // Pick the opposite codec so both fixtures exercise a
            // completed transcode rather than a planner skip.
            config.transcode.output_codec = target_codec;
        })
        .await?;

        let state = enqueue_and_process(db.as_ref(), &pipeline, &input, &output).await?;

        assert_eq!(
            state,
            JobState::Completed,
            "Job should complete successfully for {}",
            filename
        );
        verify_output_codec(&output, expected_output_codec)?;

        // Cleanup
        let _ = std::fs::remove_file(db_path);
        cleanup_temp_dir(&temp_dir);
    }

    Ok(())
}

#[tokio::test]
async fn test_analyzer_stream_detection() -> Result<()> {
    if !ffprobe_available() {
        println!("Skipping test: FFprobe not available");
        return Ok(());
    }

    let fixtures = fixtures_path();
    let analyzer = FfmpegAnalyzer;

    // Test video-only file
    let video_only = fixtures.join("test_h264.mp4");
    if video_only.exists() {
        let analysis = analyzer.analyze(&video_only).await?;
        assert!(
            !analysis.metadata.codec_name.is_empty(),
            "Should detect video codec"
        );
        assert!(
            analysis.metadata.audio_codec.is_none(),
            "Should not detect audio codec in video-only file"
        );
        assert!(
            analysis.metadata.subtitle_streams.is_empty(),
            "Should not detect subtitle streams in video-only file"
        );
    }

    // Test video+audio file
    let video_audio = fixtures.join("test_h264_with_audio.mp4");
    if video_audio.exists() {
        let analysis = analyzer.analyze(&video_audio).await?;
        assert!(
            !analysis.metadata.codec_name.is_empty(),
            "Should detect video codec"
        );
        assert!(
            analysis.metadata.audio_codec.is_some(),
            "Should detect audio codec"
        );
    }

    // Test video+subtitle file
    let video_subs = fixtures.join("test_h264_with_subtitles.mkv");
    if video_subs.exists() {
        let analysis = analyzer.analyze(&video_subs).await?;
        assert!(
            !analysis.metadata.codec_name.is_empty(),
            "Should detect video codec"
        );
        assert!(
            !analysis.metadata.subtitle_streams.is_empty(),
            "Should detect subtitle streams"
        );
    }

    Ok(())
}

#[cfg(test)]
mod hardware_fallback_tests {
    use super::*;

    #[tokio::test]
    async fn test_cpu_fallback_when_hardware_unavailable() -> Result<()> {
        if !ffmpeg_ready() {
            println!("Skipping test: FFmpeg not available");
            return Ok(());
        }

        let fixtures = fixtures_path();
        let input = fixtures.join("test_h264.mp4");

        if !input.exists() {
            println!("Skipping test: Fixture file not found: {}", input.display());
            return Ok(());
        }

        let temp_dir = temp_output_dir("cpu_fallback")?;
        let output = temp_dir.join("output_fallback.mp4");

        let (db, pipeline, db_path) = build_test_pipeline(|config| {
            config.transcode.output_codec = OutputCodec::Hevc;
            config.hardware.allow_cpu_encoding = true;
            config.hardware.allow_cpu_fallback = true;
            // Simulate hardware being unavailable by only allowing CPU
        })
        .await?;

        let state = enqueue_and_process(db.as_ref(), &pipeline, &input, &output).await?;

        // Should complete with CPU fallback
        assert_eq!(
            state,
            JobState::Completed,
            "Job should complete with CPU fallback"
        );
        verify_output_codec(&output, "hevc")?;

        // Cleanup
        let _ = std::fs::remove_file(db_path);
        cleanup_temp_dir(&temp_dir);

        Ok(())
    }
}
