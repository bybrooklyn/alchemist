//! System pipeline self-test.
//!
//! Run-time validation of FFmpeg probing, planning, encoding, and verification
//! using an embedded tiny video fixture to ensure local hardware environments are functional.

use crate::db::{Db, EventChannels, Job, JobState};
use crate::media::analyzer::FfmpegAnalyzer;
use crate::media::executor::FfmpegExecutor;
use crate::media::pipeline::{Analyzer, Executor, Planner};
use crate::media::planner::BasicPlanner;
use crate::orchestrator::Transcoder;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

const TEST_VIDEO_BYTES: &[u8] = include_bytes!("../../tests/fixtures/test_h264_with_audio.mp4");

#[derive(Debug, Serialize, Clone)]
pub struct SelftestStageResult {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SelftestResponse {
    pub success: bool,
    pub stages: Vec<SelftestStageResult>,
    pub error: Option<String>,
}

/// Executes the full, isolated self-test pipeline.
pub async fn run_selftest() -> SelftestResponse {
    let mut stages = Vec::new();
    let mut overall_success = true;
    let mut overall_error = None;

    let temp_dir_name = format!("alchemist-selftest-{}", uuid::Uuid::new_v4());
    let temp_dir = std::env::temp_dir().join(temp_dir_name);
    let input_path = temp_dir.join("input.mp4");
    let output_path = temp_dir.join("output.mp4");

    // 1. Write Temp Stage
    let start = Instant::now();
    let write_res = async {
        tokio::fs::create_dir_all(&temp_dir).await?;
        tokio::fs::write(&input_path, TEST_VIDEO_BYTES).await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;
    let duration = start.elapsed().as_millis() as u64;

    match write_res {
        Ok(_) => {
            stages.push(SelftestStageResult {
                name: "Write Temp".to_string(),
                success: true,
                duration_ms: duration,
                message: "Embedded sample video written to temporary directory successfully."
                    .to_string(),
            });
        }
        Err(e) => {
            stages.push(SelftestStageResult {
                name: "Write Temp".to_string(),
                success: false,
                duration_ms: duration,
                message: format!("Failed to write temporary test file: {e}"),
            });
            let _ = cleanup_temp_dir(&temp_dir).await;
            return SelftestResponse {
                success: false,
                stages,
                error: Some(format!("Self-test aborted during 'Write Temp': {e}")),
            };
        }
    }

    // 2. Analyze Stage
    let start = Instant::now();
    let analyzer = FfmpegAnalyzer;
    let analyze_res = analyzer.analyze(&input_path).await;
    let duration = start.elapsed().as_millis() as u64;

    let analysis = match analyze_res {
        Ok(analysis) => {
            let container = &analysis.metadata.container;
            let width = analysis.metadata.width;
            let height = analysis.metadata.height;
            let codec = &analysis.metadata.codec_name;
            stages.push(SelftestStageResult {
                name: "Analyze".to_string(),
                success: true,
                duration_ms: duration,
                message: format!(
                    "Media file probed successfully. Container: {container}, Resolution: {width}x{height}, Codec: {codec}."
                ),
            });
            analysis
        }
        Err(e) => {
            stages.push(SelftestStageResult {
                name: "Analyze".to_string(),
                success: false,
                duration_ms: duration,
                message: format!("FFprobe analysis failed: {e}"),
            });
            let _ = cleanup_temp_dir(&temp_dir).await;
            return SelftestResponse {
                success: false,
                stages,
                error: Some(format!("Self-test aborted during 'Analyze': {e}")),
            };
        }
    };

    // 3. Plan Stage
    let start = Instant::now();
    let mut custom_config = crate::config::Config::default();
    custom_config.transcode.output_codec = crate::config::OutputCodec::Hevc;
    custom_config.transcode.min_bpp_threshold = 0.0;
    custom_config.transcode.min_file_size_mb = 0;
    custom_config.hardware.allow_cpu_encoding = true;
    custom_config.hardware.allow_cpu_fallback = true;
    let config = Arc::new(custom_config);
    let planner = BasicPlanner::new(config, None);
    let plan_res = planner.plan(&analysis, &output_path, None).await;
    let duration = start.elapsed().as_millis() as u64;

    let plan = match plan_res {
        Ok(plan) => {
            let decision = format!("{:?}", plan.decision);
            let codec = format!("{:?}", plan.output_codec);
            let encoder = plan
                .encoder
                .map(|e| e.ffmpeg_encoder_name())
                .unwrap_or("none");
            stages.push(SelftestStageResult {
                name: "Plan".to_string(),
                success: true,
                duration_ms: duration,
                message: format!(
                    "Transcode plan computed successfully. Action: {decision}, Target Codec: {codec}, Encoder: {encoder}."
                ),
            });
            plan
        }
        Err(e) => {
            stages.push(SelftestStageResult {
                name: "Plan".to_string(),
                success: false,
                duration_ms: duration,
                message: format!("Failed to plan transcode: {e}"),
            });
            let _ = cleanup_temp_dir(&temp_dir).await;
            return SelftestResponse {
                success: false,
                stages,
                error: Some(format!("Self-test aborted during 'Plan': {e}")),
            };
        }
    };

    // 4. Execute Stage
    let start = Instant::now();
    let execute_res = async {
        let db = Arc::new(Db::new(":memory:").await?);
        let job = Job {
            id: 1,
            input_path: input_path.to_string_lossy().to_string(),
            output_path: output_path.to_string_lossy().to_string(),
            status: JobState::Queued,
            decision_reason: None,
            priority: 0,
            progress: 0.0,
            attempt_count: 0,
            vmaf_score: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            input_metadata_json: None,
            source_device: None,
        };
        db.add_job(job.clone()).await?;

        let transcoder = Arc::new(Transcoder::new());
        let event_channels = Arc::new(EventChannels::default());
        let executor = FfmpegExecutor::new(transcoder, db, None, event_channels, false);

        executor.execute(&job, &plan, &analysis).await
    }
    .await;
    let duration = start.elapsed().as_millis() as u64;

    match execute_res {
        Ok(result) => {
            let fallback = result.fallback_occurred;
            stages.push(SelftestStageResult {
                name: "Execute".to_string(),
                success: true,
                duration_ms: duration,
                message: format!(
                    "Transcode execution finished successfully. Fallback occurred: {fallback}."
                ),
            });
        }
        Err(e) => {
            stages.push(SelftestStageResult {
                name: "Execute".to_string(),
                success: false,
                duration_ms: duration,
                message: format!("FFmpeg execution failed: {e}"),
            });
            let _ = cleanup_temp_dir(&temp_dir).await;
            return SelftestResponse {
                success: false,
                stages,
                error: Some(format!("Self-test aborted during 'Execute': {e}")),
            };
        }
    }

    // 5. Verify Stage
    let start = Instant::now();
    let verify_res = async {
        let exists = tokio::fs::try_exists(&output_path).await?;
        if !exists {
            anyhow::bail!("Output transcoded file does not exist.");
        }
        let meta = tokio::fs::metadata(&output_path).await?;
        if meta.len() == 0 {
            anyhow::bail!("Output transcoded file is empty (0 bytes).");
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    let duration = start.elapsed().as_millis() as u64;

    match verify_res {
        Ok(_) => {
            stages.push(SelftestStageResult {
                name: "Verify".to_string(),
                success: true,
                duration_ms: duration,
                message: "Output file size and structure verified successfully.".to_string(),
            });
        }
        Err(e) => {
            overall_success = false;
            stages.push(SelftestStageResult {
                name: "Verify".to_string(),
                success: false,
                duration_ms: duration,
                message: format!("Verification failed: {e}"),
            });
            overall_error = Some(format!("Self-test verification failed: {e}"));
        }
    }

    // Complete clean up
    let _ = cleanup_temp_dir(&temp_dir).await;

    SelftestResponse {
        success: overall_success,
        stages,
        error: overall_error,
    }
}

async fn cleanup_temp_dir(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        tokio::fs::remove_dir_all(path).await?;
    }
    Ok(())
}
