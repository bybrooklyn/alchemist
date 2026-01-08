use crate::error::Result;
use std::path::Path;
use std::process::Command;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Cpu, // Software fallback
}

impl std::fmt::Display for Vendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Vendor::Nvidia => write!(f, "NVIDIA (NVENC)"),
            Vendor::Amd => write!(f, "AMD (VAAPI/AMF)"),
            Vendor::Intel => write!(f, "Intel (QSV)"),
            Vendor::Apple => write!(f, "Apple (VideoToolbox)"),
            Vendor::Cpu => write!(f, "CPU (Software Encoding)"),
        }
    }
}

pub struct HardwareInfo {
    pub vendor: Vendor,
    pub device_path: Option<String>,
}

pub fn detect_hardware(allow_cpu_fallback: bool) -> Result<HardwareInfo> {
    info!("=== Hardware Detection Starting ===");
    info!("OS: {}", std::env::consts::OS);
    info!("Architecture: {}", std::env::consts::ARCH);

    // 0. Check for Apple (macOS)
    if cfg!(target_os = "macos") {
        info!("✓ Detected macOS platform");
        info!("✓ Hardware acceleration: VideoToolbox (Apple Silicon/Intel)");
        return Ok(HardwareInfo {
            vendor: Vendor::Apple,
            device_path: None,
        });
    }

    // 1. Check for NVIDIA (Simplest check via nvidia-smi or /dev/nvidiactl)
    info!("Checking for NVIDIA GPU...");
    if Path::new("/dev/nvidiactl").exists() {
        info!("✓ Found NVIDIA device: /dev/nvidiactl");
        if let Ok(output) = Command::new("nvidia-smi").output() {
            if output.status.success() {
                info!("✓ nvidia-smi command successful");
                info!("✓ Hardware acceleration: NVENC");
                return Ok(HardwareInfo {
                    vendor: Vendor::Nvidia,
                    device_path: None,
                });
            }
        }
    } else if Command::new("nvidia-smi").output().is_ok() {
        info!("✓ nvidia-smi available");
        info!("✓ Hardware acceleration: NVENC");
        return Ok(HardwareInfo {
            vendor: Vendor::Nvidia,
            device_path: None,
        });
    }
    info!("✗ No NVIDIA GPU detected");

    // 2. Check for Intel (Priority on renderD129 for dGPU Arc)
    info!("Checking for Intel GPU...");
    if Path::new("/dev/dri/renderD129").exists() {
        info!("✓ Found render node: /dev/dri/renderD129");
        info!("✓ Detected Intel Arc/dGPU (discrete graphics)");
        info!("✓ Hardware acceleration: Intel Quick Sync Video (QSV)");
        return Ok(HardwareInfo {
            vendor: Vendor::Intel,
            device_path: Some("/dev/dri/renderD129".to_string()),
        });
    }
    info!("✗ No Intel dGPU at renderD129");

    // 3. Check for Intel iGPU or general Intel/AMD via renderD128
    info!("Checking for integrated GPU at renderD128...");
    if Path::new("/dev/dri/renderD128").exists() {
        info!("✓ Found render node: /dev/dri/renderD128");

        // Try to disambiguate via /sys/class/drm/renderD128/device/vendor
        let vendor_path = "/sys/class/drm/renderD128/device/vendor";
        let vendor_id = std::fs::read_to_string(vendor_path)
            .unwrap_or_default()
            .trim()
            .to_lowercase();

        info!(
            "  Vendor ID from sysfs: {}",
            if vendor_id.is_empty() {
                "unknown"
            } else {
                &vendor_id
            }
        );

        if vendor_id.contains("0x8086") {
            info!("✓ Detected Intel iGPU (integrated graphics)");
            info!("✓ Hardware acceleration: Intel Quick Sync Video (QSV)");
            return Ok(HardwareInfo {
                vendor: Vendor::Intel,
                device_path: Some("/dev/dri/renderD128".to_string()),
            });
        } else if vendor_id.contains("0x1002") {
            info!("✓ Detected AMD GPU");
            info!("✓ Hardware acceleration: VAAPI/AMF");
            return Ok(HardwareInfo {
                vendor: Vendor::Amd,
                device_path: Some("/dev/dri/renderD128".to_string()),
            });
        }

        // Fallback for VAAPI if we can't be sure but the node exists
        warn!(
            "Found /dev/dri/renderD128 but couldn't verify vendor ID ({}). Assuming generic VAAPI.",
            vendor_id
        );
        info!("✓ Hardware acceleration: Generic VAAPI");
        return Ok(HardwareInfo {
            vendor: Vendor::Intel, // Assume Intel for VAAPI
            device_path: Some("/dev/dri/renderD128".to_string()),
        });
    }
    info!("✗ No GPU render nodes found");

    // 4. CPU Fallback
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

    Ok(HardwareInfo {
        vendor: Vendor::Cpu,
        device_path: None,
    })
}
