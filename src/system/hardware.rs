use crate::error::Result;
use std::collections::{BTreeSet, HashMap};
use std::io;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
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

impl Vendor {
    fn as_str(self) -> &'static str {
        match self {
            Vendor::Nvidia => "nvidia",
            Vendor::Amd => "amd",
            Vendor::Intel => "intel",
            Vendor::Apple => "apple",
            Vendor::Cpu => "cpu",
        }
    }

    fn short_name(self) -> &'static str {
        match self {
            Vendor::Nvidia => "NVIDIA",
            Vendor::Amd => "AMD",
            Vendor::Intel => "Intel",
            Vendor::Apple => "Apple",
            Vendor::Cpu => "CPU",
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProbeSummary {
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProbeEntry {
    pub encoder: String,
    pub backend: String,
    pub device_path: Option<String>,
    pub success: bool,
    pub stderr: Option<String>,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub summary: String,
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
    #[serde(default)]
    pub selection_reason: String,
    #[serde(default)]
    pub probe_summary: ProbeSummary,
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
            selection_reason: String::new(),
            probe_summary: ProbeSummary::default(),
        }
    }

    pub fn supports_codec(&self, codec: &str) -> bool {
        self.supported_codecs.iter().any(|value| value == codec)
    }

    fn with_detection_notes(mut self, detection_notes: Vec<String>) -> Self {
        self.detection_notes = detection_notes;
        self
    }

    fn with_probe_details(
        mut self,
        selection_reason: impl Into<String>,
        probe_summary: ProbeSummary,
    ) -> Self {
        self.selection_reason = selection_reason.into();
        self.probe_summary = probe_summary;
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
}

pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn output(&self, program: &str, args: &[String]) -> std::io::Result<Output> {
        run_command_with_timeout(program, args, Duration::from_secs(8))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeCandidate {
    vendor: Vendor,
    backend: HardwareBackend,
    codec: String,
    encoder: String,
    device_path: Option<String>,
    discovery_note: String,
}

#[derive(Debug, Clone)]
struct SuccessfulCandidateSet {
    vendor: Vendor,
    device_path: Option<String>,
    backends: Vec<BackendCapability>,
    discovery_notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionMode {
    Auto,
    PreferredVendor(Vendor),
    ExplicitDevicePath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinuxRenderNode {
    path: String,
    vendor: Vendor,
    discovery_note: String,
}

#[derive(Debug, Clone)]
struct ProbeResult {
    candidate: ProbeCandidate,
    success: bool,
    stderr: String,
    summary: String,
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
        "color=c=black:s=256x256:d=0.1".to_string(),
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

fn run_command_with_timeout(
    program: &str,
    args: &[String],
    timeout: Duration,
) -> io::Result<Output> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let start = std::time::Instant::now();

    loop {
        if let Some(_status) = child.try_wait()? {
            return child.wait_with_output();
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "probe timed out after {}s while running {}",
                    timeout.as_secs(),
                    program
                ),
            ));
        }

        std::thread::sleep(Duration::from_millis(50));
    }
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

fn probe_candidates_for_vendor(
    vendor: Vendor,
    device_path: Option<&str>,
    discovery_note: &str,
) -> Vec<ProbeCandidate> {
    let device_path = device_path.map(|value| value.to_string());
    let discovery_note = discovery_note.to_string();
    let build = |backend, codec: &str, encoder: &str, device_path: Option<String>| ProbeCandidate {
        vendor,
        backend,
        codec: codec.to_string(),
        encoder: encoder.to_string(),
        device_path,
        discovery_note: discovery_note.clone(),
    };

    match vendor {
        Vendor::Apple => vec![
            build(
                HardwareBackend::Videotoolbox,
                "av1",
                "av1_videotoolbox",
                None,
            ),
            build(
                HardwareBackend::Videotoolbox,
                "hevc",
                "hevc_videotoolbox",
                None,
            ),
            build(
                HardwareBackend::Videotoolbox,
                "h264",
                "h264_videotoolbox",
                None,
            ),
        ],
        Vendor::Nvidia => vec![
            build(HardwareBackend::Nvenc, "av1", "av1_nvenc", None),
            build(HardwareBackend::Nvenc, "hevc", "hevc_nvenc", None),
            build(HardwareBackend::Nvenc, "h264", "h264_nvenc", None),
        ],
        Vendor::Intel => vec![
            build(
                HardwareBackend::Vaapi,
                "av1",
                "av1_vaapi",
                device_path.clone(),
            ),
            build(
                HardwareBackend::Vaapi,
                "hevc",
                "hevc_vaapi",
                device_path.clone(),
            ),
            build(
                HardwareBackend::Vaapi,
                "h264",
                "h264_vaapi",
                device_path.clone(),
            ),
            build(HardwareBackend::Qsv, "av1", "av1_qsv", device_path.clone()),
            build(
                HardwareBackend::Qsv,
                "hevc",
                "hevc_qsv",
                device_path.clone(),
            ),
            build(HardwareBackend::Qsv, "h264", "h264_qsv", device_path),
        ],
        Vendor::Amd if cfg!(target_os = "windows") => vec![
            build(HardwareBackend::Amf, "av1", "av1_amf", None),
            build(HardwareBackend::Amf, "hevc", "hevc_amf", None),
            build(HardwareBackend::Amf, "h264", "h264_amf", None),
        ],
        Vendor::Amd => vec![
            build(
                HardwareBackend::Vaapi,
                "av1",
                "av1_vaapi",
                device_path.clone(),
            ),
            build(
                HardwareBackend::Vaapi,
                "hevc",
                "hevc_vaapi",
                device_path.clone(),
            ),
            build(HardwareBackend::Vaapi, "h264", "h264_vaapi", device_path),
        ],
        Vendor::Cpu => Vec::new(),
    }
}

fn pci_vendor_to_vendor(value: &str) -> Option<Vendor> {
    match value.trim().to_ascii_lowercase().as_str() {
        "0x8086" => Some(Vendor::Intel),
        "0x1002" => Some(Vendor::Amd),
        "0x10de" => Some(Vendor::Nvidia),
        _ => None,
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
    pci_vendor_to_vendor(&vendor_id)
}

fn enumerate_linux_render_nodes_under(
    sys_class_drm: &Path,
    dev_dri_root: &Path,
) -> Vec<LinuxRenderNode> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }

    let entries = match std::fs::read_dir(sys_class_drm) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut render_nodes = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let render_name = match file_name.to_str() {
            Some(value) if value.starts_with("renderD") => value.to_string(),
            _ => continue,
        };

        let vendor_path = entry.path().join("device/vendor");
        let vendor_id = match std::fs::read_to_string(vendor_path) {
            Ok(vendor_id) => vendor_id,
            Err(_) => continue,
        };
        let vendor = match pci_vendor_to_vendor(&vendor_id) {
            Some(vendor) => vendor,
            None => continue,
        };

        let device_path = dev_dri_root.join(&render_name);
        if !device_path.exists() {
            continue;
        }

        render_nodes.push(LinuxRenderNode {
            path: device_path.to_string_lossy().to_string(),
            vendor,
            discovery_note: format!("Discovered DRM render node {}", device_path.display()),
        });
    }

    render_nodes.sort_by(|a, b| a.path.cmp(&b.path));
    render_nodes
}

fn enumerate_linux_render_nodes() -> Vec<LinuxRenderNode> {
    enumerate_linux_render_nodes_under(Path::new("/sys/class/drm"), Path::new("/dev/dri"))
}

fn summarize_probe_failure(stderr: &str) -> String {
    let first_line = stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("probe failed");
    let lower = stderr.to_ascii_lowercase();

    if lower.contains("timed out") {
        "Probe timed out".to_string()
    } else if lower.contains("unknown encoder") {
        "Encoder unavailable in current FFmpeg build".to_string()
    } else if lower.contains("permission denied") {
        "Device permission denied".to_string()
    } else if lower.contains("no such file or directory")
        || lower.contains("cannot open")
        || lower.contains("failed to open")
    {
        "Device path unavailable".to_string()
    } else if lower.contains("cannot load libcuda")
        || lower.contains("cuda")
        || lower.contains("nvenc")
    {
        "NVENC/CUDA initialization failed".to_string()
    } else if lower.contains("no va display found")
        || lower.contains("vaapi")
        || lower.contains("hwupload")
    {
        "VAAPI device initialization failed".to_string()
    } else if lower.contains("device creation failed")
        || lower.contains("failed to initialise")
        || lower.contains("failed to initialize")
    {
        "Hardware device initialization failed".to_string()
    } else if lower.contains("frame dimension") && lower.contains("minimum supported value") {
        "Probe frame was rejected by the encoder".to_string()
    } else {
        first_line.to_string()
    }
}

fn probe_candidate_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    candidate: &ProbeCandidate,
) -> ProbeResult {
    let (success, stderr) = probe_backend_encoder_verbose_with_runner(
        runner,
        candidate.backend,
        &candidate.encoder,
        candidate.device_path.as_deref(),
    );
    let stderr = stderr.trim().to_string();
    let summary = if success {
        format!(
            "{} {} probe succeeded",
            candidate.backend.display_name(),
            candidate.codec.to_uppercase()
        )
    } else {
        summarize_probe_failure(&stderr)
    };

    ProbeResult {
        candidate: candidate.clone(),
        success,
        stderr,
        summary,
    }
}

fn collect_probe_results_verbose<R: CommandRunner + ?Sized>(
    runner: &R,
    candidates: Vec<ProbeCandidate>,
    probe_log: &mut HardwareProbeLog,
) -> Vec<ProbeResult> {
    let mut results = Vec::new();

    for candidate in candidates {
        let result = probe_candidate_with_runner(runner, &candidate);
        let stderr_value = (!result.stderr.is_empty()).then_some(result.stderr.clone());

        probe_log.entries.push(HardwareProbeEntry {
            encoder: candidate.encoder.clone(),
            backend: candidate.backend.as_str().to_string(),
            device_path: candidate.device_path.clone(),
            success: result.success,
            stderr: stderr_value,
            vendor: candidate.vendor.as_str().to_string(),
            codec: candidate.codec.clone(),
            selected: false,
            summary: result.summary.clone(),
        });

        if !result.success {
            debug!(
                "{} probe failed for {}: {}",
                candidate.backend.display_name(),
                candidate.encoder,
                result.summary,
            );
        }

        results.push(result);
    }

    results
}

fn build_successful_candidate_sets(results: &[ProbeResult]) -> Vec<SuccessfulCandidateSet> {
    let mut groups: HashMap<(Vendor, Option<String>), SuccessfulCandidateSet> = HashMap::new();

    for result in results.iter().filter(|result| result.success) {
        let key = (
            result.candidate.vendor,
            result.candidate.device_path.clone(),
        );
        let entry = groups.entry(key).or_insert_with(|| SuccessfulCandidateSet {
            vendor: result.candidate.vendor,
            device_path: result.candidate.device_path.clone(),
            backends: Vec::new(),
            discovery_notes: Vec::new(),
        });
        if !entry
            .discovery_notes
            .iter()
            .any(|note| note == &result.candidate.discovery_note)
        {
            entry
                .discovery_notes
                .push(result.candidate.discovery_note.clone());
        }
        entry.backends.push(BackendCapability {
            kind: result.candidate.backend,
            codec: result.candidate.codec.clone(),
            encoder: result.candidate.encoder.clone(),
            device_path: result.candidate.device_path.clone(),
        });
    }

    let mut sets: Vec<_> = groups.into_values().collect();
    sets.sort_by(|a, b| a.device_path.cmp(&b.device_path));
    sets
}

fn codec_weight(codec: &str) -> u32 {
    match codec {
        "av1" => 4,
        "hevc" => 2,
        "h264" => 1,
        _ => 0,
    }
}

fn codec_coverage_score(backends: &[BackendCapability]) -> u32 {
    let codecs: BTreeSet<_> = backends
        .iter()
        .map(|backend| backend.codec.as_str())
        .collect();
    codecs.into_iter().map(codec_weight).sum()
}

fn backend_preference_rank(vendor: Vendor, backends: &[BackendCapability]) -> usize {
    let mut best_rank = usize::MAX;
    for backend in backends {
        let rank = match (vendor, backend.kind) {
            (Vendor::Intel, HardwareBackend::Vaapi) => 0,
            (Vendor::Intel, HardwareBackend::Qsv) => 1,
            (Vendor::Amd, HardwareBackend::Amf) => 0,
            (Vendor::Amd, HardwareBackend::Vaapi) => 1,
            (Vendor::Apple, HardwareBackend::Videotoolbox) => 0,
            (Vendor::Nvidia, HardwareBackend::Nvenc) => 0,
            (_, _) => 2,
        };
        best_rank = best_rank.min(rank);
    }
    best_rank
}

fn vendor_auto_rank(vendor: Vendor) -> usize {
    match vendor {
        Vendor::Apple => 0,
        Vendor::Nvidia => 1,
        Vendor::Intel => 2,
        Vendor::Amd => 3,
        Vendor::Cpu => 4,
    }
}

fn compare_candidate_sets(
    left: &SuccessfulCandidateSet,
    right: &SuccessfulCandidateSet,
    include_vendor_rank: bool,
) -> std::cmp::Ordering {
    codec_coverage_score(right.backends.as_slice())
        .cmp(&codec_coverage_score(left.backends.as_slice()))
        .then_with(|| {
            backend_preference_rank(left.vendor, left.backends.as_slice()).cmp(
                &backend_preference_rank(right.vendor, right.backends.as_slice()),
            )
        })
        .then_with(|| {
            if include_vendor_rank {
                vendor_auto_rank(left.vendor).cmp(&vendor_auto_rank(right.vendor))
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .then_with(|| left.device_path.cmp(&right.device_path))
}

fn choose_best_candidate_set(
    sets: &[SuccessfulCandidateSet],
    preferred_vendor: Option<Vendor>,
    explicit_device_path: bool,
) -> Option<(SuccessfulCandidateSet, SelectionMode)> {
    if let Some(preferred_vendor) = preferred_vendor.filter(|vendor| *vendor != Vendor::Cpu) {
        let preferred_sets: Vec<_> = sets
            .iter()
            .filter(|set| set.vendor == preferred_vendor)
            .cloned()
            .collect();
        if !preferred_sets.is_empty() {
            let mut preferred_sets = preferred_sets;
            preferred_sets.sort_by(|left, right| compare_candidate_sets(left, right, false));
            let mode = if explicit_device_path {
                SelectionMode::ExplicitDevicePath
            } else {
                SelectionMode::PreferredVendor(preferred_vendor)
            };
            return preferred_sets.into_iter().next().map(|set| (set, mode));
        }
    }

    let mut sets = sets.to_vec();
    sets.sort_by(|left, right| compare_candidate_sets(left, right, !explicit_device_path));
    let mode = if explicit_device_path {
        SelectionMode::ExplicitDevicePath
    } else {
        SelectionMode::Auto
    };
    sets.into_iter().next().map(|set| (set, mode))
}

fn format_codec_list(backends: &[BackendCapability]) -> String {
    let codecs: BTreeSet<_> = backends
        .iter()
        .map(|backend| match backend.codec.as_str() {
            "h264" => "H.264".to_string(),
            "hevc" => "HEVC".to_string(),
            "av1" => "AV1".to_string(),
            other => other.to_uppercase(),
        })
        .collect();
    codecs.into_iter().collect::<Vec<_>>().join(", ")
}

fn format_backend_list(backends: &[BackendCapability]) -> String {
    let backend_names: BTreeSet<_> = backends
        .iter()
        .map(|backend| backend.kind.display_name().to_string())
        .collect();
    backend_names.into_iter().collect::<Vec<_>>().join("/")
}

fn selection_reason_for(
    selected: &SuccessfulCandidateSet,
    selection_mode: SelectionMode,
) -> String {
    let codecs = format_codec_list(selected.backends.as_slice());
    let backend_names = format_backend_list(selected.backends.as_slice());
    let path_fragment = selected
        .device_path
        .as_deref()
        .map(|path| format!(" at {}", path))
        .unwrap_or_default();

    match selection_mode {
        SelectionMode::ExplicitDevicePath => format!(
            "Selected {}{} because it matched the configured device path and exposed {} through {}.",
            selected.vendor.short_name(),
            path_fragment,
            codecs,
            backend_names
        ),
        SelectionMode::PreferredVendor(vendor) => format!(
            "Selected preferred vendor {}{} because it exposed {} through {}.",
            vendor.short_name(),
            path_fragment,
            codecs,
            backend_names
        ),
        SelectionMode::Auto => format!(
            "Auto-selected {}{} because it had the strongest codec coverage ({} via {}).",
            selected.vendor.short_name(),
            path_fragment,
            codecs,
            backend_names
        ),
    }
}

fn probe_summary_for_log(probe_log: &HardwareProbeLog) -> ProbeSummary {
    let attempted = probe_log.entries.len();
    let succeeded = probe_log
        .entries
        .iter()
        .filter(|entry| entry.success)
        .count();
    ProbeSummary {
        attempted,
        succeeded,
        failed: attempted.saturating_sub(succeeded),
    }
}

fn append_failed_vendor_note(notes: &mut Vec<String>, vendor: Vendor, results: &[ProbeResult]) {
    let vendor_results: Vec<_> = results
        .iter()
        .filter(|result| result.candidate.vendor == vendor)
        .collect();
    if vendor_results.is_empty() || vendor_results.iter().any(|result| result.success) {
        return;
    }

    let first_failure = vendor_results
        .iter()
        .find(|result| !result.success)
        .map(|result| {
            let path = result
                .candidate
                .device_path
                .as_deref()
                .map(|path| format!(" at {}", path))
                .unwrap_or_default();
            format!("{}{} — {}", result.candidate.encoder, path, result.summary)
        })
        .unwrap_or_else(|| "unknown failure".to_string());

    append_detection_note(
        notes,
        format!(
            "{} probes failed ({} attempts). First failure: {}",
            vendor.short_name(),
            vendor_results.len(),
            first_failure
        ),
    );
}

fn mark_selected_probe_entries(
    probe_log: &mut HardwareProbeLog,
    selected: &SuccessfulCandidateSet,
) {
    for entry in &mut probe_log.entries {
        entry.selected = entry.success
            && entry.vendor == selected.vendor.as_str()
            && entry.device_path == selected.device_path;
    }
}

fn discover_nvidia_candidates_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    detection_notes: &mut Vec<String>,
) -> Vec<ProbeCandidate> {
    if cfg!(target_os = "windows") {
        return probe_candidates_for_vendor(
            Vendor::Nvidia,
            None,
            "Discovered NVIDIA candidate on Windows",
        );
    }

    if !Path::new("/dev/nvidiactl").exists() {
        append_detection_note(
            detection_notes,
            "NVIDIA not discovered — no /dev/nvidiactl found",
        );
        return Vec::new();
    }

    match runner.output("nvidia-smi", &[]) {
        Ok(output) if output.status.success() => probe_candidates_for_vendor(
            Vendor::Nvidia,
            None,
            "Discovered NVIDIA candidate via nvidia-smi",
        ),
        Ok(output) => {
            append_detection_note(
                detection_notes,
                format!(
                    "NVIDIA discovery failed — {}",
                    summarize_probe_failure(&String::from_utf8_lossy(&output.stderr))
                ),
            );
            Vec::new()
        }
        Err(err) => {
            append_detection_note(
                detection_notes,
                format!("NVIDIA discovery failed — {}", err),
            );
            Vec::new()
        }
    }
}

fn discover_probe_candidates_with_runner<R: CommandRunner + ?Sized>(
    runner: &R,
    detection_notes: &mut Vec<String>,
) -> Vec<ProbeCandidate> {
    let mut candidates = Vec::new();

    if cfg!(target_os = "macos") {
        candidates.extend(probe_candidates_for_vendor(
            Vendor::Apple,
            None,
            "Discovered Apple VideoToolbox candidate on macOS",
        ));
    }

    candidates.extend(discover_nvidia_candidates_with_runner(
        runner,
        detection_notes,
    ));

    let render_nodes = enumerate_linux_render_nodes();
    if cfg!(target_os = "linux") {
        if !render_nodes.iter().any(|node| node.vendor == Vendor::Intel) {
            append_detection_note(
                detection_notes,
                "Intel not discovered — no Intel DRM render node found",
            );
        }
        if !render_nodes.iter().any(|node| node.vendor == Vendor::Amd) {
            append_detection_note(
                detection_notes,
                "AMD not discovered — no AMD DRM render node found",
            );
        }
    }

    for render_node in render_nodes {
        match render_node.vendor {
            Vendor::Intel | Vendor::Amd => {
                candidates.extend(probe_candidates_for_vendor(
                    render_node.vendor,
                    Some(render_node.path.as_str()),
                    &render_node.discovery_note,
                ));
            }
            Vendor::Nvidia | Vendor::Apple | Vendor::Cpu => {}
        }
    }

    if cfg!(target_os = "windows") {
        candidates.extend(probe_candidates_for_vendor(
            Vendor::Amd,
            None,
            "Discovered AMD AMF candidate on Windows",
        ));
    }

    candidates
}

#[cfg(all(test, target_os = "linux"))]
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
    if !cfg!(target_os = "linux") || !Path::new(device_path).exists() {
        return None;
    }

    let mut detection_notes = Vec::new();
    let resolved_vendor =
        preferred_vendor.or_else(|| vendor_from_explicit_device_path(Path::new(device_path)));
    let candidates = match resolved_vendor {
        Some(Vendor::Intel) => probe_candidates_for_vendor(
            Vendor::Intel,
            Some(device_path),
            &format!("Using configured device path {}", device_path),
        ),
        Some(Vendor::Amd) => probe_candidates_for_vendor(
            Vendor::Amd,
            Some(device_path),
            &format!("Using configured device path {}", device_path),
        ),
        Some(Vendor::Cpu) | Some(Vendor::Apple) | Some(Vendor::Nvidia) => Vec::new(),
        None => {
            append_detection_note(
                &mut detection_notes,
                format!(
                    "Configured device path '{}' could not be mapped to a vendor; probing as both Intel and AMD render node",
                    device_path
                ),
            );
            let intel = probe_candidates_for_vendor(
                Vendor::Intel,
                Some(device_path),
                &format!("Using configured device path {}", device_path),
            );
            let amd = probe_candidates_for_vendor(
                Vendor::Amd,
                Some(device_path),
                &format!("Using configured device path {}", device_path),
            );
            [intel, amd].concat()
        }
    };

    if candidates.is_empty() {
        return None;
    }

    let results = collect_probe_results_verbose(runner, candidates, probe_log);
    let successful_sets = build_successful_candidate_sets(&results);
    let selected = choose_best_candidate_set(&successful_sets, preferred_vendor, true)?;
    let (selected, selection_mode) = selected;
    let selection_reason = selection_reason_for(&selected, selection_mode);
    let probe_summary = probe_summary_for_log(probe_log);
    let vendor = selected.vendor;
    let device_path = selected.device_path.clone();
    let backends = selected.backends.clone();

    for vendor in [Vendor::Intel, Vendor::Amd] {
        append_failed_vendor_note(&mut detection_notes, vendor, &results);
    }

    mark_selected_probe_entries(probe_log, &selected);
    Some(
        HardwareInfo::new(vendor, device_path, backends)
            .with_detection_notes(detection_notes)
            .with_probe_details(selection_reason, probe_summary),
    )
}

fn append_detection_note(notes: &mut Vec<String>, note: impl Into<String>) {
    let note = note.into();
    if !notes.iter().any(|existing| existing == &note) {
        notes.push(note);
    }
}

fn cpu_selection_reason(preferred_cpu: bool) -> String {
    if preferred_cpu {
        "Selected CPU because it was configured as the preferred vendor.".to_string()
    } else {
        "Selected CPU fallback because no GPU probe succeeded.".to_string()
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
    let parsed_preferred_vendor = preferred_vendor.as_deref().and_then(parse_preferred_vendor);
    if let Some(preferred_vendor) = preferred_vendor.as_deref() {
        if parsed_preferred_vendor.is_none() {
            warn!(
                "Unknown preferred vendor '{}'. Falling back to auto detection.",
                preferred_vendor
            );
            append_detection_note(
                &mut detection_notes,
                format!(
                    "Unknown preferred vendor '{}' — falling back to auto detection",
                    preferred_vendor
                ),
            );
        }
    }

    if matches!(parsed_preferred_vendor, Some(Vendor::Cpu)) && !allow_cpu_fallback {
        warn!("Preferred vendor 'cpu' requested but CPU fallback is disabled.");
    }

    let candidates = discover_probe_candidates_with_runner(runner, &mut detection_notes);
    let probe_results = collect_probe_results_verbose(runner, candidates, &mut probe_log);
    let successful_sets = build_successful_candidate_sets(&probe_results);

    if let Some(preferred_vendor) = parsed_preferred_vendor.filter(|vendor| *vendor != Vendor::Cpu)
    {
        if !successful_sets
            .iter()
            .any(|set| set.vendor == preferred_vendor)
        {
            append_detection_note(
                &mut detection_notes,
                format!(
                    "Preferred vendor '{}' had no successful probes. Falling back to auto detection.",
                    preferred_vendor.short_name()
                ),
            );
        }
    }

    for vendor in [Vendor::Apple, Vendor::Nvidia, Vendor::Intel, Vendor::Amd] {
        append_failed_vendor_note(&mut detection_notes, vendor, &probe_results);
    }

    if matches!(parsed_preferred_vendor, Some(Vendor::Cpu)) && allow_cpu_fallback {
        return Ok((
            HardwareInfo::new(Vendor::Cpu, None, Vec::new())
                .with_detection_notes(detection_notes)
                .with_probe_details(
                    cpu_selection_reason(true),
                    probe_summary_for_log(&probe_log),
                ),
            probe_log,
        ));
    }

    if let Some((selected, selection_mode)) =
        choose_best_candidate_set(&successful_sets, parsed_preferred_vendor, false)
    {
        let selection_reason = selection_reason_for(&selected, selection_mode);
        let probe_summary = probe_summary_for_log(&probe_log);
        let vendor = selected.vendor;
        let device_path = selected.device_path.clone();
        let backends = selected.backends.clone();
        mark_selected_probe_entries(&mut probe_log, &selected);
        info!(
            "✓ Hardware acceleration: {} ({})",
            selected.vendor.short_name(),
            format_backend_list(selected.backends.as_slice())
        );
        return Ok((
            HardwareInfo::new(vendor, device_path, backends)
                .with_detection_notes(detection_notes)
                .with_probe_details(selection_reason, probe_summary),
            probe_log,
        ));
    }

    if !allow_cpu_fallback {
        error!("✗ No supported GPU detected and CPU fallback is disabled.");
        for note in &detection_notes {
            warn!("{}", note);
        }
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
        HardwareInfo::new(Vendor::Cpu, None, Vec::new())
            .with_detection_notes(detection_notes)
            .with_probe_details(
                cpu_selection_reason(false),
                probe_summary_for_log(&probe_log),
            ),
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
    fn fake_runner_groups_successful_probes_by_vendor_and_device() {
        let runner = FakeRunner::with_successful_encoders(&[
            "av1_nvenc",
            if cfg!(target_os = "windows") {
                "hevc_amf"
            } else {
                "hevc_vaapi"
            },
            "av1_vaapi",
            "av1_qsv",
            "h264_vaapi",
            "hevc_videotoolbox",
        ]);
        let mut probe_log = HardwareProbeLog::default();
        let candidates = [
            probe_candidates_for_vendor(Vendor::Nvidia, None, "nvidia"),
            probe_candidates_for_vendor(Vendor::Amd, Some("/dev/dri/renderD128"), "amd"),
            probe_candidates_for_vendor(Vendor::Intel, Some("/dev/dri/renderD129"), "intel"),
            probe_candidates_for_vendor(Vendor::Apple, None, "apple"),
        ]
        .concat();

        let results = collect_probe_results_verbose(&runner, candidates, &mut probe_log);
        let sets = build_successful_candidate_sets(&results);

        assert!(sets.iter().any(|set| set.vendor == Vendor::Nvidia));
        assert!(sets.iter().any(|set| {
            set.vendor == Vendor::Amd && set.backends.iter().any(|backend| backend.codec == "hevc")
        }));
        assert!(sets.iter().any(|set| {
            set.vendor == Vendor::Intel
                && set
                    .backends
                    .iter()
                    .any(|backend| backend.kind == HardwareBackend::Qsv)
        }));
        assert!(sets.iter().any(|set| set.vendor == Vendor::Apple));
    }

    #[test]
    fn detect_hardware_with_runner_can_fall_back_to_cpu() {
        let runner = FakeRunner::default();
        let info = detect_hardware_with_runner(&runner, true).expect("cpu fallback");
        assert_eq!(info.vendor, Vendor::Cpu);
        assert_eq!(info.probe_summary.succeeded, 0);
        assert!(!info.selection_reason.is_empty());
    }

    fn candidate_set(
        vendor: Vendor,
        device_path: Option<&str>,
        backends: &[(HardwareBackend, &str, &str)],
    ) -> SuccessfulCandidateSet {
        SuccessfulCandidateSet {
            vendor,
            device_path: device_path.map(str::to_string),
            backends: backends
                .iter()
                .map(|(kind, codec, encoder)| BackendCapability {
                    kind: *kind,
                    codec: (*codec).to_string(),
                    encoder: (*encoder).to_string(),
                    device_path: device_path.map(str::to_string),
                })
                .collect(),
            discovery_notes: vec!["test".to_string()],
        }
    }

    #[test]
    fn preferred_vendor_falls_back_to_auto_selection() {
        let nvidia = candidate_set(
            Vendor::Nvidia,
            None,
            &[(HardwareBackend::Nvenc, "av1", "av1_nvenc")],
        );
        let amd = candidate_set(
            Vendor::Amd,
            Some("/dev/dri/renderD128"),
            &[(HardwareBackend::Vaapi, "hevc", "hevc_vaapi")],
        );

        let (selected, mode) =
            choose_best_candidate_set(&[nvidia.clone(), amd], Some(Vendor::Intel), false)
                .expect("selected set");
        assert_eq!(mode, SelectionMode::Auto);
        assert_eq!(selected.vendor, Vendor::Nvidia);
    }

    #[test]
    fn candidate_scoring_prefers_vaapi_over_qsv_for_intel_ties() {
        let vaapi = candidate_set(
            Vendor::Intel,
            Some("/dev/dri/renderD128"),
            &[(HardwareBackend::Vaapi, "hevc", "hevc_vaapi")],
        );
        let qsv = candidate_set(
            Vendor::Intel,
            Some("/dev/dri/renderD129"),
            &[(HardwareBackend::Qsv, "hevc", "hevc_qsv")],
        );

        let (selected, mode) =
            choose_best_candidate_set(&[qsv, vaapi.clone()], None, false).expect("selected set");
        assert_eq!(mode, SelectionMode::Auto);
        assert_eq!(selected.device_path, vaapi.device_path);
    }

    #[test]
    fn probe_log_entries_include_vendor_codec_summary_and_selection() {
        let runner = FakeRunner::with_successful_encoders(&["hevc_qsv"]);
        let mut probe_log = HardwareProbeLog::default();
        let candidates = probe_candidates_for_vendor(
            Vendor::Intel,
            Some("/dev/dri/renderD129"),
            "intel render node",
        );

        let results = collect_probe_results_verbose(&runner, candidates, &mut probe_log);
        let successful_sets = build_successful_candidate_sets(&results);
        let (selected, _) = choose_best_candidate_set(&successful_sets, Some(Vendor::Intel), false)
            .expect("selected set");
        mark_selected_probe_entries(&mut probe_log, &selected);

        assert!(
            probe_log
                .entries
                .iter()
                .all(|entry| !entry.vendor.is_empty())
        );
        assert!(
            probe_log
                .entries
                .iter()
                .all(|entry| !entry.codec.is_empty())
        );
        assert!(
            probe_log
                .entries
                .iter()
                .all(|entry| !entry.summary.is_empty())
        );
        assert!(probe_log.entries.iter().any(|entry| entry.selected));
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

    #[cfg(target_os = "linux")]
    #[test]
    fn explicit_device_path_probe_returns_none_when_no_probe_succeeds() {
        let temp_root = std::env::temp_dir();
        let missing_path = temp_root.join(format!(
            "alchemist_explicit_failure_{}",
            rand::random::<u64>()
        ));
        std::fs::write(&missing_path, b"render").expect("create explicit device path");

        let runner = FakeRunner::default();
        let info = detect_explicit_device_path_with_runner(
            &runner,
            missing_path.to_string_lossy().as_ref(),
            Some(Vendor::Intel),
        );
        assert!(info.is_none());

        let _ = std::fs::remove_file(missing_path);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_render_node_enumeration_discovers_multiple_devices() {
        let temp_root =
            std::env::temp_dir().join(format!("alchemist_render_enum_{}", rand::random::<u64>()));
        let sys_root = temp_root.join("sys/class/drm");
        let dev_root = temp_root.join("dev/dri");
        std::fs::create_dir_all(sys_root.join("renderD128/device")).expect("create intel sys path");
        std::fs::create_dir_all(sys_root.join("renderD129/device")).expect("create amd sys path");
        std::fs::create_dir_all(&dev_root).expect("create dev root");
        std::fs::write(sys_root.join("renderD128/device/vendor"), "0x8086")
            .expect("write intel vendor");
        std::fs::write(sys_root.join("renderD129/device/vendor"), "0x1002")
            .expect("write amd vendor");
        std::fs::write(dev_root.join("renderD128"), b"render").expect("write intel render node");
        std::fs::write(dev_root.join("renderD129"), b"render").expect("write amd render node");

        let nodes = enumerate_linux_render_nodes_under(&sys_root, &dev_root);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].vendor, Vendor::Intel);
        assert_eq!(nodes[1].vendor, Vendor::Amd);
        assert!(nodes[0].path.ends_with("renderD128"));
        assert!(nodes[1].path.ends_with("renderD129"));

        let _ = std::fs::remove_dir_all(temp_root);
    }
}
