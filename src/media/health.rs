use crate::error::{AlchemistError, Result};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

pub struct HealthChecker;

impl HealthChecker {
    pub async fn check_file(path: &Path) -> Result<Option<String>> {
        let mut command = Command::new("ffmpeg");
        command
            .kill_on_drop(true)
            .args(["-v", "error", "-i"])
            .arg(path)
            .args(["-f", "null", "-"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let child = command.spawn().map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => AlchemistError::FFmpegNotFound,
            _ => AlchemistError::FFmpeg(format!(
                "Failed to start library health check for {}: {}",
                path.display(),
                err
            )),
        })?;

        let output = match timeout(Duration::from_secs(60), child.wait_with_output()).await {
            Ok(result) => result.map_err(AlchemistError::Io)?,
            Err(_) => {
                return Err(AlchemistError::FFmpeg(format!(
                    "Library health check timed out after 60 seconds for {}",
                    path.display()
                )));
            }
        };

        let stderr_output = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if output.status.success() && stderr_output.is_empty() {
            return Ok(None);
        }

        if !stderr_output.is_empty() {
            return Ok(Some(stderr_output));
        }

        Ok(Some(format!(
            "ffmpeg exited with status {}",
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        )))
    }
}
