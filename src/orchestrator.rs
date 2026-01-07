use crate::db::AlchemistEvent;
use crate::error::{AlchemistError, Result};
use crate::hardware::{HardwareInfo, Vendor};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, oneshot};
use tracing::{error, info, warn};

pub struct Transcoder {
    cancel_channels: Arc<Mutex<HashMap<i64, oneshot::Sender<()>>>>,
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
        cpu_preset: &str,
        dry_run: bool,
        metadata: &crate::analyzer::MediaMetadata,
        event_target: Option<(i64, Arc<broadcast::Sender<AlchemistEvent>>)>,
    ) -> Result<()> {
        if dry_run {
            info!("[DRY RUN] Transcoding {:?} to {:?}", input, output);
            return Ok(());
        }

        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-hide_banner").arg("-y").arg("-i").arg(input);

        let total_duration = metadata.format.duration.parse::<f64>().unwrap_or(0.0);

        // Select encoder based on hardware
        if let Some(info) = hw_info {
            info!("Encoder selection: Hardware acceleration ({})", info.vendor);
            match info.vendor {
                Vendor::Intel => {
                    info!("  Using: av1_qsv (Intel Quick Sync)");
                    if let Some(ref device_path) = info.device_path {
                        cmd.arg("-init_hw_device")
                            .arg(format!("qsv=qsv:{}", device_path));
                        cmd.arg("-filter_hw_device").arg("qsv");
                    }
                    cmd.arg("-c:v").arg("av1_qsv");
                    cmd.arg("-global_quality").arg("25");
                    cmd.arg("-look_ahead").arg("1");
                }
                Vendor::Nvidia => {
                    info!("  Using: av1_nvenc (NVIDIA NVENC)");
                    cmd.arg("-c:v").arg("av1_nvenc");
                    cmd.arg("-preset").arg("p4");
                    cmd.arg("-cq").arg("25");
                }
                Vendor::Apple => {
                    info!("  Using: av1_videotoolbox (Apple VideoToolbox)");
                    cmd.arg("-c:v").arg("av1_videotoolbox");
                }
                Vendor::Amd => {
                    info!("  Using: av1_vaapi (AMD VAAPI)");
                    if let Some(ref device_path) = info.device_path {
                        cmd.arg("-vaapi_device").arg(device_path);
                    }
                    cmd.arg("-c:v").arg("av1_vaapi");
                }
                Vendor::Cpu => {
                    warn!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    warn!("  SOFTWARE ENCODING (CPU) - SLOW PERFORMANCE");
                    warn!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    info!("  Using: libsvtav1 (Software AV1 encoder)");
                    
                    let (preset_val, crf_val) = match cpu_preset {
                        "slow" => ("4", "28"),
                        "medium" => ("8", "32"),
                        "fast" => ("12", "35"),
                        "faster" => ("13", "38"),
                        _ => ("8", "32"), // Default to medium
                    };

                    info!("  Preset: {} ({})", preset_val, cpu_preset);
                    info!("  CRF:    {}", crf_val);
                    info!("  This will be significantly slower than GPU encoding.");

                    cmd.arg("-c:v").arg("libsvtav1");
                    cmd.arg("-preset").arg(preset_val);
                    cmd.arg("-crf").arg(crf_val);
                    cmd.arg("-svtav1-params").arg("tune=0:film-grain=8");
                }
            }
        } else {
            // Fallback if no hardware info (shouldn't happen now)
            warn!("No hardware info provided, using legacy fallback");
            cmd.arg("-c:v").arg("libsvtav1");
            cmd.arg("-preset").arg("8");
            cmd.arg("-crf").arg("32");
        }

        cmd.arg("-c:a").arg("copy");
        cmd.arg("-c:s").arg("copy");
        cmd.arg(output);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

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

        let total_duration = metadata.format.duration.parse::<f64>().unwrap_or(0.0);
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
                                let _ = tx.send(AlchemistEvent::Log { job_id, message: line.clone() });

                                if line.contains("time=") {
                                    if let Some(time_part) = line.split("time=").nth(1) {
                                        let time_str = time_part.split_whitespace().next().unwrap_or("");
                                        let current_time = Self::parse_duration(time_str);
                                        let percentage = if total_duration > 0.0 {
                                            (current_time / total_duration * 100.0).min(100.0)
                                        } else {
                                            0.0
                                        };
                                        let _ = tx.send(AlchemistEvent::Progress { job_id, percentage, time: time_str.to_string() });
                                    }
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

        let status = child.wait().await?;

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

    fn parse_duration(s: &str) -> f64 {
        // HH:MM:SS.ms
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return 0.0;
        }

        let hours = parts[0].parse::<f64>().unwrap_or(0.0);
        let minutes = parts[1].parse::<f64>().unwrap_or(0.0);
        let seconds = parts[2].parse::<f64>().unwrap_or(0.0);

        hours * 3600.0 + minutes * 60.0 + seconds
    }
}
