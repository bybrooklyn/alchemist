use crate::error::{AlchemistError, Result};
use crate::media::pipeline::{
    AnalysisConfidence, AnalysisWarning, Analyzer as AnalyzerTrait, AnalyzerLabel, AnalyzerMetrics,
    AnalyzerReport, AudioStreamMetadata, DynamicRange, MediaAnalysis, MediaMetadata,
    SubtitleStreamMetadata, TranscodeDecision,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::UNIX_EPOCH;
use tokio::process::Command;
use tokio::sync::OnceCell;

const FFPROBE_TIMEOUT_SECS: u64 = 30;
const FFPROBE_ANALYZE_ARGS: &[&str] = &[
    "-v",
    "quiet",
    "-analyzeduration",
    "1M",
    "-probesize",
    "1M",
    "-print_format",
    "json",
    "-show_entries",
    "format=duration,size,bit_rate,format_name,format_long_name:stream=codec_type,codec_name,pix_fmt,width,height,coded_width,coded_height,bit_rate,bits_per_raw_sample,channel_layout,channels,avg_frame_rate,r_frame_rate,nb_frames,duration,disposition,color_primaries,color_transfer,color_space,color_range,field_order:stream_side_data=side_data_type:stream_tags=language,title:chapter=id",
];
const ANALYZER_REPORT_CACHE_SCHEMA: &str = "analysis_report_v1";
static FFPROBE_VERSION_MARKER: OnceCell<String> = OnceCell::const_new();

async fn run_ffprobe(args: &[&str], path: &Path) -> Result<std::process::Output> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(FFPROBE_TIMEOUT_SECS),
        Command::new("ffprobe").args(args).arg(path).output(),
    )
    .await
    {
        Ok(Ok(output)) => {
            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(AlchemistError::Analyzer(format!("ffprobe failed: {}", err)));
            }
            Ok(output)
        }
        Ok(Err(e)) => Err(AlchemistError::Analyzer(format!(
            "Failed to run ffprobe: {}",
            e
        ))),
        Err(_) => Err(AlchemistError::Analyzer(format!(
            "ffprobe timed out after {}s: {}",
            FFPROBE_TIMEOUT_SECS,
            path.display()
        ))),
    }
}

#[derive(Debug, Clone)]
struct ProbeCacheKey {
    input_path: String,
    mtime_ns: i64,
    size_bytes: i64,
    probe_version: String,
    file_id: Option<String>,
}

async fn ffprobe_version_marker() -> String {
    FFPROBE_VERSION_MARKER
        .get_or_init(|| async {
            match Command::new("ffprobe").arg("-version").output().await {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    stdout
                        .lines()
                        .next()
                        .map(|line| line.trim().to_string())
                        .filter(|line| !line.is_empty())
                        .map(|line| format!("{line}|{ANALYZER_REPORT_CACHE_SCHEMA}"))
                        .unwrap_or_else(|| {
                            format!("ffprobe:unknown|{ANALYZER_REPORT_CACHE_SCHEMA}")
                        })
                }
                _ => format!("ffprobe:unknown|{ANALYZER_REPORT_CACHE_SCHEMA}"),
            }
        })
        .await
        .clone()
}

fn file_mtime_ns(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_nanos()).ok())
        .unwrap_or(0)
}

fn file_size_i64(metadata: &std::fs::Metadata) -> i64 {
    i64::try_from(metadata.len()).unwrap_or(i64::MAX)
}

/// Optional secondary identity hint for the probe cache (PERF-3).
/// Unix: device + inode (`MetadataExt::dev()` + `ino()`) — the inode number
/// alone is only unique within a single filesystem, so a mount/snapshot swap
/// could reuse it; pairing it with the device id gives the full POSIX file
/// identity. Windows: not exposed by stdlib without extra ffi work, so we
/// return `None` and rely on (path, size, mtime) alone. Returning `None`
/// always degrades gracefully.
fn file_id_marker(metadata: &std::fs::Metadata) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(format!("dev:{}:ino:{}", metadata.dev(), metadata.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FfprobeMetadata {
    pub streams: Vec<Stream>,
    pub format: Format,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Stream {
    pub codec_name: String,
    pub codec_type: String,
    pub pix_fmt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub coded_width: Option<u32>,
    pub coded_height: Option<u32>,
    pub bit_rate: Option<String>,
    pub bits_per_raw_sample: Option<String>,
    pub channel_layout: Option<String>,
    pub channels: Option<u32>,
    pub avg_frame_rate: Option<String>,
    pub r_frame_rate: Option<String>,
    pub nb_frames: Option<String>,
    pub duration: Option<String>,
    pub disposition: Option<Disposition>,
    pub tags: Option<Tags>,
    pub color_primaries: Option<String>,
    pub color_transfer: Option<String>,
    pub color_space: Option<String>,
    pub color_range: Option<String>,
    pub field_order: Option<String>,
    #[serde(default)]
    pub side_data_list: Vec<SideData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SideData {
    pub side_data_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Disposition {
    pub default: Option<i32>,
    pub forced: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tags {
    pub language: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Format {
    pub format_name: String,
    pub format_long_name: Option<String>,
    pub duration: String,
    pub size: String,
    pub bit_rate: String,
}

pub struct FfmpegAnalyzer;

impl AnalyzerTrait for FfmpegAnalyzer {
    async fn analyze(&self, path: &Path) -> Result<MediaAnalysis> {
        let path = path.to_path_buf();

        let output = run_ffprobe(FFPROBE_ANALYZE_ARGS, &path).await?;

        tokio::task::spawn_blocking(move || {
            let metadata: FfprobeMetadata =
                serde_json::from_slice(&output.stdout).map_err(|e| {
                    AlchemistError::Analyzer(format!("Failed to parse ffprobe JSON: {}", e))
                })?;

            let video_stream = select_video_stream(&metadata.streams)
                .ok_or_else(|| AlchemistError::Analyzer("No video stream found".to_string()))?;

            let audio_stream = metadata.streams.iter().find(|s| s.codec_type == "audio");
            let audio_bitrate_bps = audio_stream
                .and_then(|stream| stream.bit_rate.as_deref())
                .and_then(parse_u64);
            let audio_is_heavy = audio_stream
                .map(Analyzer::should_transcode_audio)
                .unwrap_or(false);
            let subtitle_streams = metadata
                .streams
                .iter()
                .filter(|s| s.codec_type == "subtitle")
                .enumerate()
                .map(|(stream_index, stream)| SubtitleStreamMetadata {
                    stream_index,
                    codec_name: stream.codec_name.clone(),
                    language: stream.tags.as_ref().and_then(|tags| tags.language.clone()),
                    title: stream.tags.as_ref().and_then(|tags| tags.title.clone()),
                    default: stream
                        .disposition
                        .as_ref()
                        .and_then(|disposition| disposition.default)
                        .unwrap_or(0)
                        == 1,
                    forced: stream
                        .disposition
                        .as_ref()
                        .and_then(|disposition| disposition.forced)
                        .unwrap_or(0)
                        == 1,
                    burnable: subtitle_codec_is_burnable(&stream.codec_name),
                })
                .collect::<Vec<_>>();
            let audio_streams = metadata
                .streams
                .iter()
                .filter(|s| s.codec_type == "audio")
                .enumerate()
                .map(|(stream_index, stream)| AudioStreamMetadata {
                    stream_index,
                    codec_name: stream.codec_name.clone(),
                    language: stream.tags.as_ref().and_then(|tags| tags.language.clone()),
                    title: stream.tags.as_ref().and_then(|tags| tags.title.clone()),
                    channels: stream.channels,
                    default: stream
                        .disposition
                        .as_ref()
                        .and_then(|disposition| disposition.default)
                        .unwrap_or(0)
                        == 1,
                    forced: stream
                        .disposition
                        .as_ref()
                        .and_then(|disposition| disposition.forced)
                        .unwrap_or(0)
                        == 1,
                })
                .collect::<Vec<_>>();

            let color_transfer = video_stream.color_transfer.clone();
            let color_primaries = video_stream.color_primaries.clone();
            let dynamic_range =
                detect_dynamic_range(color_transfer.as_deref(), color_primaries.as_deref());

            let mut warnings = Vec::new();

            let fps_from_average_rate = video_stream
                .avg_frame_rate
                .as_deref()
                .and_then(Analyzer::parse_fps);
            let fps_from_frame_count =
                fps_from_frame_count(video_stream, parse_f64(&metadata.format.duration));
            let fps = selected_metadata_fps(video_stream, fps_from_frame_count);

            let duration_secs = parse_f64(&metadata.format.duration)
                .or_else(|| video_stream.duration.as_deref().and_then(parse_f64))
                .or_else(|| {
                    let frames = video_stream.nb_frames.as_deref().and_then(parse_f64)?;
                    if fps > 0.0 { Some(frames / fps) } else { None }
                })
                .unwrap_or(0.0);

            if video_stream.bit_rate.is_none() {
                warnings.push(AnalysisWarning::MissingVideoBitrate);
            }
            if metadata.format.bit_rate.parse::<u64>().is_err() {
                warnings.push(AnalysisWarning::MissingContainerBitrate);
            }
            if fps <= 0.0 {
                warnings.push(AnalysisWarning::MissingFps);
            }
            if duration_secs <= 0.0 {
                warnings.push(AnalysisWarning::MissingDuration);
            }
            if infer_bit_depth(video_stream).is_none() {
                warnings.push(AnalysisWarning::MissingBitDepth);
            }
            if video_stream
                .pix_fmt
                .as_deref()
                .is_some_and(|pix_fmt| bit_depth_from_pix_fmt(pix_fmt).is_none())
            {
                warnings.push(AnalysisWarning::UnrecognizedPixelFormat);
            }

            let confidence = if warnings.is_empty() {
                AnalysisConfidence::High
            } else if warnings.len() >= 3 {
                AnalysisConfidence::Low
            } else {
                AnalysisConfidence::Medium
            };

            let media_metadata = MediaMetadata {
                path: path.clone(),
                duration_secs,
                codec_name: video_stream.codec_name.clone(),
                width: video_stream.width.or(video_stream.coded_width).unwrap_or(0),
                height: video_stream
                    .height
                    .or(video_stream.coded_height)
                    .unwrap_or(0),
                bit_depth: infer_bit_depth(video_stream),
                color_primaries,
                color_transfer,
                color_space: video_stream.color_space.clone(),
                color_range: video_stream.color_range.clone(),
                dynamic_range,
                size_bytes: metadata.format.size.parse().unwrap_or(0),
                video_bitrate_bps: video_stream.bit_rate.as_deref().and_then(parse_u64),
                container_bitrate_bps: parse_u64(&metadata.format.bit_rate),
                fps,
                container: metadata.format.format_name.clone(),
                audio_codec: audio_stream.map(|s| s.codec_name.clone()),
                audio_bitrate_bps,
                audio_channels: audio_stream.and_then(|s| s.channels),
                audio_is_heavy,
                subtitle_streams,
                audio_streams,
                chapter_count: u32::try_from(metadata.chapters.len()).unwrap_or(u32::MAX),
            };

            let analysis_report = build_analyzer_report(
                &media_metadata,
                &warnings,
                &metadata.streams,
                video_stream,
                fps_from_average_rate,
                fps_from_frame_count,
            );

            Ok(MediaAnalysis {
                metadata: media_metadata,
                warnings,
                confidence,
                analysis_report,
            })
        })
        .await
        .map_err(|e| AlchemistError::Analyzer(format!("spawn_blocking failed: {}", e)))?
    }
}

impl FfmpegAnalyzer {
    async fn probe_cache_key_for_path(path: &Path) -> Result<ProbeCacheKey> {
        let fs_metadata = tokio::fs::metadata(path).await.map_err(|err| {
            AlchemistError::Analyzer(format!(
                "Failed to stat input for probe cache key ({}): {}",
                path.display(),
                err
            ))
        })?;
        let probe_version = ffprobe_version_marker().await;
        Ok(ProbeCacheKey {
            input_path: path.to_string_lossy().to_string(),
            mtime_ns: file_mtime_ns(&fs_metadata),
            size_bytes: file_size_i64(&fs_metadata),
            probe_version,
            file_id: file_id_marker(&fs_metadata),
        })
    }

    pub async fn analyze_with_cache(
        &self,
        db: &crate::db::Db,
        path: &Path,
    ) -> Result<MediaAnalysis> {
        let cache_key = Self::probe_cache_key_for_path(path).await?;
        let cached_json = match db
            .get_media_probe_cache_with_file_id(
                &cache_key.input_path,
                cache_key.mtime_ns,
                cache_key.size_bytes,
                &cache_key.probe_version,
                cache_key.file_id.as_deref(),
            )
            .await
        {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(
                    input_path = %cache_key.input_path,
                    "Media probe cache lookup failed; reprobe fallback: {err}"
                );
                None
            }
        };

        if let Some(json) = cached_json {
            match serde_json::from_str::<MediaAnalysis>(&json) {
                Ok(analysis) => {
                    tracing::debug!(input_path = %cache_key.input_path, "Media probe cache hit");
                    return Ok(analysis);
                }
                Err(err) => {
                    tracing::warn!(
                        input_path = %cache_key.input_path,
                        "Media probe cache decode failed; reprobe fallback: {err}"
                    );
                }
            }
        }

        let analysis = self.analyze(path).await?;

        match serde_json::to_string(&analysis) {
            Ok(serialized) => {
                if let Err(err) = db
                    .upsert_media_probe_cache_with_file_id(
                        &cache_key.input_path,
                        cache_key.mtime_ns,
                        cache_key.size_bytes,
                        &cache_key.probe_version,
                        &serialized,
                        cache_key.file_id.as_deref(),
                    )
                    .await
                {
                    tracing::warn!(
                        input_path = %cache_key.input_path,
                        "Media probe cache write failed: {err}"
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    input_path = %cache_key.input_path,
                    "Media probe cache serialization failed: {err}"
                );
            }
        }

        Ok(analysis)
    }
}

pub struct Analyzer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputProbe {
    pub codec_name: String,
    pub stream_encoder_tag: Option<String>,
    pub format_encoder_tag: Option<String>,
}

impl Analyzer {
    /// Async version of probe that doesn't block the runtime
    pub async fn probe_async(path: &Path) -> Result<FfprobeMetadata> {
        let output = run_ffprobe(
            &[
                "-v",
                "error",
                "-analyzeduration",
                "1M",
                "-probesize",
                "1M",
                "-print_format",
                "json",
                "-show_entries",
                "format=duration,size,bit_rate,format_name,format_long_name:stream=codec_type,codec_name,pix_fmt,width,height,coded_width,coded_height,bit_rate,bits_per_raw_sample,channel_layout,channels,avg_frame_rate,r_frame_rate,nb_frames,duration,disposition,color_primaries,color_transfer,color_space,color_range,field_order:stream_side_data=side_data_type:stream_tags=language,title:chapter=id",
            ],
            path,
        )
        .await?;

        let metadata: FfprobeMetadata = serde_json::from_slice(&output.stdout).map_err(|e| {
            AlchemistError::Analyzer(format!("Failed to parse ffprobe JSON: {}", e))
        })?;

        Ok(metadata)
    }

    pub async fn probe_chapter_count(path: &Path) -> Result<u32> {
        #[derive(Deserialize)]
        struct ChapterProbe {
            #[serde(default)]
            chapters: Vec<Chapter>,
        }

        let output = run_ffprobe(
            &[
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_entries",
                "chapter=id",
            ],
            path,
        )
        .await?;

        let parsed: ChapterProbe = serde_json::from_slice(&output.stdout).map_err(|e| {
            AlchemistError::Analyzer(format!("Failed to parse ffprobe JSON: {}", e))
        })?;

        Ok(u32::try_from(parsed.chapters.len()).unwrap_or(u32::MAX))
    }

    pub async fn probe_video_codec(path: &Path) -> Result<String> {
        let output = run_ffprobe(
            &[
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=codec_name",
                "-of",
                "default=nokey=1:noprint_wrappers=1",
            ],
            path,
        )
        .await?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub async fn probe_output_details(path: &Path) -> Result<OutputProbe> {
        #[derive(Debug, Deserialize)]
        struct ProbeData {
            streams: Vec<ProbeStream>,
            #[serde(default)]
            format: ProbeFormat,
        }

        #[derive(Debug, Deserialize)]
        struct ProbeStream {
            codec_name: Option<String>,
            #[serde(default)]
            tags: ProbeTags,
        }

        #[derive(Debug, Default, Deserialize)]
        struct ProbeFormat {
            #[serde(default)]
            tags: ProbeTags,
        }

        #[derive(Debug, Default, Deserialize)]
        struct ProbeTags {
            encoder: Option<String>,
        }

        let output = run_ffprobe(
            &[
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-print_format",
                "json",
                "-show_entries",
                "stream=codec_name:stream_tags=encoder:format_tags=encoder",
            ],
            path,
        )
        .await?;

        let parsed: ProbeData = serde_json::from_slice(&output.stdout).map_err(|e| {
            AlchemistError::Analyzer(format!("Failed to parse ffprobe JSON: {}", e))
        })?;
        let codec_name = parsed
            .streams
            .first()
            .and_then(|s| s.codec_name.clone())
            .unwrap_or_default();
        Ok(OutputProbe {
            codec_name,
            stream_encoder_tag: parsed.streams.first().and_then(|s| s.tags.encoder.clone()),
            format_encoder_tag: parsed.format.tags.encoder,
        })
    }

    // ... should_transcode adapted below ...

    pub fn should_transcode(
        _path: &Path,
        metadata: &MediaMetadata,
        config: &crate::config::Config,
    ) -> TranscodeDecision {
        // 1. Codec Check (skip if already AV1 + 10-bit)
        if metadata.codec_name == "av1" && metadata.bit_depth == Some(10) {
            return TranscodeDecision::Skip {
                reason: "Already AV1 10-bit".to_string(),
            };
        }

        // 2. Efficiency Rules (BPP)
        let bitrate = metadata.video_bitrate_bps;
        let width = metadata.width as f64;
        let height = metadata.height as f64;
        let fps = metadata.fps;

        let bitrate = match bitrate {
            Some(bitrate) if bitrate > 0 => bitrate as f64,
            _ => {
                return TranscodeDecision::Skip {
                    reason: "Incomplete metadata (bitrate/resolution)".to_string(),
                };
            }
        };

        if width == 0.0 || height == 0.0 || fps <= 0.0 {
            return TranscodeDecision::Skip {
                reason: "Incomplete metadata (bitrate/resolution)".to_string(),
            };
        }

        let bpp = bitrate / (width * height * fps);

        // Normalize BPP based on resolution (4K needs less BPP than 1080p for same quality)
        let res_correction = if width >= 3840.0 {
            0.6 // 4K
        } else if width >= 1920.0 {
            0.8 // 1080p
        } else {
            1.0 // 720p and below
        };
        let normalized_bpp = bpp * res_correction;

        // Heuristic: If BPP is already very low, don't murder it further.
        if normalized_bpp < config.transcode.min_bpp_threshold {
            return TranscodeDecision::Skip {
                reason: format!(
                    "BPP too low ({:.4} normalized < {:.2}), avoiding quality murder",
                    normalized_bpp, config.transcode.min_bpp_threshold
                ),
            };
        }

        // 4. Projected Size Logic
        let size_bytes = metadata.size_bytes;
        let min_size_bytes = config.transcode.min_file_size_mb * 1024 * 1024;
        if size_bytes < min_size_bytes {
            return TranscodeDecision::Skip {
                reason: format!(
                    "File too small ({}MB < {}MB) to justify transcode overhead",
                    size_bytes / 1024 / 1024,
                    config.transcode.min_file_size_mb
                ),
            };
        }

        TranscodeDecision::Transcode {
            reason: format!(
                "Ready for AV1 transcode (Current codec: {}, BPP: {:.4})",
                metadata.codec_name, bpp
            ),
        }
    }

    fn parse_fps(s: &str) -> Option<f64> {
        if s.contains('/') {
            let parts: Vec<&str> = s.split('/').collect();
            if parts.len() == 2 {
                let num: f64 = parts[0].parse().ok()?;
                let den: f64 = parts[1].parse().ok()?;
                if den == 0.0 {
                    return None;
                }
                return Some(num / den);
            }
        }
        s.parse().ok()
    }

    pub fn should_transcode_audio(stream: &Stream) -> bool {
        if stream.codec_type != "audio" {
            return false;
        }

        // Only transcode lossless or exotic heavy codecs.
        // Standard compressed codecs (eac3, ac3, dts) copy
        // fine into MKV regardless of bitrate — eac3 Atmos
        // at 768 kbps is normal and should not be transcoded.
        let heavy_codecs = [
            "truehd",
            "mlp",
            "dts-hd",
            "flac",
            "pcm_s24le",
            "pcm_s16le",
            "pcm_s32le",
            "pcm_f32le",
        ];
        heavy_codecs.contains(&stream.codec_name.to_lowercase().as_str())
    }
}

fn parse_f64(s: &str) -> Option<f64> {
    s.parse().ok()
}

fn parse_u64(s: &str) -> Option<u64> {
    s.parse().ok()
}

fn subtitle_codec_is_burnable(codec_name: &str) -> bool {
    matches!(
        codec_name.to_ascii_lowercase().as_str(),
        "subrip" | "srt" | "ass" | "ssa" | "webvtt" | "text" | "mov_text" | "tx3g"
    )
}

fn infer_bit_depth(stream: &Stream) -> Option<u8> {
    if let Some(ref pix_fmt) = stream.pix_fmt
        && let Some(depth) = bit_depth_from_pix_fmt(pix_fmt)
    {
        return Some(depth);
    }

    stream
        .bits_per_raw_sample
        .as_deref()
        .and_then(|s| s.parse().ok())
}

fn bit_depth_from_pix_fmt(pix_fmt: &str) -> Option<u8> {
    let fmt = pix_fmt.to_ascii_lowercase();
    let depth_candidates = [
        (16u8, ["p16", "p016", "16le", "16be"]),
        (14u8, ["p14", "p014", "14le", "14be"]),
        (12u8, ["p12", "p012", "12le", "12be"]),
        (10u8, ["p10", "p010", "10le", "10be"]),
        (9u8, ["p09", "p9", "9le", "9be"]),
        (8u8, ["p08", "p8", "8le", "8be"]),
    ];

    for (depth, patterns) in depth_candidates.iter() {
        if patterns.iter().any(|pattern| fmt.contains(pattern)) {
            return Some(*depth);
        }
    }

    None
}

fn detect_dynamic_range(
    color_transfer: Option<&str>,
    color_primaries: Option<&str>,
) -> DynamicRange {
    match color_transfer {
        Some("smpte2084") => DynamicRange::Hdr10,
        Some("arib-std-b67") => DynamicRange::Hlg,
        Some(_) => DynamicRange::Sdr,
        None => {
            if matches!(color_primaries, Some("bt2020")) {
                DynamicRange::Unknown
            } else {
                DynamicRange::Sdr
            }
        }
    }
}

// Density labels are factual buckets for later UI/intelligence evidence.
// They are intentionally not transcode/skip policy thresholds.
const HIGH_NORMALIZED_BPP_THRESHOLD: f64 = 0.12;
const LOW_NORMALIZED_BPP_THRESHOLD: f64 = 0.03;
// Container bitrate within 8% of video bitrate usually means little mux overhead.
const REMUX_LIKE_CONTAINER_OVERHEAD_RATIO: f64 = 1.08;

fn build_analyzer_report(
    metadata: &MediaMetadata,
    warnings: &[AnalysisWarning],
    streams: &[Stream],
    video_stream: &Stream,
    fps_from_average_rate: Option<f64>,
    fps_from_frame_count: Option<f64>,
) -> AnalyzerReport {
    let mut labels = Vec::new();
    let estimated_container_bitrate_bps = estimated_container_bitrate_bps(metadata);
    let raw_bpp = bpp(
        metadata.video_bitrate_bps,
        metadata.width,
        metadata.height,
        metadata.fps,
    );
    let normalized_bpp = raw_bpp.map(|value| normalize_bpp(value, metadata.width));
    let report_audio_bitrate_bps = total_audio_bitrate_bps(streams).or(metadata.audio_bitrate_bps);
    let audio_bitrate_share = match (report_audio_bitrate_bps, estimated_container_bitrate_bps) {
        (Some(audio), Some(total)) if total > 0 => {
            Some((audio as f64 / total as f64).clamp(0.0, 1.0))
        }
        _ => None,
    };

    if normalized_bpp.is_some_and(|value| value >= HIGH_NORMALIZED_BPP_THRESHOLD) {
        push_label(&mut labels, AnalyzerLabel::HighBppDensity);
    }
    if normalized_bpp.is_some_and(|value| value <= LOW_NORMALIZED_BPP_THRESHOLD) {
        push_label(&mut labels, AnalyzerLabel::LowBppDensity);
    }
    if remux_like_density(metadata.video_bitrate_bps, estimated_container_bitrate_bps) {
        push_label(&mut labels, AnalyzerLabel::RemuxLikeDensity);
    }
    if metadata.audio_is_heavy
        || streams
            .iter()
            .filter(|stream| stream.codec_type == "audio")
            .any(Analyzer::should_transcode_audio)
    {
        push_label(&mut labels, AnalyzerLabel::HeavyAudio);
    }
    if metadata
        .audio_codec
        .as_deref()
        .is_some_and(audio_codec_is_lossless)
        || streams
            .iter()
            .filter(|stream| stream.codec_type == "audio")
            .any(|stream| audio_codec_is_lossless(&stream.codec_name))
    {
        push_label(&mut labels, AnalyzerLabel::LosslessAudio);
    }

    let image_subtitle_count = metadata
        .subtitle_streams
        .iter()
        .filter(|stream| subtitle_codec_is_image(&stream.codec_name))
        .count() as u32;
    let text_subtitle_count = metadata
        .subtitle_streams
        .iter()
        .filter(|stream| subtitle_codec_is_text(&stream.codec_name))
        .count() as u32;
    if image_subtitle_count > 0 {
        push_label(&mut labels, AnalyzerLabel::ImageSubtitle);
    }
    if metadata
        .subtitle_streams
        .iter()
        .any(|stream| subtitle_codec_is_styled(&stream.codec_name))
    {
        push_label(&mut labels, AnalyzerLabel::StyledSubtitle);
    }

    let hdr_metadata_present = metadata.dynamic_range.is_hdr();
    let has_bt2020_metadata = metadata
        .color_primaries
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("bt2020"));
    let has_missing_color_transfer = metadata.color_transfer.is_none();
    if hdr_metadata_present {
        push_label(&mut labels, AnalyzerLabel::HdrMetadata);
    }
    if has_bt2020_metadata && has_missing_color_transfer {
        push_label(&mut labels, AnalyzerLabel::Bt2020WithoutTransfer);
    }
    if has_dolby_vision_metadata(video_stream) {
        push_label(&mut labels, AnalyzerLabel::DolbyVisionMetadata);
    }
    if video_stream
        .field_order
        .as_deref()
        .is_some_and(field_order_is_interlaced)
    {
        push_label(&mut labels, AnalyzerLabel::InterlacedMetadata);
    }
    if variable_frame_rate_hint(fps_from_average_rate, fps_from_frame_count) {
        push_label(&mut labels, AnalyzerLabel::VariableFrameRateHint);
    }

    for warning in warnings {
        push_label(&mut labels, warning_label(warning));
    }

    AnalyzerReport {
        labels,
        metrics: AnalyzerMetrics {
            raw_bpp,
            normalized_bpp,
            estimated_container_bitrate_bps,
            audio_bitrate_share,
            video_stream_count: Some(
                streams.iter().filter(|s| s.codec_type == "video").count() as u32
            ),
            audio_stream_count: Some(
                streams.iter().filter(|s| s.codec_type == "audio").count() as u32
            ),
            subtitle_stream_count: Some(metadata.subtitle_streams.len() as u32),
            image_subtitle_count: Some(image_subtitle_count),
            text_subtitle_count: Some(text_subtitle_count),
            hdr_metadata_present: Some(hdr_metadata_present),
            has_bt2020_metadata: Some(has_bt2020_metadata),
            has_missing_color_transfer: Some(has_missing_color_transfer),
            fps_from_average_rate,
            fps_from_frame_count,
        },
    }
}

fn push_label(labels: &mut Vec<AnalyzerLabel>, label: AnalyzerLabel) {
    if !labels.contains(&label) {
        labels.push(label);
    }
}

fn estimated_container_bitrate_bps(metadata: &MediaMetadata) -> Option<u64> {
    metadata.container_bitrate_bps.or_else(|| {
        if metadata.duration_secs > 0.0 && metadata.size_bytes > 0 {
            Some(((metadata.size_bytes as f64 * 8.0) / metadata.duration_secs).round() as u64)
        } else {
            None
        }
    })
}

fn selected_metadata_fps(video_stream: &Stream, fps_from_frame_count: Option<f64>) -> f64 {
    video_stream
        .avg_frame_rate
        .as_deref()
        .or(video_stream.r_frame_rate.as_deref())
        .and_then(Analyzer::parse_fps)
        .or(fps_from_frame_count)
        .unwrap_or(0.0)
}

fn fps_from_frame_count(video_stream: &Stream, format_duration: Option<f64>) -> Option<f64> {
    let stream_duration = video_stream.duration.as_deref().and_then(parse_f64);
    let duration = stream_duration.or(format_duration);
    let frames = video_stream.nb_frames.as_deref().and_then(parse_f64);
    match (frames, duration) {
        (Some(frames), Some(duration)) if duration > 0.0 => Some(frames / duration),
        _ => None,
    }
}

fn bpp(video_bitrate_bps: Option<u64>, width: u32, height: u32, fps: f64) -> Option<f64> {
    let bitrate = video_bitrate_bps?;
    if bitrate == 0 || width == 0 || height == 0 || fps <= 0.0 {
        return None;
    }
    Some(bitrate as f64 / (width as f64 * height as f64 * fps))
}

fn total_audio_bitrate_bps(streams: &[Stream]) -> Option<u64> {
    let total = streams
        .iter()
        .filter(|stream| stream.codec_type == "audio")
        .filter_map(|stream| stream.bit_rate.as_deref().and_then(parse_u64))
        .sum::<u64>();
    (total > 0).then_some(total)
}

fn normalize_bpp(raw_bpp: f64, width: u32) -> f64 {
    let correction = if width >= 3840 {
        0.6
    } else if width >= 1920 {
        0.8
    } else {
        1.0
    };
    raw_bpp * correction
}

fn remux_like_density(video_bitrate_bps: Option<u64>, container_bitrate_bps: Option<u64>) -> bool {
    match (video_bitrate_bps, container_bitrate_bps) {
        (Some(video), Some(container)) if video > 0 && container >= video => {
            (container as f64 / video as f64) <= REMUX_LIKE_CONTAINER_OVERHEAD_RATIO
        }
        _ => false,
    }
}

fn audio_codec_is_lossless(codec_name: &str) -> bool {
    matches!(
        codec_name.to_ascii_lowercase().as_str(),
        "truehd" | "mlp" | "flac" | "alac" | "pcm_s24le" | "pcm_s16le" | "pcm_s32le" | "pcm_f32le"
    )
}

fn subtitle_codec_is_image(codec_name: &str) -> bool {
    matches!(
        codec_name.to_ascii_lowercase().as_str(),
        "hdmv_pgs_subtitle" | "pgs" | "dvd_subtitle" | "dvb_subtitle" | "xsub"
    )
}

fn subtitle_codec_is_text(codec_name: &str) -> bool {
    subtitle_codec_is_burnable(codec_name)
}

fn subtitle_codec_is_styled(codec_name: &str) -> bool {
    matches!(codec_name.to_ascii_lowercase().as_str(), "ass" | "ssa")
}

fn has_dolby_vision_metadata(stream: &Stream) -> bool {
    stream.side_data_list.iter().any(|side_data| {
        side_data.side_data_type.as_deref().is_some_and(|value| {
            value.to_ascii_lowercase().contains("dovi")
                || value.to_ascii_lowercase().contains("dolby vision")
        })
    })
}

fn field_order_is_interlaced(field_order: &str) -> bool {
    matches!(
        field_order.to_ascii_lowercase().as_str(),
        "tt" | "bb" | "tb" | "bt" | "interlaced" | "tff" | "bff"
    )
}

fn variable_frame_rate_hint(
    fps_from_average_rate: Option<f64>,
    fps_from_frame_count: Option<f64>,
) -> bool {
    match (fps_from_average_rate, fps_from_frame_count) {
        (Some(avg), Some(counted)) if avg > 0.0 && counted > 0.0 => {
            ((avg - counted).abs() / avg.max(counted)) > 0.01
        }
        _ => false,
    }
}

fn warning_label(warning: &AnalysisWarning) -> AnalyzerLabel {
    match warning {
        AnalysisWarning::MissingVideoBitrate => AnalyzerLabel::MissingVideoBitrate,
        AnalysisWarning::MissingContainerBitrate => AnalyzerLabel::MissingContainerBitrate,
        AnalysisWarning::MissingDuration => AnalyzerLabel::MissingDuration,
        AnalysisWarning::MissingFps => AnalyzerLabel::MissingFps,
        AnalysisWarning::MissingBitDepth => AnalyzerLabel::MissingBitDepth,
        AnalysisWarning::UnrecognizedPixelFormat => AnalyzerLabel::UnrecognizedPixelFormat,
    }
}

fn select_video_stream(streams: &[Stream]) -> Option<&Stream> {
    let mut best: Option<&Stream> = None;
    let mut best_pixels = 0u64;
    let mut best_is_default = false;

    for stream in streams.iter().filter(|s| s.codec_type == "video") {
        let is_default = stream
            .disposition
            .as_ref()
            .and_then(|d| d.default)
            .unwrap_or(0)
            == 1;
        let width = stream.width.or(stream.coded_width).unwrap_or(0) as u64;
        let height = stream.height.or(stream.coded_height).unwrap_or(0) as u64;
        let pixels = width.saturating_mul(height);

        if best.is_none()
            || (is_default && !best_is_default)
            || (is_default == best_is_default && pixels > best_pixels)
        {
            best = Some(stream);
            best_pixels = pixels;
            best_is_default = is_default;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn stream(codec_type: &str, codec_name: &str) -> Stream {
        Stream {
            codec_name: codec_name.to_string(),
            codec_type: codec_type.to_string(),
            pix_fmt: None,
            width: None,
            height: None,
            coded_width: None,
            coded_height: None,
            bit_rate: None,
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            avg_frame_rate: None,
            r_frame_rate: None,
            nb_frames: None,
            duration: None,
            disposition: None,
            tags: None,
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            field_order: None,
            side_data_list: Vec::new(),
        }
    }

    fn metadata() -> MediaMetadata {
        MediaMetadata {
            path: PathBuf::from("/media/test.mkv"),
            duration_secs: 100.0,
            codec_name: "h264".to_string(),
            width: 1920,
            height: 1080,
            bit_depth: Some(8),
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            size_bytes: 100_000_000,
            video_bitrate_bps: Some(8_000_000),
            container_bitrate_bps: Some(8_400_000),
            fps: 24.0,
            container: "matroska".to_string(),
            audio_codec: Some("aac".to_string()),
            audio_bitrate_bps: Some(192_000),
            audio_channels: Some(2),
            audio_is_heavy: false,
            subtitle_streams: Vec::new(),
            audio_streams: Vec::new(),
            dynamic_range: DynamicRange::Sdr,
            chapter_count: 0,
        }
    }

    #[test]
    fn test_parse_fps() {
        assert_eq!(Analyzer::parse_fps("24/1"), Some(24.0));
        assert_eq!(Analyzer::parse_fps("23.976"), Some(23.976));
        assert_eq!(Analyzer::parse_fps("60000/1001"), Some(60000.0 / 1001.0));
        assert_eq!(Analyzer::parse_fps("invalid"), None);
        assert_eq!(Analyzer::parse_fps("24/0"), None);
    }

    #[test]
    fn test_should_transcode_audio() {
        let heavy = Stream {
            codec_name: "truehd".into(),
            codec_type: "audio".into(),
            pix_fmt: None,
            width: None,
            height: None,
            coded_width: None,
            coded_height: None,
            bit_rate: None,
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            avg_frame_rate: None,
            r_frame_rate: None,
            nb_frames: None,
            duration: None,
            disposition: None,
            tags: None,
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            field_order: None,
            side_data_list: Vec::new(),
        };
        assert!(Analyzer::should_transcode_audio(&heavy));

        let standard = Stream {
            codec_name: "ac3".into(),
            codec_type: "audio".into(),
            pix_fmt: None,
            width: None,
            height: None,
            coded_width: None,
            coded_height: None,
            bit_rate: Some("384000".into()),
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            avg_frame_rate: None,
            r_frame_rate: None,
            nb_frames: None,
            duration: None,
            disposition: None,
            tags: None,
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            field_order: None,
            side_data_list: Vec::new(),
        };
        assert!(!Analyzer::should_transcode_audio(&standard));

        let atmos_eac3 = Stream {
            codec_name: "eac3".into(),
            codec_type: "audio".into(),
            pix_fmt: None,
            width: None,
            height: None,
            coded_width: None,
            coded_height: None,
            bit_rate: Some("768000".into()),
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            avg_frame_rate: None,
            r_frame_rate: None,
            nb_frames: None,
            duration: None,
            disposition: None,
            tags: None,
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            field_order: None,
            side_data_list: Vec::new(),
        };
        assert!(!Analyzer::should_transcode_audio(&atmos_eac3));

        let lossless_pcm = Stream {
            codec_name: "pcm_s32le".into(),
            codec_type: "audio".into(),
            pix_fmt: None,
            width: None,
            height: None,
            coded_width: None,
            coded_height: None,
            bit_rate: Some("2000000".into()),
            bits_per_raw_sample: None,
            channel_layout: None,
            channels: None,
            avg_frame_rate: None,
            r_frame_rate: None,
            nb_frames: None,
            duration: None,
            disposition: None,
            tags: None,
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            field_order: None,
            side_data_list: Vec::new(),
        };
        assert!(Analyzer::should_transcode_audio(&lossless_pcm));
    }

    #[test]
    fn analyzer_report_labels_high_bpp_and_bitrate_fallback() {
        let mut metadata = metadata();
        metadata.container_bitrate_bps = None;
        metadata.video_bitrate_bps = Some(20_000_000);
        let streams = vec![stream("video", "h264")];
        let report = build_analyzer_report(
            &metadata,
            &[],
            &streams,
            &streams[0],
            Some(24.0),
            Some(24.0),
        );

        assert!(report.labels.contains(&AnalyzerLabel::HighBppDensity));
        assert_eq!(
            report.metrics.estimated_container_bitrate_bps,
            Some(8_000_000)
        );
        assert!(report.metrics.normalized_bpp.is_some());
    }

    #[test]
    fn analyzer_report_labels_low_bpp_and_remux_like_density() {
        let mut metadata = metadata();
        metadata.video_bitrate_bps = Some(1_000_000);
        metadata.container_bitrate_bps = Some(1_050_000);
        let streams = vec![stream("video", "h264")];
        let report = build_analyzer_report(
            &metadata,
            &[],
            &streams,
            &streams[0],
            Some(24.0),
            Some(24.0),
        );

        assert!(report.labels.contains(&AnalyzerLabel::LowBppDensity));
        assert!(report.labels.contains(&AnalyzerLabel::RemuxLikeDensity));
    }

    #[test]
    fn analyzer_report_omits_density_labels_without_video_bitrate() {
        let mut metadata = metadata();
        metadata.video_bitrate_bps = None;
        metadata.container_bitrate_bps = None;
        let streams = vec![stream("video", "h264")];
        let report = build_analyzer_report(
            &metadata,
            &[],
            &streams,
            &streams[0],
            Some(24.0),
            Some(24.0),
        );

        assert_eq!(
            report.metrics.estimated_container_bitrate_bps,
            Some(8_000_000)
        );
        assert!(report.metrics.raw_bpp.is_none());
        assert!(!report.labels.contains(&AnalyzerLabel::HighBppDensity));
        assert!(!report.labels.contains(&AnalyzerLabel::LowBppDensity));
    }

    #[test]
    fn metadata_fps_preserves_legacy_rate_selection() {
        let mut video = stream("video", "h264");
        video.avg_frame_rate = Some("0/0".to_string());
        video.r_frame_rate = Some("24/1".to_string());
        video.nb_frames = Some("240".to_string());
        video.duration = Some("10".to_string());

        assert_eq!(fps_from_frame_count(&video, None), Some(24.0));
        assert_eq!(selected_metadata_fps(&video, Some(24.0)), 24.0);
        assert_eq!(selected_metadata_fps(&video, None), 0.0);
    }

    #[test]
    fn analyzer_report_labels_audio_and_subtitle_facts() {
        let mut metadata = metadata();
        metadata.audio_codec = Some("flac".to_string());
        metadata.audio_is_heavy = true;
        metadata.subtitle_streams = vec![
            SubtitleStreamMetadata {
                stream_index: 2,
                codec_name: "hdmv_pgs_subtitle".to_string(),
                language: Some("eng".to_string()),
                title: None,
                default: false,
                forced: false,
                burnable: false,
            },
            SubtitleStreamMetadata {
                stream_index: 3,
                codec_name: "ass".to_string(),
                language: Some("eng".to_string()),
                title: Some("Signs".to_string()),
                default: false,
                forced: true,
                burnable: true,
            },
        ];
        let streams = vec![stream("video", "h264")];
        let report = build_analyzer_report(
            &metadata,
            &[],
            &streams,
            &streams[0],
            Some(24.0),
            Some(24.0),
        );

        assert!(report.labels.contains(&AnalyzerLabel::HeavyAudio));
        assert!(report.labels.contains(&AnalyzerLabel::LosslessAudio));
        assert!(report.labels.contains(&AnalyzerLabel::ImageSubtitle));
        assert!(report.labels.contains(&AnalyzerLabel::StyledSubtitle));
        assert_eq!(report.metrics.image_subtitle_count, Some(1));
        assert_eq!(report.metrics.text_subtitle_count, Some(1));
    }

    #[test]
    fn analyzer_report_labels_hdr_structure_and_warning_facts() {
        let mut metadata = metadata();
        metadata.dynamic_range = DynamicRange::Hdr10;
        metadata.color_primaries = Some("bt2020".to_string());
        metadata.color_transfer = None;
        let mut video = stream("video", "hevc");
        video.field_order = Some("tt".to_string());
        video.side_data_list = vec![SideData {
            side_data_type: Some("DOVI configuration record".to_string()),
        }];
        let warnings = vec![
            AnalysisWarning::MissingVideoBitrate,
            AnalysisWarning::MissingContainerBitrate,
            AnalysisWarning::MissingDuration,
            AnalysisWarning::MissingFps,
            AnalysisWarning::MissingBitDepth,
            AnalysisWarning::UnrecognizedPixelFormat,
        ];
        let streams = vec![video];
        let report = build_analyzer_report(
            &metadata,
            &warnings,
            &streams,
            &streams[0],
            Some(24.0),
            Some(23.0),
        );

        assert!(report.labels.contains(&AnalyzerLabel::HdrMetadata));
        assert!(report.labels.contains(&AnalyzerLabel::DolbyVisionMetadata));
        assert!(report.labels.contains(&AnalyzerLabel::InterlacedMetadata));
        assert!(
            report
                .labels
                .contains(&AnalyzerLabel::Bt2020WithoutTransfer)
        );
        assert!(
            report
                .labels
                .contains(&AnalyzerLabel::VariableFrameRateHint)
        );
        assert!(report.labels.contains(&AnalyzerLabel::MissingVideoBitrate));
        assert!(
            report
                .labels
                .contains(&AnalyzerLabel::MissingContainerBitrate)
        );
        assert!(report.labels.contains(&AnalyzerLabel::MissingDuration));
        assert!(report.labels.contains(&AnalyzerLabel::MissingFps));
        assert!(report.labels.contains(&AnalyzerLabel::MissingBitDepth));
        assert!(
            report
                .labels
                .contains(&AnalyzerLabel::UnrecognizedPixelFormat)
        );
    }

    #[test]
    fn analyzer_report_metrics_count_streams_and_audio_share() {
        let mut metadata = metadata();
        metadata.container_bitrate_bps = Some(10_000_000);
        metadata.audio_bitrate_bps = Some(192_000);
        metadata.subtitle_streams = vec![SubtitleStreamMetadata {
            stream_index: 2,
            codec_name: "subrip".to_string(),
            language: Some("eng".to_string()),
            title: None,
            default: true,
            forced: false,
            burnable: true,
        }];
        let mut first_audio = stream("audio", "aac");
        first_audio.bit_rate = Some("2000000".to_string());
        let mut second_audio = stream("audio", "ac3");
        second_audio.bit_rate = Some("1000000".to_string());
        let streams = vec![
            stream("video", "h264"),
            first_audio,
            second_audio,
            stream("subtitle", "subrip"),
        ];
        let report = build_analyzer_report(
            &metadata,
            &[],
            &streams,
            &streams[0],
            Some(24.0),
            Some(24.0),
        );

        assert_eq!(report.metrics.video_stream_count, Some(1));
        assert_eq!(report.metrics.audio_stream_count, Some(2));
        assert_eq!(report.metrics.subtitle_stream_count, Some(1));
        assert_eq!(report.metrics.audio_bitrate_share, Some(0.3));
    }

    #[test]
    fn analyzer_report_labels_secondary_heavy_and_lossless_audio() {
        let mut metadata = metadata();
        metadata.audio_codec = Some("aac".to_string());
        metadata.audio_is_heavy = false;
        let mut lossless_audio = stream("audio", "truehd");
        lossless_audio.channel_layout = Some("7.1".to_string());
        let streams = vec![
            stream("video", "h264"),
            stream("audio", "aac"),
            lossless_audio,
        ];
        let report = build_analyzer_report(
            &metadata,
            &[],
            &streams,
            &streams[0],
            Some(24.0),
            Some(24.0),
        );

        assert!(report.labels.contains(&AnalyzerLabel::HeavyAudio));
        assert!(report.labels.contains(&AnalyzerLabel::LosslessAudio));
    }

    #[test]
    fn legacy_media_analysis_json_defaults_report() {
        let json = r#"{
            "metadata": {
                "path": "/media/test.mkv",
                "duration_secs": 60.0,
                "codec_name": "h264",
                "width": 1920,
                "height": 1080,
                "bit_depth": 8,
                "color_primaries": null,
                "color_transfer": null,
                "color_space": null,
                "color_range": null,
                "size_bytes": 1000000,
                "video_bitrate_bps": 4000000,
                "container_bitrate_bps": 4200000,
                "fps": 24.0,
                "container": "matroska",
                "audio_codec": null,
                "audio_bitrate_bps": null,
                "audio_channels": null,
                "audio_is_heavy": false,
                "subtitle_streams": [],
                "audio_streams": [],
                "dynamic_range": "sdr"
            },
            "warnings": [],
            "confidence": "high"
        }"#;

        let analysis: MediaAnalysis = match serde_json::from_str(json) {
            Ok(analysis) => analysis,
            Err(err) => panic!("legacy analysis json failed to decode: {err}"),
        };
        assert!(analysis.analysis_report.labels.is_empty());
        assert_eq!(analysis.analysis_report.metrics, AnalyzerMetrics::default());
        assert_eq!(analysis.metadata.chapter_count, 0);
    }

    #[test]
    fn ffprobe_metadata_counts_chapters_when_present() {
        let json = r#"{
            "streams": [],
            "format": {
                "format_name": "matroska",
                "format_long_name": "Matroska",
                "duration": "60.0",
                "size": "1000",
                "bit_rate": "1000"
            },
            "chapters": [{ "id": 0 }, { "id": 1 }]
        }"#;

        let metadata: FfprobeMetadata = match serde_json::from_str(json) {
            Ok(metadata) => metadata,
            Err(err) => panic!("ffprobe metadata json failed to decode: {err}"),
        };
        assert_eq!(metadata.chapters.len(), 2);
    }

    #[test]
    fn partial_analysis_report_json_defaults_nested_fields() {
        let report: AnalyzerReport = match serde_json::from_str(r#"{"labels":["heavy_audio"]}"#) {
            Ok(report) => report,
            Err(err) => panic!("partial analyzer report json failed to decode: {err}"),
        };
        assert_eq!(report.labels, vec![AnalyzerLabel::HeavyAudio]);
        assert_eq!(report.metrics, AnalyzerMetrics::default());

        let serialized = match serde_json::to_string(&AnalyzerReport::default()) {
            Ok(value) => value,
            Err(err) => panic!("default analyzer report failed to encode: {err}"),
        };
        assert_eq!(serialized, "{}");
    }
}
