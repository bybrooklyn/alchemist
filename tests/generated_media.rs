use alchemist::config::{Config, HdrMode, OutputCodec, SubtitleMode};
use alchemist::db::{Db, JobState};
use alchemist::media::pipeline::Pipeline;
use alchemist::orchestrator::Transcoder;
use alchemist::system::hardware::{HardwareInfo, HardwareState, Vendor};
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{broadcast, RwLock};

#[derive(Clone)]
struct SubtitleInputSpec {
    text: String,
    language: &'static str,
    default: bool,
    forced: bool,
}

fn ffmpeg_ready() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
        && Command::new("ffprobe")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
}

fn temp_root(prefix: &str) -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    path.push(format!("{prefix}_{}", rand::random::<u64>()));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn cleanup_root(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

fn run_cmd(program: &str, args: &[String]) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {program}"))?;
    if !output.status.success() {
        bail!(
            "{} failed: {}",
            program,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn write_srt(path: &Path, text: &str) -> Result<()> {
    std::fs::write(path, format!("1\n00:00:00,000 --> 00:00:01,500\n{text}\n"))?;
    Ok(())
}

fn generate_subtitled_input(output: &Path, subtitles: &[SubtitleInputSpec]) -> Result<()> {
    let mut args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        "color=c=black:s=160x90:d=2".to_string(),
    ];

    for (index, subtitle) in subtitles.iter().enumerate() {
        let subtitle_path = output
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(format!("subtitle-{index}.srt"));
        write_srt(&subtitle_path, &subtitle.text)?;
        args.push("-i".to_string());
        args.push(subtitle_path.display().to_string());
    }

    args.extend([
        "-map".to_string(),
        "0:v:0".to_string(),
        "-c:v".to_string(),
        "mpeg4".to_string(),
        "-t".to_string(),
        "2".to_string(),
    ]);

    for (index, subtitle) in subtitles.iter().enumerate() {
        args.push("-map".to_string());
        args.push(format!("{}:0", index + 1));
        args.push(format!("-metadata:s:{index}"));
        args.push(format!("language={}", subtitle.language));

        let disposition = match (subtitle.default, subtitle.forced) {
            (true, true) => Some("default+forced"),
            (true, false) => Some("default"),
            (false, true) => Some("forced"),
            (false, false) => None,
        };
        if let Some(disposition) = disposition {
            args.push(format!("-disposition:s:{index}"));
            args.push(disposition.to_string());
        }
    }

    if !subtitles.is_empty() {
        args.extend(["-c:s".to_string(), "srt".to_string()]);
    }

    args.push(output.display().to_string());
    run_cmd("ffmpeg", &args)
}

fn generate_hdr_input(output: &Path) -> Result<()> {
    let args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        "testsrc2=size=160x90:rate=24:d=2".to_string(),
        "-vf".to_string(),
        "format=yuv420p10le".to_string(),
        "-c:v".to_string(),
        "libx265".to_string(),
        "-x265-params".to_string(),
        "log-level=error".to_string(),
        "-color_primaries".to_string(),
        "bt2020".to_string(),
        "-color_trc".to_string(),
        "smpte2084".to_string(),
        "-colorspace".to_string(),
        "bt2020nc".to_string(),
        "-t".to_string(),
        "2".to_string(),
        output.display().to_string(),
    ];
    run_cmd("ffmpeg", &args)
}

fn generate_heavy_audio_input(output: &Path) -> Result<()> {
    let args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        "color=c=black:s=160x90:d=2".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        "sine=frequency=1000:sample_rate=48000:duration=2".to_string(),
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "1:a:0".to_string(),
        "-c:v".to_string(),
        "mpeg4".to_string(),
        "-c:a".to_string(),
        "flac".to_string(),
        "-shortest".to_string(),
        output.display().to_string(),
    ];
    run_cmd("ffmpeg", &args)
}

fn frame_hash(path: &Path) -> Result<String> {
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            path.to_string_lossy().as_ref(),
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "-",
        ])
        .output()
        .context("extract frame")?;
    if !output.status.success() {
        bail!(
            "ffmpeg frame hash failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(format!("{:x}", Sha256::digest(output.stdout)))
}

fn ffprobe_json(path: &Path) -> Result<serde_json::Value> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
            path.to_string_lossy().as_ref(),
        ])
        .output()
        .context("ffprobe json")?;
    if !output.status.success() {
        bail!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).context("parse ffprobe json")
}

fn subtitle_stream_count(path: &Path) -> Result<usize> {
    let json = ffprobe_json(path)?;
    Ok(json["streams"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .filter(|stream| stream["codec_type"].as_str() == Some("subtitle"))
        .count())
}

fn audio_codec(path: &Path) -> Result<Option<String>> {
    let json = ffprobe_json(path)?;
    Ok(json["streams"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .find(|stream| stream["codec_type"].as_str() == Some("audio"))
        .and_then(|stream| stream["codec_name"].as_str().map(str::to_string)))
}

fn color_tags(path: &Path) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let json = ffprobe_json(path)?;
    let empty = Vec::new();
    let streams = json["streams"].as_array().unwrap_or(&empty);
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"].as_str() == Some("video"));
    Ok((
        video.and_then(|stream| stream["color_primaries"].as_str().map(str::to_string)),
        video.and_then(|stream| stream["color_transfer"].as_str().map(str::to_string)),
        video.and_then(|stream| stream["color_space"].as_str().map(str::to_string)),
    ))
}

async fn build_pipeline<F>(configure: F) -> Result<(Arc<Db>, Pipeline, PathBuf)>
where
    F: FnOnce(&mut Config),
{
    let db_path = {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "alchemist_generated_media_{}.db",
            rand::random::<u64>()
        ));
        path
    };
    let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
    let mut config = Config::default();
    config.transcode.output_codec = OutputCodec::H264;
    config.transcode.min_file_size_mb = 0;
    config.transcode.min_bpp_threshold = 0.0;
    config.transcode.size_reduction_threshold = -1.0;
    config.quality.enable_vmaf = false;
    config.hardware.allow_cpu_encoding = true;
    config.hardware.allow_cpu_fallback = true;
    configure(&mut config);

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
        })),
        Arc::new(broadcast::channel(16).0),
        false,
    );

    Ok((db, pipeline, db_path))
}

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
        bail!("job failed with {:?}\n{}", failure, details);
    }
    Ok(db
        .get_job_by_id(job.id)
        .await?
        .context("updated job missing")?
        .status)
}

#[tokio::test]
async fn burn_subtitles_changes_video_frame_for_mkv_and_mp4() -> Result<()> {
    if !ffmpeg_ready() {
        return Ok(());
    }

    let root = temp_root("alchemist_burn_subtitles")?;
    let input = root.join("input.mkv");
    generate_subtitled_input(
        &input,
        &[SubtitleInputSpec {
            text: "ALCHEMIST".to_string(),
            language: "eng",
            default: true,
            forced: false,
        }],
    )?;

    for extension in ["mkv", "mp4"] {
        let baseline_output = root.join(format!("baseline.{extension}"));
        let burn_output = root.join(format!("burn.{extension}"));

        let (baseline_db, baseline_pipeline, baseline_db_path) = build_pipeline(|config| {
            config.transcode.subtitle_mode = SubtitleMode::None;
        })
        .await?;
        let baseline_state = enqueue_and_process(
            baseline_db.as_ref(),
            &baseline_pipeline,
            &input,
            &baseline_output,
        )
        .await?;
        assert_eq!(baseline_state, JobState::Completed);

        let (burn_db, burn_pipeline, burn_db_path) = build_pipeline(|config| {
            config.transcode.subtitle_mode = SubtitleMode::Burn;
        })
        .await?;
        let burn_state =
            enqueue_and_process(burn_db.as_ref(), &burn_pipeline, &input, &burn_output).await?;
        assert_eq!(burn_state, JobState::Completed);
        assert_eq!(subtitle_stream_count(&burn_output)?, 0);
        assert_ne!(frame_hash(&baseline_output)?, frame_hash(&burn_output)?);

        let _ = std::fs::remove_file(baseline_db_path);
        let _ = std::fs::remove_file(burn_db_path);
    }

    cleanup_root(&root);
    Ok(())
}

#[tokio::test]
async fn extract_subtitles_writes_sidecar_and_strips_main_output() -> Result<()> {
    if !ffmpeg_ready() {
        return Ok(());
    }

    let root = temp_root("alchemist_extract_subtitles")?;
    let input = root.join("input.mkv");
    let output = root.join("output.mkv");
    let sidecar = root.join("output.subs.mks");
    generate_subtitled_input(
        &input,
        &[
            SubtitleInputSpec {
                text: "English".to_string(),
                language: "eng",
                default: true,
                forced: false,
            },
            SubtitleInputSpec {
                text: "Spanish".to_string(),
                language: "spa",
                default: false,
                forced: false,
            },
        ],
    )?;

    let (db, pipeline, db_path) = build_pipeline(|config| {
        config.transcode.subtitle_mode = SubtitleMode::Extract;
    })
    .await?;
    let state = enqueue_and_process(db.as_ref(), &pipeline, &input, &output).await?;
    assert_eq!(state, JobState::Completed);
    assert_eq!(subtitle_stream_count(&output)?, 0);
    assert!(sidecar.exists());
    assert_eq!(subtitle_stream_count(&sidecar)?, 2);
    assert!(!root.join("output.subs.mks.alchemist-part").exists());

    let _ = std::fs::remove_file(db_path);
    cleanup_root(&root);
    Ok(())
}

#[tokio::test]
async fn tonemap_outputs_bt709_color_tags() -> Result<()> {
    if !ffmpeg_ready() {
        return Ok(());
    }

    let root = temp_root("alchemist_tonemap")?;
    let input = root.join("hdr-input.mkv");
    let output = root.join("tonemapped.mkv");
    generate_hdr_input(&input)?;

    let (db, pipeline, db_path) = build_pipeline(|config| {
        config.transcode.hdr_mode = HdrMode::Tonemap;
        config.transcode.output_codec = OutputCodec::H264;
    })
    .await?;
    let state = enqueue_and_process(db.as_ref(), &pipeline, &input, &output).await?;
    assert_eq!(state, JobState::Completed);
    let (primaries, transfer, space) = color_tags(&output)?;
    assert_ne!(primaries.as_deref(), Some("bt2020"));
    assert_ne!(transfer.as_deref(), Some("smpte2084"));
    if let Some(primaries) = primaries.as_deref() {
        assert_eq!(primaries, "bt709");
    }
    if let Some(transfer) = transfer.as_deref() {
        assert_eq!(transfer, "bt709");
    }
    let _ = space;

    let _ = std::fs::remove_file(db_path);
    cleanup_root(&root);
    Ok(())
}

#[tokio::test]
async fn heavy_audio_inputs_are_transcoded() -> Result<()> {
    if !ffmpeg_ready() {
        return Ok(());
    }

    let root = temp_root("alchemist_heavy_audio")?;
    let input = root.join("input.mkv");
    let output = root.join("output.mkv");
    generate_heavy_audio_input(&input)?;

    let (db, pipeline, db_path) = build_pipeline(|config| {
        config.transcode.subtitle_mode = SubtitleMode::None;
    })
    .await?;
    let state = enqueue_and_process(db.as_ref(), &pipeline, &input, &output).await?;
    assert_eq!(state, JobState::Completed);
    assert_eq!(audio_codec(&output)?.as_deref(), Some("opus"));

    let _ = std::fs::remove_file(db_path);
    cleanup_root(&root);
    Ok(())
}

#[tokio::test]
async fn fallback_disabled_skips_without_spawning_transcode() -> Result<()> {
    if !ffmpeg_ready() {
        return Ok(());
    }

    let root = temp_root("alchemist_fallback_disabled")?;
    let input = root.join("input.mkv");
    let output = root.join("output.mkv");
    generate_heavy_audio_input(&input)?;

    let (db, pipeline, db_path) = build_pipeline(|config| {
        config.transcode.output_codec = OutputCodec::Av1;
        config.transcode.allow_fallback = false;
        config.hardware.allow_cpu_encoding = false;
        config.hardware.allow_cpu_fallback = false;
    })
    .await?;
    let state = enqueue_and_process(db.as_ref(), &pipeline, &input, &output).await?;
    assert_eq!(state, JobState::Skipped);
    assert!(!output.exists());

    let _ = std::fs::remove_file(db_path);
    cleanup_root(&root);
    Ok(())
}
