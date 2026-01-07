use crate::error::{AlchemistError, Result};
use std::path::Path;
#[cfg(feature = "ssr")]
use std::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
}

impl std::fmt::Display for Vendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Vendor::Nvidia => write!(f, "NVIDIA (NVENC)"),
            Vendor::Amd => write!(f, "AMD (VAAPI/AMF)"),
            Vendor::Intel => write!(f, "Intel (QSV)"),
            Vendor::Apple => write!(f, "Apple (VideoToolbox)"),
        }
    }
}

pub struct HardwareInfo {
    pub vendor: Vendor,
    pub device_path: Option<String>,
}

#[cfg(feature = "ssr")]
pub fn detect_hardware() -> Result<HardwareInfo> {
    // 0. Check for Apple (macOS)
    if cfg!(target_os = "macos") {
        info!("Detected macOS hardware");
        return Ok(HardwareInfo {
            vendor: Vendor::Apple,
            device_path: None,
        });
    }

    // 1. Check for NVIDIA (Simplest check via nvidia-smi or /dev/nvidiactl)
    if Path::new("/dev/nvidiactl").exists() || Command::new("nvidia-smi").output().is_ok() {
        info!("Detected NVIDIA hardware");
        return Ok(HardwareInfo {
            vendor: Vendor::Nvidia,
            device_path: None, // NVENC usually doesn't need a specific DRI path in FFmpeg
        });
    }

    // 2. Check for Intel (Priority on renderD129 for dGPU Arc)
    if Path::new("/dev/dri/renderD129").exists() {
        info!("Detected Intel Arc/dGPU at /dev/dri/renderD129");
        return Ok(HardwareInfo {
            vendor: Vendor::Intel,
            device_path: Some("/dev/dri/renderD129".to_string()),
        });
    }

    // 3. Check for Intel iGPU or general Intel/AMD via renderD128
    if Path::new("/dev/dri/renderD128").exists() {
        // We can try to disambiguate via /sys/class/drm/renderD128/device/vendor
        // Intel: 0x8086, AMD: 0x1002
        let vendor_id = std::fs::read_to_string("/sys/class/drm/renderD128/device/vendor")
            .unwrap_or_default()
            .trim()
            .to_lowercase();

        if vendor_id.contains("0x8086") {
            info!("Detected Intel iGPU at /dev/dri/renderD128");
            return Ok(HardwareInfo {
                vendor: Vendor::Intel,
                device_path: Some("/dev/dri/renderD128".to_string()),
            });
        } else if vendor_id.contains("0x1002") {
            info!("Detected AMD GPU at /dev/dri/renderD128");
            return Ok(HardwareInfo {
                vendor: Vendor::Amd,
                device_path: Some("/dev/dri/renderD128".to_string()),
            });
        }

        // Fallback for VAAPI if we can't be sure but the node exists
        warn!(
            "Found /dev/dri/renderD128 but couldn't verify vendor id ({}). Assuming VAAPI compatible.",
            vendor_id
        );
    }

    Err(AlchemistError::Hardware("No supported hardware accelerator (NVIDIA, AMD, or Intel) found. Alchemist requires hardware acceleration.".into()))
}
