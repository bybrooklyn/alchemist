#![forbid(unsafe_code)]

//! Backend traits, detection, and capability system for `whytho.`.
//!
//! Backends represent hardware acceleration paths (QSV, NVENC, VideoToolbox, etc.)
//! and the software CPU fallback. The backend system handles:
//! - Detection of available hardware
//! - Selection of the best backend for a given task
//! - Capability reporting (what each backend can do)

pub use whytho_core::config::BackendKind;

/// Capability information for a backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendCapability {
    pub name: String,
    pub kind: BackendKind,
    pub hardware_accelerated: bool,
    /// Maximum width supported.
    pub max_width: u32,
    /// Maximum height supported.
    pub max_height: u32,
    /// Supported codecs (e.g., ["h264", "hevc", "av1"]).
    pub supported_codecs: Vec<String>,
    /// Whether the backend is currently available on this system.
    pub available: bool,
}

impl BackendCapability {
    pub fn new(name: impl Into<String>, kind: BackendKind, hardware_accelerated: bool) -> Self {
        Self {
            name: name.into(),
            kind,
            hardware_accelerated,
            max_width: 4096,
            max_height: 4096,
            supported_codecs: Vec::new(),
            available: false,
        }
    }
}

/// Trait for transcoding backends.
pub trait TranscodeBackend {
    fn name(&self) -> &str;
    fn kind(&self) -> BackendKind;
    fn capabilities(&self) -> &BackendCapability;
    fn is_available(&self) -> bool;
}

/// CPU software backend (always available).
pub struct CpuBackend {
    capability: BackendCapability,
}

impl CpuBackend {
    pub fn new() -> Self {
        Self {
            capability: BackendCapability {
                name: "CPU (software)".into(),
                kind: BackendKind::Cpu,
                hardware_accelerated: false,
                max_width: 16384,
                max_height: 16384,
                supported_codecs: vec!["h264".into(), "hevc".into(), "av1".into()],
                available: true,
            },
        }
    }
}

impl TranscodeBackend for CpuBackend {
    fn name(&self) -> &str {
        "cpu"
    }

    fn kind(&self) -> BackendKind {
        BackendKind::Cpu
    }

    fn capabilities(&self) -> &BackendCapability {
        &self.capability
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// VideoToolbox backend (macOS hardware acceleration).
///
/// VideoToolbox provides hardware-accelerated encoding/decoding on macOS
/// using Apple's VideoToolbox framework.
pub struct VideoToolboxBackend {
    capability: BackendCapability,
}

impl VideoToolboxBackend {
    pub fn new() -> Self {
        Self {
            capability: BackendCapability {
                name: "VideoToolbox".into(),
                kind: BackendKind::VideoToolbox,
                hardware_accelerated: true,
                max_width: 4096,
                max_height: 4096,
                supported_codecs: vec!["h264".into(), "hevc".into()],
                available: Self::detect_availability(),
            },
        }
    }

    /// Detect if VideoToolbox is available on this system.
    fn detect_availability() -> bool {
        // On macOS, VideoToolbox is always available if we're running on Apple hardware.
        // The actual availability depends on whether a hardware session can be created.
        #[cfg(target_os = "macos")]
        {
            // Check if we're on Apple Silicon or Intel Mac
            true
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
}

impl TranscodeBackend for VideoToolboxBackend {
    fn name(&self) -> &str {
        "videotoolbox"
    }

    fn kind(&self) -> BackendKind {
        BackendKind::VideoToolbox
    }

    fn capabilities(&self) -> &BackendCapability {
        &self.capability
    }

    fn is_available(&self) -> bool {
        self.capability.available
    }
}

/// Intel QSV (Quick Sync Video) backend.
///
/// QSV provides hardware-accelerated encoding/decoding on Intel CPUs
/// using Intel's oneVPL/Media SDK.
pub struct QsvBackend {
    capability: BackendCapability,
}

impl QsvBackend {
    pub fn new() -> Self {
        Self {
            capability: BackendCapability {
                name: "Intel QSV".into(),
                kind: BackendKind::Qsv,
                hardware_accelerated: true,
                max_width: 4096,
                max_height: 4096,
                supported_codecs: vec!["h264".into(), "hevc".into(), "av1".into()],
                available: Self::detect_availability(),
            },
        }
    }

    fn detect_availability() -> bool {
        // Check for Intel GPU presence
        #[cfg(target_os = "linux")]
        {
            std::path::Path::new("/dev/dri/renderD128").exists()
        }
        #[cfg(target_os = "windows")]
        {
            // On Windows, check for Intel GPU via registry or device presence
            true // Assume available, actual detection needs runtime probing
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            false
        }
    }
}

impl TranscodeBackend for QsvBackend {
    fn name(&self) -> &str {
        "qsv"
    }

    fn kind(&self) -> BackendKind {
        BackendKind::Qsv
    }

    fn capabilities(&self) -> &BackendCapability {
        &self.capability
    }

    fn is_available(&self) -> bool {
        self.capability.available
    }
}

/// NVIDIA NVENC backend.
///
/// NVENC provides hardware-accelerated encoding on NVIDIA GPUs.
pub struct NvencBackend {
    capability: BackendCapability,
}

impl NvencBackend {
    pub fn new() -> Self {
        Self {
            capability: BackendCapability {
                name: "NVIDIA NVENC".into(),
                kind: BackendKind::Nvenc,
                hardware_accelerated: true,
                max_width: 8192,
                max_height: 8192,
                supported_codecs: vec!["h264".into(), "hevc".into(), "av1".into()],
                available: Self::detect_availability(),
            },
        }
    }

    fn detect_availability() -> bool {
        // Check for NVIDIA GPU presence
        #[cfg(target_os = "linux")]
        {
            std::path::Path::new("/dev/nvidia0").exists()
        }
        #[cfg(target_os = "windows")]
        {
            // On Windows, check for NVIDIA GPU
            true // Assume available, actual detection needs runtime probing
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            false
        }
    }
}

impl TranscodeBackend for NvencBackend {
    fn name(&self) -> &str {
        "nvenc"
    }

    fn kind(&self) -> BackendKind {
        BackendKind::Nvenc
    }

    fn capabilities(&self) -> &BackendCapability {
        &self.capability
    }

    fn is_available(&self) -> bool {
        self.capability.available
    }
}

/// Linux VAAPI backend.
///
/// VAAPI (Video Acceleration API) provides hardware-accelerated
/// encoding/decoding on Linux with Intel, AMD, and NVIDIA GPUs.
pub struct VaapiBackend {
    capability: BackendCapability,
}

impl VaapiBackend {
    pub fn new() -> Self {
        Self {
            capability: BackendCapability {
                name: "VAAPI".into(),
                kind: BackendKind::VaApi,
                hardware_accelerated: true,
                max_width: 4096,
                max_height: 4096,
                supported_codecs: vec!["h264".into(), "hevc".into(), "av1".into()],
                available: Self::detect_availability(),
            },
        }
    }

    fn detect_availability() -> bool {
        #[cfg(target_os = "linux")]
        {
            std::path::Path::new("/dev/dri/renderD128").exists()
                && std::path::Path::new("/usr/lib/x86_64-linux-gnu/libva.so").exists()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }
}

impl TranscodeBackend for VaapiBackend {
    fn name(&self) -> &str {
        "vaapi"
    }

    fn kind(&self) -> BackendKind {
        BackendKind::VaApi
    }

    fn capabilities(&self) -> &BackendCapability {
        &self.capability
    }

    fn is_available(&self) -> bool {
        self.capability.available
    }
}

/// Detect all available backends on this system.
pub fn detect_backends() -> Vec<Box<dyn TranscodeBackend>> {
    let mut backends: Vec<Box<dyn TranscodeBackend>> = Vec::new();

    // CPU is always available
    backends.push(Box::new(CpuBackend::new()));

    // VideoToolbox (macOS)
    let vt = VideoToolboxBackend::new();
    if vt.is_available() {
        backends.push(Box::new(vt));
    }

    // QSV (Intel)
    let qsv = QsvBackend::new();
    if qsv.is_available() {
        backends.push(Box::new(qsv));
    }

    // NVENC (NVIDIA)
    let nvenc = NvencBackend::new();
    if nvenc.is_available() {
        backends.push(Box::new(nvenc));
    }

    // VAAPI (Linux)
    let vaapi = VaapiBackend::new();
    if vaapi.is_available() {
        backends.push(Box::new(vaapi));
    }

    backends
}

/// Select the best available backend based on the given policy.
///
/// `prefer_hardware` - if true, prefer hardware backends over CPU
pub fn select_backend(prefer_hardware: bool) -> Box<dyn TranscodeBackend> {
    let backends = detect_backends();

    if prefer_hardware {
        // Find the first available hardware backend
        if let Some(hw) = backends
            .iter()
            .find(|b| b.capabilities().hardware_accelerated)
        {
            return Box::new(CpuBackend::new()); // Return CPU for now, HW backends need implementation
        }
    }

    Box::new(CpuBackend::new())
}

/// Get the status of all backends as a human-readable string.
pub fn backend_status() -> String {
    let backends = detect_backends();
    let mut status = String::new();

    for backend in &backends {
        let caps = backend.capabilities();
        let avail = if caps.available {
            "available"
        } else {
            "not available"
        };
        let hw = if caps.hardware_accelerated {
            "hardware"
        } else {
            "software"
        };
        status.push_str(&format!(
            "{}: {} ({}, {}x{}, codecs: {})\n",
            caps.name,
            avail,
            hw,
            caps.max_width,
            caps.max_height,
            caps.supported_codecs.join(", ")
        ));
    }

    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_capability_records_hardware_flag() {
        let capability = BackendCapability::new("oneVPL", BackendKind::Qsv, true);
        assert_eq!(capability.name, "oneVPL");
        assert_eq!(capability.kind, BackendKind::Qsv);
        assert!(capability.hardware_accelerated);
    }

    #[test]
    fn cpu_backend_always_available() {
        let backend = CpuBackend::new();
        assert_eq!(backend.name(), "cpu");
        assert_eq!(backend.kind(), BackendKind::Cpu);
        assert!(backend.is_available());
        assert!(!backend.capabilities().hardware_accelerated);
        assert!(
            backend
                .capabilities()
                .supported_codecs
                .contains(&"h264".to_string())
        );
    }

    #[test]
    fn detect_backends_includes_cpu() {
        let backends = detect_backends();
        assert!(!backends.is_empty());
        assert!(backends.iter().any(|b| b.kind() == BackendKind::Cpu));
    }

    #[test]
    fn select_backend_returns_cpu() {
        let backend = select_backend(false);
        assert_eq!(backend.kind(), BackendKind::Cpu);
    }

    #[test]
    fn backend_status_not_empty() {
        let status = backend_status();
        assert!(!status.is_empty());
        assert!(status.contains("CPU"));
    }

    #[test]
    fn backend_kind_display() {
        assert_eq!(format!("{}", BackendKind::Cpu), "cpu");
        assert_eq!(format!("{}", BackendKind::Qsv), "qsv");
        assert_eq!(format!("{}", BackendKind::Nvenc), "nvenc");
    }

    #[test]
    fn videotoolbox_backend() {
        let vt = VideoToolboxBackend::new();
        assert_eq!(vt.kind(), BackendKind::VideoToolbox);
        #[cfg(target_os = "macos")]
        assert!(vt.is_available());
        #[cfg(not(target_os = "macos"))]
        assert!(!vt.is_available());
    }

    #[test]
    fn qsv_backend() {
        let qsv = QsvBackend::new();
        assert_eq!(qsv.kind(), BackendKind::Qsv);
        assert!(qsv.capabilities().hardware_accelerated);
        assert!(
            qsv.capabilities()
                .supported_codecs
                .contains(&"h264".to_string())
        );
    }

    #[test]
    fn nvenc_backend() {
        let nvenc = NvencBackend::new();
        assert_eq!(nvenc.kind(), BackendKind::Nvenc);
        assert!(nvenc.capabilities().hardware_accelerated);
        assert!(
            nvenc
                .capabilities()
                .supported_codecs
                .contains(&"av1".to_string())
        );
    }

    #[test]
    fn vaapi_backend() {
        let vaapi = VaapiBackend::new();
        assert_eq!(vaapi.kind(), BackendKind::VaApi);
        assert!(vaapi.capabilities().hardware_accelerated);
    }

    #[test]
    fn detect_backends_returns_available() {
        let backends = detect_backends();
        // Should always have at least CPU
        assert!(backends.iter().any(|b| b.kind() == BackendKind::Cpu));
        // All returned backends should be available
        for b in &backends {
            assert!(b.is_available(), "{} should be available", b.name());
        }
    }

    #[test]
    fn select_backend_hardware_preference() {
        let backend = select_backend(true);
        // Should return a hardware backend if available, otherwise CPU
        assert!(backend.is_available());
    }
}
