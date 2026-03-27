//! Minimal FFmpeg integration tests for Alchemist
//!
//! These tests verify the FFmpeg components work correctly without
//! requiring the full server infrastructure.

use alchemist::media::analyzer::FfmpegAnalyzer;
use alchemist::media::pipeline::Analyzer;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Get codec name for a specific stream type using ffprobe
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
        .output()?;

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

#[tokio::test]
async fn test_ffmpeg_analyzer_h264() -> Result<()> {
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
    assert_eq!(analysis.metadata.codec_name, "h264", "Expected H.264 codec");
    assert!(
        analysis.metadata.duration_secs > 0.0,
        "Duration should be greater than 0"
    );
    assert!(analysis.metadata.fps > 0.0, "FPS should be greater than 0");

    // Verify streams - video-only file should have no audio codec
    assert!(
        analysis.metadata.audio_codec.is_none(),
        "Should not have audio codec in video-only file"
    );
    assert!(
        analysis.metadata.subtitle_streams.is_empty(),
        "Should not have subtitle streams in video-only file"
    );

    println!("✓ H.264 analysis test passed");
    Ok(())
}

#[tokio::test]
async fn test_ffmpeg_analyzer_hevc() -> Result<()> {
    if !ffprobe_available() {
        println!("Skipping test: FFprobe not available");
        return Ok(());
    }

    let fixtures = fixtures_path();
    let input = fixtures.join("test_hevc.mp4");

    if !input.exists() {
        println!("Skipping test: Fixture file not found: {}", input.display());
        return Ok(());
    }

    let analyzer = FfmpegAnalyzer;
    let analysis = analyzer.analyze(&input).await?;

    // Verify basic analysis results
    assert_eq!(analysis.metadata.width, 320, "Expected width 320");
    assert_eq!(analysis.metadata.height, 240, "Expected height 240");
    assert_eq!(analysis.metadata.codec_name, "hevc", "Expected HEVC codec");
    assert!(
        analysis.metadata.duration_secs > 0.0,
        "Duration should be greater than 0"
    );

    println!("✓ HEVC analysis test passed");
    Ok(())
}

#[tokio::test]
async fn test_ffmpeg_analyzer_audio() -> Result<()> {
    if !ffprobe_available() {
        println!("Skipping test: FFprobe not available");
        return Ok(());
    }

    let fixtures = fixtures_path();
    let input = fixtures.join("test_h264_with_audio.mp4");

    if !input.exists() {
        println!("Skipping test: Fixture file not found: {}", input.display());
        return Ok(());
    }

    let analyzer = FfmpegAnalyzer;
    let analysis = analyzer.analyze(&input).await?;

    // Verify basic analysis results
    assert_eq!(
        analysis.metadata.codec_name, "h264",
        "Expected H.264 video codec"
    );
    assert!(
        analysis.metadata.audio_codec.is_some(),
        "Should have audio codec"
    );

    // Check audio metadata
    if let Some(audio_codec) = &analysis.metadata.audio_codec {
        assert_eq!(audio_codec, "aac", "Expected AAC audio codec");
    }

    println!("✓ Audio stream analysis test passed");
    Ok(())
}

#[tokio::test]
async fn test_ffmpeg_analyzer_subtitles() -> Result<()> {
    if !ffprobe_available() {
        println!("Skipping test: FFprobe not available");
        return Ok(());
    }

    let fixtures = fixtures_path();
    let input = fixtures.join("test_h264_with_subtitles.mkv");

    if !input.exists() {
        println!("Skipping test: Fixture file not found: {}", input.display());
        return Ok(());
    }

    let analyzer = FfmpegAnalyzer;
    let analysis = analyzer.analyze(&input).await?;

    // Verify basic analysis results
    assert_eq!(
        analysis.metadata.codec_name, "h264",
        "Expected H.264 video codec"
    );
    assert!(
        !analysis.metadata.subtitle_streams.is_empty(),
        "Should have subtitle streams"
    );

    // Check subtitle metadata
    let subtitle = &analysis.metadata.subtitle_streams[0];
    assert_eq!(subtitle.codec_name, "subrip", "Expected SRT subtitle codec");

    println!("✓ Subtitle stream analysis test passed");
    Ok(())
}

#[tokio::test]
async fn test_ffmpeg_availability() -> Result<()> {
    println!("FFmpeg available: {}", ffmpeg_available());
    println!("FFprobe available: {}", ffprobe_available());
    println!("FFmpeg ready: {}", ffmpeg_ready());

    if ffmpeg_ready() {
        // Test basic ffprobe functionality
        let fixtures = fixtures_path();
        let input = fixtures.join("test_h264.mp4");

        if input.exists() {
            let codec = get_codec_name(&input, "v:0")?;
            assert_eq!(
                codec,
                Some("h264".to_string()),
                "Expected H.264 codec from ffprobe"
            );
            println!("✓ Direct ffprobe test passed");
        }
    }

    println!("✓ FFmpeg availability test completed");
    Ok(())
}

#[tokio::test]
async fn test_multiple_format_analysis() -> Result<()> {
    if !ffprobe_available() {
        println!("Skipping test: FFprobe not available");
        return Ok(());
    }

    let fixtures = fixtures_path();
    let analyzer = FfmpegAnalyzer;

    let test_files = vec![("test_h264.mp4", "h264"), ("test_hevc.mp4", "hevc")];

    for (filename, expected_codec) in test_files {
        let input = fixtures.join(filename);

        if !input.exists() {
            println!("Skipping {}: Fixture file not found", filename);
            continue;
        }

        let analysis = analyzer.analyze(&input).await?;
        assert_eq!(
            analysis.metadata.codec_name, expected_codec,
            "Expected {} codec for {}",
            expected_codec, filename
        );

        println!("✓ {} format analysis passed", filename);
    }

    Ok(())
}

#[test]
fn test_fixture_files_exist() {
    let fixtures = fixtures_path();
    println!("Fixtures path: {}", fixtures.display());

    let expected_files = vec![
        "test_h264.mp4",
        "test_hevc.mp4",
        "test_h264_with_audio.mp4",
        "test_h264_with_subtitles.mkv",
        "test_subtitle.srt",
    ];

    for filename in expected_files {
        let path = fixtures.join(filename);
        if path.exists() {
            println!("✓ Found fixture: {}", filename);
        } else {
            println!("⚠ Missing fixture: {}", filename);
        }
    }
}
