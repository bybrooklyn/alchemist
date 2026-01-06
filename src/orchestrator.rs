use std::path::Path;
use std::process::{Command, Stdio};
use anyhow::{anyhow, Result};
use tracing::{info, error};
use crate::hardware::{Vendor, HardwareInfo};
use crate::server::AlchemistEvent;
use tokio::sync::broadcast;
use std::sync::Arc;

pub struct Orchestrator {
    ffmpeg_path: String,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            ffmpeg_path: "ffmpeg".to_string(),
        }
    }

    pub fn transcode_to_av1(&self, input: &Path, output: &Path, hw_info: Option<&HardwareInfo>, dry_run: bool, metadata: &crate::analyzer::MediaMetadata, event_target: Option<(i64, Arc<broadcast::Sender<AlchemistEvent>>)>) -> Result<()> {
        let mut args = vec![
            "-hide_banner".to_string(),
            "-y".to_string(),
        ];

        // Vendor-specific setup
        if let Some(hw) = hw_info {
            match hw.vendor {
                Vendor::Intel => {
                    args.extend_from_slice(&[
                        "-hwaccel".to_string(), "qsv".to_string(),
                        "-qsv_device".to_string(), hw.device_path.as_ref().cloned().unwrap_or_else(|| "/dev/dri/renderD128".to_string()),
                        "-i".to_string(), input.to_str().ok_or_else(|| anyhow!("Invalid input path"))?.to_string(),
                        "-c:v".to_string(), "av1_qsv".to_string(),
                        "-preset".to_string(), "medium".to_string(),
                        "-global_quality".to_string(), "25".to_string(),
                        "-pix_fmt".to_string(), "p010le".to_string(),
                    ]);
                }
                Vendor::Nvidia => {
                    args.extend_from_slice(&[
                        "-hwaccel".to_string(), "cuda".to_string(),
                        "-i".to_string(), input.to_str().ok_or_else(|| anyhow!("Invalid input path"))?.to_string(),
                        "-c:v".to_string(), "av1_nvenc".to_string(),
                        "-preset".to_string(), "p4".to_string(),
                        "-cq".to_string(), "24".to_string(),
                        "-pix_fmt".to_string(), "p010le".to_string(),
                    ]);
                }
                Vendor::Amd => {
                    args.extend_from_slice(&[
                        "-hwaccel".to_string(), "vaapi".to_string(),
                        "-vaapi_device".to_string(), hw.device_path.as_ref().cloned().unwrap_or_else(|| "/dev/dri/renderD128".to_string()),
                        "-hwaccel_output_format".to_string(), "vaapi".to_string(),
                        "-i".to_string(), input.to_str().ok_or_else(|| anyhow!("Invalid input path"))?.to_string(),
                        "-vf".to_string(), "format=nv12|vaapi,hwupload".to_string(),
                        "-c:v".to_string(), "av1_vaapi".to_string(),
                        "-qp".to_string(), "25".to_string(),
                    ]);
                }
                Vendor::Apple => {
                    args.extend_from_slice(&[
                        "-i".to_string(), input.to_str().ok_or_else(|| anyhow!("Invalid input path"))?.to_string(),
                        "-c:v".to_string(), "av1_videotoolbox".to_string(),
                        "-bitrate".to_string(), "6M".to_string(),
                        "-pix_fmt".to_string(), "p010le".to_string(),
                    ]);
                }
            }
        } else {
            // CPU fallback (libaom-av1) - VERY SLOW, but requested via allow_cpu_fallback
            args.extend_from_slice(&[
                "-i".to_string(), input.to_str().ok_or_else(|| anyhow!("Invalid input path"))?.to_string(),
                "-c:v".to_string(), "libaom-av1".to_string(),
                "-crf".to_string(), "30".to_string(),
                "-cpu-used".to_string(), "8".to_string(), // Faster preset for CPU
                "-pix_fmt".to_string(), "yuv420p10le".to_string(),
            ]);
        }

        // Audio and Subtitle Mapping
        args.extend_from_slice(&["-map".to_string(), "0:v:0".to_string()]);

        let mut audio_count = 0;
        for (i, stream) in metadata.streams.iter().enumerate() {
            if stream.codec_type == "audio" {
                args.extend_from_slice(&["-map".to_string(), format!("0:a:{}", audio_count)]);
                if crate::analyzer::Analyzer::should_transcode_audio(stream) {
                    args.extend_from_slice(&[format!("-c:a:{}", audio_count), "libopus".to_string(), format!("-b:a:{}", audio_count), "192k".to_string()]);
                } else {
                    args.extend_from_slice(&[format!("-c:a:{}", audio_count), "copy".to_string()]);
                }
                audio_count += 1;
            } else if stream.codec_type == "subtitle" {
                args.extend_from_slice(&["-map".to_string(), format!("0:s:{}", i - audio_count - 1)]); // Simplified mapping
                args.extend_from_slice(&["-c:s".to_string(), "copy".to_string()]);
            }
        }

        // If no subtitles were found or mapping is complex, fallback to a simpler copy all if needed
        // But for now, let's just map all and copy.
        
        args.push(output.to_str().ok_or_else(|| anyhow!("Invalid output path"))?.to_string());

        info!("Command: {} {}", self.ffmpeg_path, args.join(" "));

        if dry_run {
            info!("Dry run: Skipping actual execution.");
            return Ok(());
        }

        let mut child = Command::new(&self.ffmpeg_path)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        let stderr = child.stderr.take().ok_or_else(|| anyhow!("Failed to capture stderr"))?;
        let reader = std::io::BufReader::new(stderr);
        use std::io::BufRead;

        for line in reader.lines() {
            let line = line?;
            if let Some((job_id, ref tx)) = event_target {
                let _ = tx.send(AlchemistEvent::Log { job_id, message: line.clone() });
                
                if line.contains("time=") {
                    // simple parse for time=00:00:00.00
                    if let Some(time_part) = line.split("time=").nth(1) {
                        let time_str = time_part.split_whitespace().next().unwrap_or("");
                        info!("Progress: time={}", time_str);
                        let _ = tx.send(AlchemistEvent::Progress { job_id, percentage: 0.0, time: time_str.to_string() });
                    }
                }
            } else if line.contains("time=") {
                // simple parse for time=00:00:00.00
                if let Some(time_part) = line.split("time=").nth(1) {
                    let time_str = time_part.split_whitespace().next().unwrap_or("");
                    info!("Progress: time={}", time_str);
                }
            }
        }

        let status = child.wait()?;

        if status.success() {
            info!("Transcode successful: {:?}", output);
            Ok(())
        } else {
            error!("FFmpeg failed with exit code: {:?}", status.code());
            Err(anyhow!("FFmpeg execution failed"))
        }
    }
}
