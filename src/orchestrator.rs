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
    // std::sync::Mutex is intentional: critical sections never cross .await boundaries,
    // so there is no deadlock risk. Contention is negligible (≤ concurrent_jobs entries).
    cancel_channels: Arc<Mutex<HashMap<i64, oneshot::Sender<()>>>>,
    pending_cancels: Arc<Mutex<HashSet<i64>>>,
    pub(crate) cancel_requested: Arc<tokio::sync::RwLock<HashSet<i64>>>,
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
    pub clip_start_seconds: Option<f64>,
    pub clip_duration_seconds: Option<f64>,
}

#[allow(async_fn_in_trait)]
#[trait_variant::make(AsyncExecutionObserver: Send)]
pub trait LocalExecutionObserver {
    async fn on_log(&self, message: String);
    async fn on_progress(&self, progress: FFmpegProgress, total_duration: f64);
}

// The transcoder stores a trait object, so keep a dyn-safe adapter at the boundary
// while letting implementers use native async trait methods.
pub trait ExecutionObserver: Send + Sync {
    fn on_log(&self, message: String) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
    fn on_progress(
        &self,
        progress: FFmpegProgress,
        total_duration: f64,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

impl<T> ExecutionObserver for T
where
    T: AsyncExecutionObserver + Send + Sync,
{
    fn on_log(&self, message: String) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(AsyncExecutionObserver::on_log(self, message))
    }

    fn on_progress(
        &self,
        progress: FFmpegProgress,
        total_duration: f64,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(AsyncExecutionObserver::on_progress(
            self,
            progress,
            total_duration,
        ))
    }
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
            cancel_requested: Arc::new(tokio::sync::RwLock::new(HashSet::new())),
        }
    }

    pub async fn is_cancel_requested(&self, job_id: i64) -> bool {
        self.cancel_requested.read().await.contains(&job_id)
    }

    pub async fn remove_cancel_request(&self, job_id: i64) {
        self.cancel_requested.write().await.remove(&job_id);
    }

    pub async fn add_cancel_request(&self, job_id: i64) {
        self.cancel_requested.write().await.insert(job_id);
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
            None => match self.pending_cancels.lock() {
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
            },
        }
    }

    /// Cancel all currently running jobs. Used during graceful shutdown.
    pub fn cancel_all_jobs(&self) -> usize {
        let mut channels = match self.cancel_channels.lock() {
            Ok(channels) => channels,
            Err(e) => {
                error!(
                    "Cancel channels lock poisoned during shutdown, recovering: {}",
                    e
                );
                e.into_inner()
            }
        };
        let count = channels.len();
        for (job_id, tx) in channels.drain() {
            info!("Cancelling job {} for shutdown", job_id);
            let _ = tx.send(());
        }
        count
    }

    /// Returns the number of currently active transcode jobs.
    pub fn active_job_count(&self) -> usize {
        match self.cancel_channels.lock() {
            Ok(channels) => channels.len(),
            Err(e) => e.into_inner().len(),
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
        .with_clip(request.clip_start_seconds, request.clip_duration_seconds)
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
        let ffmpeg_start = std::time::Instant::now();
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

        info!(
            "Job {:?}: FFmpeg spawned ({:.3}s since command start)",
            job_id,
            ffmpeg_start.elapsed().as_secs_f64()
        );
        let mut reader = BufReader::new(stderr).lines();
        let mut kill_rx = kill_rx;
        let mut killed = false;
        let mut last_lines = std::collections::VecDeque::with_capacity(20);
        let mut progress_state = FFmpegProgressState::default();
        let mut first_frame_logged = false;

        loop {
            tokio::select! {
                line_res_timeout = tokio::time::timeout(tokio::time::Duration::from_secs(120), reader.next_line()) => {
                    match line_res_timeout {
                        Ok(line_res) => match line_res {
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

                                // Detect VideoToolbox software fallback
                                if line.contains("Using software encoder") || line.contains("using software encoder") {
                                    warn!(
                                        "Job {:?}: VideoToolbox falling back to software encoder ({}s elapsed)",
                                        job_id,
                                        ffmpeg_start.elapsed().as_secs_f64()
                                    );
                                }

                                if let Some(observer) = observer.as_ref() {
                                    observer.on_log(line.clone()).await;

                                    if let Some(total_duration) = total_duration {
                                        if let Some(progress) = progress_state.ingest_line(&line) {
                                            if !first_frame_logged {
                                                first_frame_logged = true;
                                                info!(
                                                    "Job {:?}: first progress event ({:.3}s since spawn)",
                                                    job_id,
                                                    ffmpeg_start.elapsed().as_secs_f64()
                                                );
                                            }
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
                        },
                        Err(_) => {
                            error!("Job {:?} stalled: No output from FFmpeg for 2 minutes. Killing process...", job_id);
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
            info!(
                "Job {:?}: FFmpeg completed successfully ({:.3}s total)",
                job_id,
                ffmpeg_start.elapsed().as_secs_f64()
            );
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
