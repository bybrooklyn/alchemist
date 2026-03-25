use crate::error::Result;
use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::process::{Command, ExitStatus, Output};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Cpu,
}

impl std::fmt::Display for Vendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Vendor::Nvidia => write!(f, "NVIDIA (NVENC)"),
            Vendor::Amd => write!(f, "AMD (VAAPI/AMF)"),
            Vendor::Intel => write!(f, "Intel (VAAPI/QSV)"),
            Vendor::Apple => write!(f, "Apple (VideoToolbox)"),
            Vendor::Cpu => write!(f, "CPU (Software Encoding)"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HardwareBackend {
    Nvenc,
    Amf,
    Qsv,
    Vaapi,
    Videotoolbox,
}

impl HardwareBackend {
    fn as_str(self) -> &'static str {
        match self {
            Self::Nvenc => "nvenc",
            Self::Amf => "amf",
            Self::Qsv => "qsv",
            Self::Vaapi => "vaapi",
            Self::Videotoolbox => "videotoolbox",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Nvenc => "NVENC",
            Self::Amf => "AMF",
            Self::Qsv => "QSV",
            Self::Vaapi => "VAAPI",
            Self::Videotoolbox => "VideoToolbox",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendCapability {
    pub kind: HardwareBackend,
    pub codec: String,
    pub encoder: String,
    pub device_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HardwareProbeLog {
    pub entries: Vec<HardwareProbeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProbeEntry {
    pub encoder: String,
    pub backend: String,
    pub device_path: Option<String>,
    pub success: bool,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub vendor: Vendor,
    pub device_path: Option<String>,
    pub supported_codecs: Vec<String>,
    #[serde(default)]
    pub backends: Vec<BackendCapability>,
    #[serde(default)]
    pub detection_notes: Vec<String>,
}

impl HardwareInfo {
    fn new(vendor: Vendor, device_path: Option<String>, backends: Vec<BackendCapability>) -> Self {
        let supported_codecs = if backends.is_empty() && vendor == Vendor::Cpu {
            vec!["av1".to_string(), "hevc".to_string(), "h264".to_string()]
        } else {
            let mut codecs = BTreeSet::new();
            for backend in &backends {
                codecs.insert(backend.codec.clone());
            }
            codecs.into_iter().collect()
        };

        Self {
            vendor,
            device_path,
            supported_codecs,
            backends,
            detection_notes: Vec::new(),
        }
    }

    pub fn supports_codec(&self, codec: &str) -> bool {
        self.supported_codecs.iter().any(|value| value == codec)
    }

    fn with_detection_notes(mut self, detection_notes: Vec<String>) -> Self {
        self.detection_notes = detection_notes;
        self
    }
}

#[derive(Clone, Default)]
pub struct HardwareState {
    inner: Arc<RwLock<Option<HardwareInfo>>>,
}

impl HardwareState {
    pub fn new(initial: Option<HardwareInfo>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
        }
    }

    pub async fn snapshot(&self) -> Option<HardwareInfo> {
        self.inner.read().await.clone()
    }

    pub async fn replace(&self, next: Option<HardwareInfo>) {
        *self.inner.write().await = next;
    }
}

pub trait CommandRunner {
    fn output(&self, program: &str, args: &[String]) -> std::io::Result<Output>;
    fn status(&self, program: &str, args: &[String]) -> std::io::Result<ExitStatus>;
}

pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn output(&self, program: &str, args: &[String]) -> std::io::Result<Output> {
        Command::new(program).args(args).output()
    }

    fn status(&self, program: &str, args: &[String]) -> std::io::Result<ExitStatus> {
        Command::new(program).args(args).status()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BackendProbeSpec {
    kind: HardwareBackend,
    codec: String,
    encoder: String,
    device_path: Option<String>,
}

fn probe_args_for_backend(
    backend: HardwareBackend,
    encoder: &str,
    device_path: Option<&str>,
) -> Vec<String> {
    let mut args = vec!["-v".to_string(), "quiet".to_string()];

    match backend {
        HardwareBackend::Qsv => {
            if let Some(device_path) = device_path {
                args.push("-init_hw_device".to_string());
                args.push(format!("qsv=qsv:{device_path}"));
                args.push("-filter_hw_device".to_string());
                args.push("qsv".to_string());
            }
        }
        HardwareBackend::Vaapi => {
            if let Some(device_path) = device_path {
                args.push("-vaapi_device".to_string());
                args.push(device_path.to_string());
            }
        }
        HardwareBackend::Nvenc | HardwareBackend::Amf => {}
        HardwareBackend::Videotoolbox => {}
    }

    args.extend([
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        "color=c=black:s=64x64:d=0.1".to_string(),
    ]);

    if backend == HardwareBackend::Vaapi {
        args.push("-vf".to_string());
        args.push("format=nv12,hwupload".to_string());
    }

    if backend == HardwareBackend::Videotoolbox {
        args.push("-vf".to_string());
        args.push("format=yuv420p".to_string());
    }

    args.extend(["-c:v".to_string(), encoder.to_string()]);

    if backend == HardwareBackend::Videotoolbox {
        args.push("-allow_sw".to_string());
        args.push("1".to_string());
    }

    args.extend([
        "-frames:v".to_string(),
        "1".to_string(),
        "-f".to_string(),
        "null".to_string(),
        "-y".to_string(),
        "-".to_string(),
    ]);

    args
}

#[cfg(test)]
fn probe_backend_encoder_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    backend: HardwareBackend,
    encoder: &str,
    device_path: Option<&str>,
) -> bool {
    let (success, _) =
        probe_backend_encoder_verbose_with_runner(runner, backend, encoder, device_path);
    success
}

pub fn probe_backend_encoder_verbose(
    backend: HardwareBackend,
    encoder: &str,
    device_path: Option<&str>,
) -> (bool, String) {
    probe_backend_encoder_verbose_with_runner(&SystemCommandRunner, backend, encoder, device_path)
}

fn probe_backend_encoder_verbose_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    backend: HardwareBackend,
    encoder: &str,
    device_path: Option<&str>,
) -> (bool, String) {
    let args = probe_args_for_backend(backend, encoder, device_path);
    match runner.output("ffmpeg", &args) {
        Ok(output) => (
            output.status.success(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ),
        Err(e) => (false, format!("Failed to run ffmpeg: {}", e)),
    }
}

#[cfg(test)]
fn push_backend_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    backends: &mut Vec<BackendCapability>,
    kind: HardwareBackend,
    codec: &str,
    encoder: &str,
    device_path: Option<&str>,
) {
    if probe_backend_encoder_with_runner(runner, kind, encoder, device_path) {
        backends.push(BackendCapability {
            kind,
            codec: codec.to_string(),
            encoder: encoder.to_string(),
            device_path: device_path.map(|value| value.to_string()),
        });
    }
}

fn parse_preferred_vendor(value: &str) -> Option<Vendor> {
    match value.trim().to_ascii_lowercase().as_str() {
        "nvidia" => Some(Vendor::Nvidia),
        "amd" => Some(Vendor::Amd),
        "intel" => Some(Vendor::Intel),
        "apple" => Some(Vendor::Apple),
        "cpu" => Some(Vendor::Cpu),
        _ => None,
    }
}

#[cfg(test)]
fn collect_backend_capabilities<R: CommandRunner + ?Sized>(
    runner: &R,
    specs: Vec<BackendProbeSpec>,
) -> Vec<BackendCapability> {
    let mut backends = Vec::new();
    for spec in specs {
        push_backend_with_runner(
            runner,
            &mut backends,
            spec.kind,
            &spec.codec,
            &spec.encoder,
            spec.device_path.as_deref(),
        );
    }
    dedupe_backend_capabilities_by_codec(backends)
}

fn collect_backend_capabilities_verbose<R: CommandRunner + ?Sized>(
    runner: &R,
    specs: Vec<BackendProbeSpec>,
    probe_log: &mut HardwareProbeLog,
) -> Vec<BackendCapability> {
    let mut backends = Vec::new();

    for spec in specs {
        let (success, stderr) = probe_backend_encoder_verbose_with_runner(
            runner,
            spec.kind,
            &spec.encoder,
            spec.device_path.as_deref(),
        );
        let stderr = stderr.trim().to_string();
        let stderr_value = (!stderr.is_empty()).then_some(stderr.clone());

        probe_log.entries.push(HardwareProbeEntry {
            encoder: spec.encoder.clone(),
            backend: spec.kind.as_str().to_string(),
            device_path: spec.device_path.clone(),
            success,
            stderr: stderr_value.clone(),
        });

        if success {
            backends.push(BackendCapability {
                kind: spec.kind,
                codec: spec.codec,
                encoder: spec.encoder,
                device_path: spec.device_path,
            });
        } else {
            debug!(
                "{} probe failed for {}: {}",
                spec.kind.display_name(),
                spec.encoder,
                stderr.lines().next().unwrap_or("unknown error"),
            );
        }
    }

    dedupe_backend_capabilities_by_codec(backends)
}

fn dedupe_backend_capabilities_by_codec(
    backends: Vec<BackendCapability>,
) -> Vec<BackendCapability> {
    let mut seen = HashSet::new();
    backends
        .into_iter()
        .filter(|backend| seen.insert(backend.codec.clone()))
        .collect()
}

fn backend_probe_specs_for_vendor(
    vendor: Vendor,
    device_path: Option<&str>,
) -> Vec<BackendProbeSpec> {
    let device_path = device_path.map(|value| value.to_string());
    match vendor {
        Vendor::Apple => vec![
            BackendProbeSpec {
                kind: HardwareBackend::Videotoolbox,
                codec: "av1".to_string(),
                encoder: "av1_videotoolbox".to_string(),
                device_path: None,
            },
            BackendProbeSpec {
                kind: HardwareBackend::Videotoolbox,
                codec: "hevc".to_string(),
                encoder: "hevc_videotoolbox".to_string(),
                device_path: None,
            },
            BackendProbeSpec {
                kind: HardwareBackend::Videotoolbox,
                codec: "h264".to_string(),
                encoder: "h264_videotoolbox".to_string(),
                device_path: None,
            },
        ],
        Vendor::Nvidia => vec![
            BackendProbeSpec {
                kind: HardwareBackend::Nvenc,
                codec: "av1".to_string(),
                encoder: "av1_nvenc".to_string(),
                device_path: None,
            },
            BackendProbeSpec {
                kind: HardwareBackend::Nvenc,
                codec: "hevc".to_string(),
                encoder: "hevc_nvenc".to_string(),
                device_path: None,
            },
            BackendProbeSpec {
                kind: HardwareBackend::Nvenc,
                codec: "h264".to_string(),
                encoder: "h264_nvenc".to_string(),
                device_path: None,
            },
        ],
        Vendor::Intel => vec![
            BackendProbeSpec {
                kind: HardwareBackend::Vaapi,
                codec: "av1".to_string(),
                encoder: "av1_vaapi".to_string(),
                device_path: device_path.clone(),
            },
            BackendProbeSpec {
                kind: HardwareBackend::Vaapi,
                codec: "hevc".to_string(),
                encoder: "hevc_vaapi".to_string(),
                device_path: device_path.clone(),
            },
            BackendProbeSpec {
                kind: HardwareBackend::Vaapi,
                codec: "h264".to_string(),
                encoder: "h264_vaapi".to_string(),
                device_path: device_path.clone(),
            },
            BackendProbeSpec {
                kind: HardwareBackend::Qsv,
                codec: "av1".to_string(),
                encoder: "av1_qsv".to_string(),
                device_path: device_path.clone(),
            },
            BackendProbeSpec {
                kind: HardwareBackend::Qsv,
                codec: "hevc".to_string(),
                encoder: "hevc_qsv".to_string(),
                device_path: device_path.clone(),
            },
            BackendProbeSpec {
                kind: HardwareBackend::Qsv,
                codec: "h264".to_string(),
                encoder: "h264_qsv".to_string(),
                device_path,
            },
        ],
        Vendor::Amd if cfg!(target_os = "windows") => vec![
            BackendProbeSpec {
                kind: HardwareBackend::Amf,
                codec: "av1".to_string(),
                encoder: "av1_amf".to_string(),
                device_path: None,
            },
            BackendProbeSpec {
                kind: HardwareBackend::Amf,
                codec: "hevc".to_string(),
                encoder: "hevc_amf".to_string(),
                device_path: None,
            },
            BackendProbeSpec {
                kind: HardwareBackend::Amf,
                codec: "h264".to_string(),
                encoder: "h264_amf".to_string(),
                device_path: None,
            },
        ],
        Vendor::Amd => vec![
            BackendProbeSpec {
                kind: HardwareBackend::Vaapi,
                codec: "av1".to_string(),
                encoder: "av1_vaapi".to_string(),
                device_path: device_path.clone(),
            },
            BackendProbeSpec {
                kind: HardwareBackend::Vaapi,
                codec: "hevc".to_string(),
                encoder: "hevc_vaapi".to_string(),
                device_path: device_path.clone(),
            },
            BackendProbeSpec {
                kind: HardwareBackend::Vaapi,
                codec: "h264".to_string(),
                encoder: "h264_vaapi".to_string(),
                device_path,
            },
        ],
        Vendor::Cpu => Vec::new(),
    }
}

fn vendor_from_explicit_device_path(device_path: &Path) -> Option<Vendor> {
    if !cfg!(target_os = "linux") {
        return None;
    }

    let render_node = device_path.file_name()?.to_str()?;
    let vendor_path = Path::new("/sys/class/drm")
        .join(render_node)
        .join("device/vendor");
    let vendor_id = std::fs::read_to_string(vendor_path).ok()?;
    match vendor_id.trim().to_ascii_lowercase().as_str() {
        "0x8086" => Some(Vendor::Intel),
        "0x1002" => Some(Vendor::Amd),
        "0x10de" => Some(Vendor::Nvidia),
        _ => None,
    }
}

fn detect_backend_at_device_path_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    vendor: Vendor,
    device_path: &str,
    probe_log: &mut HardwareProbeLog,
) -> Option<HardwareInfo> {
    let backends = collect_backend_capabilities_verbose(
        runner,
        backend_probe_specs_for_vendor(vendor, Some(device_path)),
        probe_log,
    );

    if backends.is_empty() {
        return None;
    }

    Some(HardwareInfo::new(
        vendor,
        Some(device_path.to_string()),
        backends,
    ))
}

#[allow(dead_code)]
fn detect_intel_at_device_path_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    device_path: &str,
) -> Option<HardwareInfo> {
    let mut probe_log = HardwareProbeLog::default();
    detect_backend_at_device_path_with_runner(runner, Vendor::Intel, device_path, &mut probe_log)
}

#[allow(dead_code)]
fn detect_amd_at_device_path_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    device_path: &str,
) -> Option<HardwareInfo> {
    let mut probe_log = HardwareProbeLog::default();
    detect_backend_at_device_path_with_runner(runner, Vendor::Amd, device_path, &mut probe_log)
}

#[allow(dead_code)]
fn detect_explicit_device_path_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    device_path: &str,
    preferred_vendor: Option<Vendor>,
) -> Option<HardwareInfo> {
    let mut probe_log = HardwareProbeLog::default();
    detect_explicit_device_path_with_runner_and_log(
        runner,
        device_path,
        preferred_vendor,
        &mut probe_log,
    )
}

fn detect_explicit_device_path_with_runner_and_log<R: CommandRunner + ?Sized>(
    runner: &R,
    device_path: &str,
    preferred_vendor: Option<Vendor>,
    probe_log: &mut HardwareProbeLog,
) -> Option<HardwareInfo> {
    if !cfg!(target_os = "linux") {
        return None;
    }

    let path = Path::new(device_path);
    if !path.exists() {
        return None;
    }

    let vendor = preferred_vendor.or_else(|| vendor_from_explicit_device_path(path));
    match vendor {
        Some(Vendor::Intel) => {
            detect_backend_at_device_path_with_runner(runner, Vendor::Intel, device_path, probe_log)
        }
        Some(Vendor::Amd) => {
            detect_backend_at_device_path_with_runner(runner, Vendor::Amd, device_path, probe_log)
        }
        Some(Vendor::Cpu) | Some(Vendor::Apple) | Some(Vendor::Nvidia) => None,
        None => {
            detect_backend_at_device_path_with_runner(runner, Vendor::Intel, device_path, probe_log)
                .or_else(|| {
                    detect_backend_at_device_path_with_runner(
                        runner,
                        Vendor::Amd,
                        device_path,
                        probe_log,
                    )
                })
        }
    }
}

#[allow(dead_code)]
fn try_detect_apple_with_runner<R: CommandRunner + ?Sized>(runner: &R) -> Option<HardwareInfo> {
    let mut probe_log = HardwareProbeLog::default();
    try_detect_apple_with_runner_and_log(runner, &mut probe_log)
}

fn try_detect_apple_with_runner_and_log<R: CommandRunner + ?Sized>(
    runner: &R,
    probe_log: &mut HardwareProbeLog,
) -> Option<HardwareInfo> {
    if !cfg!(target_os = "macos") {
        return None;
    }

    let backends = collect_backend_capabilities_verbose(
        runner,
        backend_probe_specs_for_vendor(Vendor::Apple, None),
        probe_log,
    );

    if backends.is_empty() {
        return None;
    }

    Some(HardwareInfo::new(Vendor::Apple, None, backends))
}

#[allow(dead_code)]
fn try_detect_nvidia_with_runner<R: CommandRunner + ?Sized>(runner: &R) -> Option<HardwareInfo> {
    let mut probe_log = HardwareProbeLog::default();
    try_detect_nvidia_with_runner_and_log(runner, &mut probe_log)
}

fn try_detect_nvidia_with_runner_and_log<R: CommandRunner + ?Sized>(
    runner: &R,
    probe_log: &mut HardwareProbeLog,
) -> Option<HardwareInfo> {
    let nvidia_likely = if cfg!(target_os = "windows") {
        true
    } else {
        Path::new("/dev/nvidiactl").exists()
    };

    if !nvidia_likely {
        return None;
    }

    let output = runner.output("nvidia-smi", &[]).ok()?;
    if !output.status.success() {
        return None;
    }

    let backends = collect_backend_capabilities_verbose(
        runner,
        backend_probe_specs_for_vendor(Vendor::Nvidia, None),
        probe_log,
    );

    if backends.is_empty() {
        return None;
    }

    Some(HardwareInfo::new(Vendor::Nvidia, None, backends))
}

#[allow(dead_code)]
fn try_detect_intel_with_runner<R: CommandRunner + ?Sized>(runner: &R) -> Option<HardwareInfo> {
    let mut probe_log = HardwareProbeLog::default();
    try_detect_intel_with_runner_and_log(runner, &mut probe_log)
}

fn intel_render_device_path() -> Option<String> {
    if Path::new("/dev/dri/renderD129").exists() {
        return Some("/dev/dri/renderD129".to_string());
    }

    if Path::new("/dev/dri/renderD128").exists() {
        let vendor_id = std::fs::read_to_string("/sys/class/drm/renderD128/device/vendor")
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if vendor_id.contains("0x8086") {
            return Some("/dev/dri/renderD128".to_string());
        }
    }

    None
}

fn try_detect_intel_with_runner_and_log<R: CommandRunner + ?Sized>(
    runner: &R,
    probe_log: &mut HardwareProbeLog,
) -> Option<HardwareInfo> {
    let device_path = intel_render_device_path()?;
    detect_backend_at_device_path_with_runner(runner, Vendor::Intel, &device_path, probe_log)
}

fn amd_render_device_path() -> Option<String> {
    if cfg!(target_os = "windows") {
        None
    } else if Path::new("/dev/dri/renderD128").exists() {
        let vendor_id = std::fs::read_to_string("/sys/class/drm/renderD128/device/vendor")
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if vendor_id.contains("0x1002") {
            Some("/dev/dri/renderD128".to_string())
        } else {
            None
        }
    } else {
        None
    }
}

#[allow(dead_code)]
fn try_detect_amd_with_runner<R: CommandRunner + ?Sized>(runner: &R) -> Option<HardwareInfo> {
    let mut probe_log = HardwareProbeLog::default();
    try_detect_amd_with_runner_and_log(runner, &mut probe_log)
}

fn try_detect_amd_with_runner_and_log<R: CommandRunner + ?Sized>(
    runner: &R,
    probe_log: &mut HardwareProbeLog,
) -> Option<HardwareInfo> {
    let device_path = amd_render_device_path();

    if cfg!(target_os = "windows") {
        let backends = collect_backend_capabilities_verbose(
            runner,
            backend_probe_specs_for_vendor(Vendor::Amd, None),
            probe_log,
        );
        if backends.is_empty() {
            return None;
        }
        return Some(HardwareInfo::new(Vendor::Amd, None, backends));
    }

    let device_path = device_path?;
    detect_backend_at_device_path_with_runner(runner, Vendor::Amd, &device_path, probe_log)
}

fn detect_preferred_hardware_with_runner_and_log<R: CommandRunner + ?Sized>(
    runner: &R,
    preferred_vendor: Vendor,
    probe_log: &mut HardwareProbeLog,
) -> Option<HardwareInfo> {
    match preferred_vendor {
        Vendor::Nvidia => try_detect_nvidia_with_runner_and_log(runner, probe_log),
        Vendor::Amd => try_detect_amd_with_runner_and_log(runner, probe_log),
        Vendor::Intel => try_detect_intel_with_runner_and_log(runner, probe_log),
        Vendor::Apple => try_detect_apple_with_runner_and_log(runner, probe_log),
        Vendor::Cpu => Some(HardwareInfo::new(Vendor::Cpu, None, Vec::new())),
    }
}

fn append_detection_note(notes: &mut Vec<String>, note: impl Into<String>) {
    let note = note.into();
    if !notes.iter().any(|existing| existing == &note) {
        notes.push(note);
    }
}

fn first_failed_probe_line(
    probe_log: &HardwareProbeLog,
    backend: HardwareBackend,
    device_path: Option<&str>,
) -> Option<String> {
    probe_log
        .entries
        .iter()
        .find(|entry| {
            !entry.success
                && entry.backend == backend.as_str()
                && entry.device_path.as_deref() == device_path
        })
        .and_then(|entry| entry.stderr.as_deref())
        .and_then(|stderr| stderr.lines().next())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}

fn note_for_failed_vendor(vendor: Vendor, probe_log: &HardwareProbeLog) -> Option<String> {
    match vendor {
        Vendor::Apple if cfg!(target_os = "macos") => Some(
            "VideoToolbox probe failed — check that FFmpeg was compiled with --enable-videotoolbox"
                .to_string(),
        ),
        Vendor::Nvidia => {
            if !cfg!(target_os = "windows") && !Path::new("/dev/nvidiactl").exists() {
                Some("NVIDIA probe skipped — no /dev/nvidiactl found".to_string())
            } else if let Some(stderr) =
                first_failed_probe_line(probe_log, HardwareBackend::Nvenc, None)
            {
                Some(format!("NVIDIA probe failed — {}", stderr))
            } else {
                Some("NVIDIA probe failed — nvidia-smi not available or returned error".to_string())
            }
        }
        Vendor::Intel => {
            if let Some(device_path) = intel_render_device_path() {
                let mut note = format!(
                    "Intel VAAPI/QSV probe failed at {} — check that the i915/xe driver is loaded and the device is accessible",
                    device_path
                );
                if let Some(stderr) =
                    first_failed_probe_line(probe_log, HardwareBackend::Vaapi, Some(&device_path))
                        .or_else(|| {
                            first_failed_probe_line(
                                probe_log,
                                HardwareBackend::Qsv,
                                Some(&device_path),
                            )
                        })
                {
                    note.push_str(&format!(". FFmpeg said: {}", stderr));
                }
                Some(note)
            } else {
                Some(
                    "Intel probe failed — no render node found at /dev/dri/renderD128 or renderD129"
                        .to_string(),
                )
            }
        }
        Vendor::Amd if cfg!(target_os = "windows") => {
            if let Some(stderr) = first_failed_probe_line(probe_log, HardwareBackend::Amf, None) {
                Some(format!(
                    "AMD AMF probe failed — check that FFmpeg was compiled with AMF support. FFmpeg said: {}",
                    stderr
                ))
            } else {
                Some(
                    "AMD probe failed — AMF was unavailable. Check that FFmpeg was compiled with AMF support"
                        .to_string(),
                )
            }
        }
        Vendor::Amd => {
            if let Some(device_path) = amd_render_device_path() {
                let mut note = format!(
                    "AMD VAAPI probe failed at {} — check that the amdgpu driver is loaded and the device is accessible",
                    device_path
                );
                if let Some(stderr) =
                    first_failed_probe_line(probe_log, HardwareBackend::Vaapi, Some(&device_path))
                {
                    note.push_str(&format!(". FFmpeg said: {}", stderr));
                }
                Some(note)
            } else {
                Some("AMD probe failed — no render node found at /dev/dri/renderD128".to_string())
            }
        }
        Vendor::Cpu => None,
        Vendor::Apple => None,
    }
}

fn detect_hardware_with_preference_and_runner_inner<R: CommandRunner + ?Sized>(
    runner: &R,
    allow_cpu_fallback: bool,
    preferred_vendor: Option<String>,
) -> Result<(HardwareInfo, HardwareProbeLog)> {
    info!("=== Hardware Detection Starting ===");
    info!("OS: {}", std::env::consts::OS);
    info!("Architecture: {}", std::env::consts::ARCH);

    let mut detection_notes = Vec::new();
    let mut probe_log = HardwareProbeLog::default();
    let mut attempted_preferred_vendor = None;

    if let Some(preferred_vendor) = preferred_vendor {
        if let Some(parsed_vendor) = parse_preferred_vendor(&preferred_vendor) {
            if parsed_vendor == Vendor::Cpu && !allow_cpu_fallback {
                warn!(
                    "Preferred vendor '{}' requested but CPU fallback is disabled.",
                    preferred_vendor
                );
            } else if let Some(info) =
                detect_preferred_hardware_with_runner_and_log(runner, parsed_vendor, &mut probe_log)
            {
                info!(
                    "✓ Using preferred vendor '{}' ({})",
                    preferred_vendor, info.vendor
                );
                return Ok((info.with_detection_notes(detection_notes), probe_log));
            } else {
                if let Some(note) = note_for_failed_vendor(parsed_vendor, &probe_log) {
                    append_detection_note(&mut detection_notes, note);
                }
                warn!(
                    "Preferred vendor '{}' is unavailable. Falling back to auto detection.",
                    preferred_vendor
                );
                attempted_preferred_vendor = Some(parsed_vendor);
            }
        } else {
            warn!(
                "Unknown preferred vendor '{}'. Falling back to auto detection.",
                preferred_vendor
            );
        }
    }

    if attempted_preferred_vendor != Some(Vendor::Apple) {
        if let Some(info) = try_detect_apple_with_runner_and_log(runner, &mut probe_log) {
            info!("✓ Hardware acceleration: VideoToolbox");
            return Ok((info.with_detection_notes(detection_notes), probe_log));
        }
        if let Some(note) = note_for_failed_vendor(Vendor::Apple, &probe_log) {
            append_detection_note(&mut detection_notes, note);
        }
    }

    if attempted_preferred_vendor != Some(Vendor::Nvidia) {
        if let Some(info) = try_detect_nvidia_with_runner_and_log(runner, &mut probe_log) {
            info!("✓ Hardware acceleration: NVENC");
            return Ok((info.with_detection_notes(detection_notes), probe_log));
        }
        if let Some(note) = note_for_failed_vendor(Vendor::Nvidia, &probe_log) {
            append_detection_note(&mut detection_notes, note);
        }
    }

    if attempted_preferred_vendor != Some(Vendor::Intel) {
        if let Some(info) = try_detect_intel_with_runner_and_log(runner, &mut probe_log) {
            info!("✓ Hardware acceleration: Intel VAAPI/QSV");
            return Ok((info.with_detection_notes(detection_notes), probe_log));
        }
        if let Some(note) = note_for_failed_vendor(Vendor::Intel, &probe_log) {
            append_detection_note(&mut detection_notes, note);
        }
    }

    if attempted_preferred_vendor != Some(Vendor::Amd) {
        if let Some(info) = try_detect_amd_with_runner_and_log(runner, &mut probe_log) {
            info!(
                "✓ Hardware acceleration: {}",
                if cfg!(target_os = "windows") {
                    "AMF"
                } else {
                    "VAAPI"
                }
            );
            return Ok((info.with_detection_notes(detection_notes), probe_log));
        }
        if let Some(note) = note_for_failed_vendor(Vendor::Amd, &probe_log) {
            append_detection_note(&mut detection_notes, note);
        }
    }

    if !allow_cpu_fallback {
        error!("✗ No supported GPU detected and CPU fallback is disabled.");
        return Err(crate::error::AlchemistError::Config(
            "No GPU detected and CPU fallback disabled".into(),
        ));
    }

    warn!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    warn!("⚠  NO GPU DETECTED - FALLING BACK TO CPU ENCODING");
    warn!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    warn!("CPU encoding will be significantly slower than GPU acceleration.");
    warn!("Expected performance: 10-50x slower depending on resolution.");
    warn!("Software encoder: libsvtav1 (AV1) or libx264 (H.264)");
    info!("✓ CPU fallback mode enabled");

    Ok((
        HardwareInfo::new(Vendor::Cpu, None, Vec::new()).with_detection_notes(detection_notes),
        probe_log,
    ))
}

pub fn detect_hardware_with_preference(
    allow_cpu_fallback: bool,
    preferred_vendor: Option<String>,
) -> Result<HardwareInfo> {
    detect_hardware_with_preference_and_runner(
        &SystemCommandRunner,
        allow_cpu_fallback,
        preferred_vendor,
    )
}

pub fn detect_hardware_with_preference_and_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    allow_cpu_fallback: bool,
    preferred_vendor: Option<String>,
) -> Result<HardwareInfo> {
    detect_hardware_with_preference_and_runner_inner(runner, allow_cpu_fallback, preferred_vendor)
        .map(|(info, _)| info)
}

pub fn detect_hardware(allow_cpu_fallback: bool) -> Result<HardwareInfo> {
    detect_hardware_with_runner(&SystemCommandRunner, allow_cpu_fallback)
}

pub fn detect_hardware_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    allow_cpu_fallback: bool,
) -> Result<HardwareInfo> {
    detect_hardware_with_preference_and_runner_inner(runner, allow_cpu_fallback, None)
        .map(|(info, _)| info)
}

pub async fn detect_hardware_async(allow_cpu_fallback: bool) -> Result<HardwareInfo> {
    tokio::task::spawn_blocking(move || detect_hardware(allow_cpu_fallback))
        .await
        .map_err(|e| {
            crate::error::AlchemistError::Config(format!("spawn_blocking failed: {}", e))
        })?
}

pub async fn detect_hardware_async_with_preference(
    allow_cpu_fallback: bool,
    preferred_vendor: Option<String>,
) -> Result<HardwareInfo> {
    tokio::task::spawn_blocking(move || {
        detect_hardware_with_preference(allow_cpu_fallback, preferred_vendor)
    })
    .await
    .map_err(|e| crate::error::AlchemistError::Config(format!("spawn_blocking failed: {}", e)))?
}

fn detect_hardware_for_config_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    config: &crate::config::Config,
) -> Result<(HardwareInfo, HardwareProbeLog)> {
    if let Some(device_path) = config
        .hardware
        .device_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if cfg!(target_os = "linux") {
            let preferred_vendor = config
                .hardware
                .preferred_vendor
                .as_deref()
                .and_then(parse_preferred_vendor);
            let mut probe_log = HardwareProbeLog::default();
            let info = detect_explicit_device_path_with_runner_and_log(
                runner,
                device_path,
                preferred_vendor,
                &mut probe_log,
            )
            .ok_or_else(|| {
                crate::error::AlchemistError::Config(format!(
                    "Configured device path '{}' did not expose a supported encoder",
                    device_path
                ))
            })?;

            if info.vendor == Vendor::Cpu && !config.hardware.allow_cpu_encoding {
                return Err(crate::error::AlchemistError::Config(
                    "CPU encoding disabled".into(),
                ));
            }

            return Ok((info, probe_log));
        }

        warn!(
            "Ignoring configured device path '{}' on unsupported platform {}",
            device_path,
            std::env::consts::OS
        );
    }

    let (info, probe_log) = detect_hardware_with_preference_and_runner_inner(
        runner,
        config.hardware.allow_cpu_fallback,
        config.hardware.preferred_vendor.clone(),
    )?;

    if info.vendor == Vendor::Cpu && !config.hardware.allow_cpu_encoding {
        return Err(crate::error::AlchemistError::Config(
            "CPU encoding disabled".into(),
        ));
    }

    Ok((info, probe_log))
}

pub async fn detect_hardware_with_log(
    config: &crate::config::Config,
) -> Result<(HardwareInfo, HardwareProbeLog)> {
    let config = config.clone();
    tokio::task::spawn_blocking(move || {
        detect_hardware_for_config_with_runner(&SystemCommandRunner, &config)
    })
    .await
    .map_err(|e| crate::error::AlchemistError::Config(format!("spawn_blocking failed: {}", e)))?
}

pub async fn detect_hardware_for_config(config: &crate::config::Config) -> Result<HardwareInfo> {
    detect_hardware_with_log(config).await.map(|(info, _)| info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::process::Output;

    #[derive(Default)]
    struct FakeRunner {
        successful_encoders: HashSet<String>,
        nvidia_smi_ok: bool,
    }

    impl FakeRunner {
        fn with_successful_encoders(encoders: &[&str]) -> Self {
            Self {
                successful_encoders: encoders.iter().map(|encoder| encoder.to_string()).collect(),
                nvidia_smi_ok: true,
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn output(&self, program: &str, args: &[String]) -> std::io::Result<Output> {
            match program {
                "nvidia-smi" if self.nvidia_smi_ok => Ok(Output {
                    status: exit_status(true),
                    stdout: b"GPU 0".to_vec(),
                    stderr: Vec::new(),
                }),
                "nvidia-smi" => Ok(Output {
                    status: exit_status(false),
                    stdout: Vec::new(),
                    stderr: b"missing".to_vec(),
                }),
                "ffmpeg" => {
                    let success = args
                        .iter()
                        .any(|arg| self.successful_encoders.contains(arg));
                    Ok(Output {
                        status: exit_status(success),
                        stdout: Vec::new(),
                        stderr: if success {
                            Vec::new()
                        } else {
                            b"encoder unavailable".to_vec()
                        },
                    })
                }
                _ => Ok(Output {
                    status: exit_status(false),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                }),
            }
        }

        fn status(
            &self,
            program: &str,
            args: &[String],
        ) -> std::io::Result<std::process::ExitStatus> {
            if program != "ffmpeg" {
                return Ok(exit_status(false));
            }

            let success = args
                .iter()
                .any(|arg| self.successful_encoders.contains(arg));
            Ok(exit_status(success))
        }
    }

    fn exit_status(success: bool) -> std::process::ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(if success { 0 } else { 1 } << 8)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(if success { 0 } else { 1 })
        }
    }

    #[tokio::test]
    async fn hardware_state_updates_snapshot() {
        let state = HardwareState::new(Some(HardwareInfo::new(
            Vendor::Nvidia,
            None,
            vec![BackendCapability {
                kind: HardwareBackend::Nvenc,
                codec: "av1".to_string(),
                encoder: "av1_nvenc".to_string(),
                device_path: None,
            }],
        )));
        assert_eq!(state.snapshot().await.unwrap().vendor, Vendor::Nvidia);

        state
            .replace(Some(HardwareInfo::new(Vendor::Cpu, None, Vec::new())))
            .await;

        assert_eq!(state.snapshot().await.unwrap().vendor, Vendor::Cpu);
    }

    #[test]
    fn vaapi_probe_uses_hwupload() {
        let args = probe_args_for_backend(
            HardwareBackend::Vaapi,
            "hevc_vaapi",
            Some("/dev/dri/renderD128"),
        );
        assert!(args.contains(&"-vaapi_device".to_string()));
        assert!(args.contains(&"format=nv12,hwupload".to_string()));
    }

    #[test]
    fn qsv_probe_uses_hw_device_init() {
        let args =
            probe_args_for_backend(HardwareBackend::Qsv, "av1_qsv", Some("/dev/dri/renderD129"));
        assert!(args.contains(&"-init_hw_device".to_string()));
        assert!(args.contains(&"qsv=qsv:/dev/dri/renderD129".to_string()));
    }

    #[test]
    fn videotoolbox_probe_uses_yuv420p_filter_and_software_fallback() {
        let args = probe_args_for_backend(HardwareBackend::Videotoolbox, "hevc_videotoolbox", None);
        assert!(args.contains(&"format=yuv420p".to_string()));
        assert!(args.contains(&"-allow_sw".to_string()));
    }

    #[test]
    fn fake_runner_collects_probe_capabilities_for_all_backends() {
        let runner = FakeRunner::with_successful_encoders(&[
            "av1_nvenc",
            "hevc_amf",
            "av1_vaapi",
            "av1_qsv",
            "h264_vaapi",
            "hevc_videotoolbox",
        ]);

        let nvidia = collect_backend_capabilities(
            &runner,
            backend_probe_specs_for_vendor(Vendor::Nvidia, None),
        );
        assert_eq!(nvidia[0].kind, HardwareBackend::Nvenc);

        let amd = collect_backend_capabilities(
            &runner,
            backend_probe_specs_for_vendor(Vendor::Amd, Some("/dev/dri/renderD128")),
        );
        assert_eq!(
            amd[0].kind,
            if cfg!(target_os = "windows") {
                HardwareBackend::Amf
            } else {
                HardwareBackend::Vaapi
            }
        );

        let intel = collect_backend_capabilities(
            &runner,
            backend_probe_specs_for_vendor(Vendor::Intel, Some("/dev/dri/renderD129")),
        );
        assert_eq!(intel[0].kind, HardwareBackend::Vaapi);
        assert_eq!(intel[0].codec, "av1");
        assert!(
            intel.iter().any(|backend| {
                backend.kind == HardwareBackend::Vaapi && backend.codec == "h264"
            })
        );
        assert_eq!(
            intel
                .iter()
                .filter(|backend| backend.codec == "av1")
                .count(),
            1
        );

        let apple = collect_backend_capabilities(
            &runner,
            backend_probe_specs_for_vendor(Vendor::Apple, None),
        );
        assert_eq!(apple[0].kind, HardwareBackend::Videotoolbox);
    }

    #[test]
    fn detect_hardware_with_runner_can_fall_back_to_cpu() {
        let runner = FakeRunner::default();
        let info = detect_hardware_with_runner(&runner, true).expect("cpu fallback");
        assert_eq!(info.vendor, Vendor::Cpu);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn explicit_device_path_probe_supports_qsv_and_vaapi() {
        let temp_root = std::env::temp_dir();
        let qsv_path = temp_root.join(format!("alchemist_qsv_{}", rand::random::<u64>()));
        let vaapi_path = temp_root.join(format!("alchemist_vaapi_{}", rand::random::<u64>()));
        std::fs::write(&qsv_path, b"render").expect("create qsv path");
        std::fs::write(&vaapi_path, b"render").expect("create vaapi path");

        let qsv_runner = FakeRunner::with_successful_encoders(&["av1_qsv"]);
        let qsv_info = detect_explicit_device_path_with_runner(
            &qsv_runner,
            qsv_path.to_string_lossy().as_ref(),
            Some(Vendor::Intel),
        )
        .expect("qsv info");
        assert_eq!(qsv_info.vendor, Vendor::Intel);

        let vaapi_runner = FakeRunner::with_successful_encoders(&["hevc_vaapi"]);
        let vaapi_info = detect_explicit_device_path_with_runner(
            &vaapi_runner,
            vaapi_path.to_string_lossy().as_ref(),
            Some(Vendor::Amd),
        )
        .expect("vaapi info");
        assert_eq!(vaapi_info.vendor, Vendor::Amd);

        let _ = std::fs::remove_file(qsv_path);
        let _ = std::fs::remove_file(vaapi_path);
    }
}
