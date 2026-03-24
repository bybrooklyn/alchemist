use crate::error::{AlchemistError, Result};
use crate::media::ffmpeg::{FFmpegCommandBuilder, FFmpegProgress, FFmpegProgressState};
use crate::media::pipeline::TranscodePlan;
use crate::system::hardware::HardwareInfo;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::oneshot;
use tracing::{error, info, warn};

pub struct Transcoder {
    cancel_channels: Arc<Mutex<HashMap<i64, oneshot::Sender<()>>>>,
    pending_cancels: Arc<Mutex<HashSet<i64>>>,
}

pub struct TranscodeRequest<'a> {
    pub job_id: Option<i64>,
    pub input: &'a Path,
    pub output: &'a Path,
    pub hw_info: Option<&'a HardwareInfo>,
    pub dry_run: bool,
    pub metadata: &'a crate::media::pipeline::MediaMetadata,
    pub plan: &'a TranscodePlan,
    pub observer: Option<Arc<dyn ExecutionObserver>>,
}

pub trait ExecutionObserver: Send + Sync {
    fn on_log(&self, message: String) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
    fn on_progress(
        &self,
        progress: FFmpegProgress,
        total_duration: f64,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
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
            pending_cancels: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn cancel_job(&self, job_id: i64) -> bool {
        let mut channels = match self.cancel_channels.lock() {
            Ok(channels) => channels,
            Err(e) => {
                error!("Cancel channels lock poisoned, recovering: {}", e);
                e.into_inner()
            }
        };
        match channels.remove(&job_id) {
            Some(tx) => {
                let _ = tx.send(());
                true
            }
            None => {
                drop(channels);
                match self.pending_cancels.lock() {
                    Ok(mut pending) => {
                        pending.insert(job_id);
                        true
                    }
                    Err(e) => {
                        error!("Pending cancels lock poisoned, recovering: {}", e);
                        let mut pending = e.into_inner();
                        pending.insert(job_id);
                        true
                    }
                }
            }
        }
    }

    pub async fn transcode_media(&self, request: TranscodeRequest<'_>) -> Result<()> {
        if request.dry_run {
            info!(
                "[DRY RUN] Transcoding {:?} to {:?} with {:?}",
                request.input, request.output, request.plan.encoder
            );
            return Ok(());
        }

        if request.input == request.output {
            return Err(AlchemistError::Config(
                "Output path matches input path".into(),
            ));
        }

        if let Some(parent) = request.output.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    error!("Failed to create output directory {:?}: {}", parent, e);
                    AlchemistError::FFmpeg(format!(
                        "Failed to create output directory {:?}: {}",
                        parent, e
                    ))
                })?;
            }
        }

        let cmd = FFmpegCommandBuilder::new(
            request.input,
            request.output,
            request.metadata,
            request.plan,
        )
        .with_hardware(request.hw_info)
        .build()?;

        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("Starting transcode:");
        info!("  Input:  {:?}", request.input);
        info!("  Output: {:?}", request.output);
        info!("  Encoder: {:?}", request.plan.encoder);
        info!("  Duration: {:.2}s", request.metadata.duration_secs);
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        self.run_ffmpeg_command(
            cmd,
            request.job_id,
            request.observer,
            Some(request.metadata.duration_secs),
        )
        .await
    }

    pub async fn extract_subtitles(&self, request: TranscodeRequest<'_>) -> Result<()> {
        if request.dry_run {
            info!("[DRY RUN] Extracting subtitles from {:?}", request.input);
            return Ok(());
        }

        let builder = FFmpegCommandBuilder::new(
            request.input,
            request.output,
            request.metadata,
            request.plan,
        )
        .with_hardware(request.hw_info);
        let Some(args) = builder.build_subtitle_extract_args()? else {
            return Ok(());
        };

        for sidecar_output in request.plan.subtitles.sidecar_outputs() {
            if let Some(parent) = sidecar_output.temp_path.parent() {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent).await.map_err(|e| {
                        AlchemistError::FFmpeg(format!(
                            "Failed to create subtitle output directory {:?}: {}",
                            parent, e
                        ))
                    })?;
                }
            }
        }

        let mut cmd = tokio::process::Command::new("ffmpeg");
        cmd.args(&args);
        self.run_ffmpeg_command(cmd, request.job_id, request.observer, None)
            .await
    }

    async fn run_ffmpeg_command(
        &self,
        mut cmd: tokio::process::Command,
        job_id: Option<i64>,
        observer: Option<Arc<dyn ExecutionObserver>>,
        total_duration: Option<f64>,
    ) -> Result<()> {
        info!("Executing FFmpeg command: {:?}", cmd);
        cmd.stdout(Stdio::null()).stderr(Stdio::piped());

        if let Some(id) = job_id {
            let mut pending = match self.pending_cancels.lock() {
                Ok(pending) => pending,
                Err(e) => {
                    error!("Pending cancels lock poisoned, recovering: {}", e);
                    e.into_inner()
                }
            };
            if pending.remove(&id) {
                warn!("Job {id} cancelled before FFmpeg spawn");
                return Err(AlchemistError::Cancelled);
            }
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| AlchemistError::FFmpeg(format!("Failed to spawn FFmpeg: {}", e)))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AlchemistError::FFmpeg("Failed to capture stderr".into()))?;

        let (kill_tx, kill_rx) = oneshot::channel();

        if let Some(id) = job_id {
            match self.cancel_channels.lock() {
                Ok(mut channels) => {
                    channels.insert(id, kill_tx);
                }
                Err(e) => {
                    error!("Cancel channels lock poisoned, recovering: {}", e);
                    e.into_inner().insert(id, kill_tx);
                }
            }
            let mut pending = match self.pending_cancels.lock() {
                Ok(pending) => pending,
                Err(e) => {
                    error!("Pending cancels lock poisoned, recovering: {}", e);
                    e.into_inner()
                }
            };
            if pending.remove(&id) {
                if let Ok(mut channels) = self.cancel_channels.lock() {
                    if let Some(tx) = channels.remove(&id) {
                        let _ = tx.send(());
                    }
                }
            }
        }

        let mut reader = BufReader::new(stderr).lines();
        let mut kill_rx = kill_rx;
        let mut killed = false;
        let mut last_lines = std::collections::VecDeque::with_capacity(20);
        let mut progress_state = FFmpegProgressState::default();

        loop {
            tokio::select! {
                line_res = reader.next_line() => {
                    match line_res {
                        Ok(Some(line)) => {
                            let line = if line.len() > 4096 {
                                format!("{}...[truncated]", &line[..4096])
                            } else {
                                line
                            };
                            last_lines.push_back(line.clone());
                            if last_lines.len() > 20 {
                                last_lines.pop_front();
                            }

                            if let Some(observer) = observer.as_ref() {
                                observer.on_log(line.clone()).await;

                                if let Some(total_duration) = total_duration {
                                    if let Some(progress) = progress_state.ingest_line(&line) {
                                        observer.on_progress(progress, total_duration).await;
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
                        match self.cancel_channels.lock() {
                            Ok(mut channels) => { channels.remove(&id); }
                            Err(e) => { e.into_inner().remove(&id); }
                        }
                    }
                    break;
                }
            }
        }

        let status = child.wait().await?;

        if let Some(id) = job_id {
            match self.cancel_channels.lock() {
                Ok(mut channels) => {
                    channels.remove(&id);
                }
                Err(e) => {
                    e.into_inner().remove(&id);
                }
            }
            match self.pending_cancels.lock() {
                Ok(mut pending) => {
                    pending.remove(&id);
                }
                Err(e) => {
                    e.into_inner().remove(&id);
                }
            }
        }

        if killed {
            return Err(AlchemistError::Cancelled);
        }

        if status.success() {
            info!("FFmpeg command completed successfully");
            Ok(())
        } else {
            let error_detail = last_lines.make_contiguous().join("\n");
            error!(
                "FFmpeg failed with status: {}\nDetails:\n{}",
                status, error_detail
            );
            Err(AlchemistError::FFmpeg(format!(
                "FFmpeg failed ({}). Last output:\n{}",
                status, error_detail
            )))
        }
    }
}
