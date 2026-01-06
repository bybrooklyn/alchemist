use std::path::Path;
use std::process::{Command, Stdio};
use anyhow::{anyhow, Result};
use tracing::{info, error};
use crate::hardware::{Vendor, HardwareInfo};

pub struct Orchestrator {
    ffmpeg_path: String,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            ffmpeg_path: "ffmpeg".to_string(),
        }
    }

    pub fn transcode_to_av1(&self, input: &Path, output: &Path, hw_info: &HardwareInfo, dry_run: bool) -> Result<()> {
        let mut args = vec![
            "-hide_banner".to_string(),
            "-y".to_string(),
        ];

        // Vendor-specific setup
        match hw_info.vendor {
            Vendor::Intel => {
                args.extend_from_slice(&[
                    "-hwaccel".to_string(), "qsv".to_string(),
                    "-qsv_device".to_string(), hw_info.device_path.as_ref().cloned().unwrap_or_else(|| "/dev/dri/renderD128".to_string()),
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
                // Assuming VAAPI for AMD on Linux as it's more common than AMF CLI support in many ffmpeg builds
                args.extend_from_slice(&[
                    "-hwaccel".to_string(), "vaapi".to_string(),
                    "-vaapi_device".to_string(), hw_info.device_path.as_ref().cloned().unwrap_or_else(|| "/dev/dri/renderD128".to_string()),
                    "-hwaccel_output_format".to_string(), "vaapi".to_string(),
                    "-i".to_string(), input.to_str().ok_or_else(|| anyhow!("Invalid input path"))?.to_string(),
                    "-vf".to_string(), "format=nv12|vaapi,hwupload".to_string(),
                    "-c:v".to_string(), "av1_vaapi".to_string(),
                    "-qp".to_string(), "25".to_string(),
                ]);
            }
        }

        // Common arguments
        args.extend_from_slice(&[
            "-c:a".to_string(), "copy".to_string(),
            output.to_str().ok_or_else(|| anyhow!("Invalid output path"))?.to_string(),
        ]);

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
