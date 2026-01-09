use crate::config::{CpuPreset, QualityProfile};
use crate::error::{AlchemistError, Result};
use crate::media::ffmpeg::{FFmpegCommandBuilder, FFmpegProgress};
use crate::system::hardware::HardwareInfo;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{broadcast, oneshot};
use tracing::{error, info, warn};

pub struct Transcoder {
    cancel_channels: Arc<Mutex<HashMap<i64, oneshot::Sender<()>>>>,
}

impl Default for Transcoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Transcoder {
    pub fn new() -> Self {
        Self {
            cancel_channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn cancel_job(&self, job_id: i64) -> bool {
        let mut channels = self.cancel_channels.lock().unwrap();
        if let Some(tx) = channels.remove(&job_id) {
            let _ = tx.send(());
            true
        } else {
            false
        }
    }

    pub async fn transcode_to_av1(
        &self,
        input: &Path,
        output: &Path,
        hw_info: Option<&HardwareInfo>,
        quality_profile: QualityProfile,
        cpu_preset: CpuPreset,
        dry_run: bool,
        metadata: &crate::media::pipeline::MediaMetadata,
        event_target: Option<(i64, Arc<broadcast::Sender<crate::db::AlchemistEvent>>)>,
    ) -> Result<()> {
        if dry_run {
            info!("[DRY RUN] Transcoding {:?} to {:?}", input, output);
            return Ok(());
        }

        let mut cmd = FFmpegCommandBuilder::new(input, output)
            .with_hardware(hw_info)
            .with_profile(quality_profile)
            .with_cpu_preset(cpu_preset)
            .build();
        cmd.arg(output);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let total_duration = metadata.duration_secs;

        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("Starting transcode:");
        info!("  Input:  {:?}", input);
        info!("  Output: {:?}", output);
        info!("  Duration: {:.2}s", total_duration);
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let mut child = cmd
            .spawn()
            .map_err(|e| AlchemistError::FFmpeg(format!("Failed to spawn FFmpeg: {}", e)))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AlchemistError::FFmpeg("Failed to capture stderr".into()))?;

        let (kill_tx, kill_rx) = oneshot::channel();
        let job_id = event_target.as_ref().map(|(id, _)| *id);

        if let Some(id) = job_id {
            self.cancel_channels.lock().unwrap().insert(id, kill_tx);
        }

        let mut reader = BufReader::new(stderr).lines();
        let event_target_clone = event_target.clone();

        let mut kill_rx = kill_rx;
        let mut killed = false;

        loop {
            tokio::select! {
                line_res = reader.next_line() => {
                    match line_res {
                        Ok(Some(line)) => {
                            if let Some((job_id, ref tx)) = event_target_clone {
                                let _ = tx.send(crate::db::AlchemistEvent::Log { job_id, message: line.clone() });

                                if let Some(progress) = FFmpegProgress::parse_line(&line) {
                                    let percentage: f64 = progress.percentage(total_duration);
                                    let _ = tx.send(crate::db::AlchemistEvent::Progress {
                                        job_id,
                                        percentage,
                                        time: progress.time
                                    });
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            error!("Error reading FFmpeg stderr: {}", e);
                            break;
                        }
                    }
                }
                _ = &mut kill_rx => {
                    warn!("Job {:?} cancelled. Killing FFmpeg process...", job_id);
                    let _ = child.kill().await;
                    killed = true;
                    if let Some(id) = job_id {
                        self.cancel_channels.lock().unwrap().remove(&id);
                    }
                    break;
                }
            }
        }

        let status: std::process::ExitStatus = child.wait().await?;

        if let Some(id) = job_id {
            self.cancel_channels.lock().unwrap().remove(&id);
        }

        if killed {
            return Err(AlchemistError::Cancelled);
        }

        if status.success() {
            info!("Transcode successful: {:?}", output);
            Ok(())
        } else {
            error!("FFmpeg failed with status: {}", status);
            Err(AlchemistError::FFmpeg(format!(
                "FFmpeg failed with status: {}",
                status
            )))
        }
    }

    // Redundant parse_duration removed, use FFmpegProgress instead.
}
