use crate::error::Result;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub vendor: Vendor,
    pub device_path: Option<String>,
    pub supported_codecs: Vec<String>,
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

fn check_encoder_support(encoder: &str) -> bool {
    let null_output = if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    };

    // Attempt a tiny 1-frame encode
    let status = Command::new("ffmpeg")
        .args([
            "-v",
            "quiet",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=64x64:d=0.1",
            "-c:v",
            encoder,
            "-frames:v",
            "1",
            "-y",
            null_output,
        ])
        .status();

    match status {
        Ok(s) => s.success(),
        Err(_) => false,
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

fn try_detect_apple() -> Option<HardwareInfo> {
    if !cfg!(target_os = "macos") {
        return None;
    }

    let mut codecs = Vec::new();
    if check_encoder_support("av1_videotoolbox") {
        codecs.push("av1".to_string());
    }
    if check_encoder_support("hevc_videotoolbox") {
        codecs.push("hevc".to_string());
    }
    if check_encoder_support("h264_videotoolbox") {
        codecs.push("h264".to_string());
    }

    Some(HardwareInfo {
        vendor: Vendor::Apple,
        device_path: None,
        supported_codecs: codecs,
    })
}

fn try_detect_nvidia() -> Option<HardwareInfo> {
    let nvidia_likely = if cfg!(target_os = "windows") {
        true
    } else {
        Path::new("/dev/nvidiactl").exists()
    };

    if !nvidia_likely {
        return None;
    }

    let output = Command::new("nvidia-smi").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let mut codecs = Vec::new();
    if check_encoder_support("av1_nvenc") {
        codecs.push("av1".to_string());
    }
    if check_encoder_support("hevc_nvenc") {
        codecs.push("hevc".to_string());
    }
    if check_encoder_support("h264_nvenc") {
        codecs.push("h264".to_string());
    }

    Some(HardwareInfo {
        vendor: Vendor::Nvidia,
        device_path: None,
        supported_codecs: codecs,
    })
}

fn try_detect_intel() -> Option<HardwareInfo> {
    if Path::new("/dev/dri/renderD129").exists() {
        let mut codecs = Vec::new();
        if check_encoder_support("av1_qsv") {
            codecs.push("av1".to_string());
        }
        if check_encoder_support("hevc_qsv") {
            codecs.push("hevc".to_string());
        }
        if check_encoder_support("h264_qsv") {
            codecs.push("h264".to_string());
        }
        return Some(HardwareInfo {
            vendor: Vendor::Intel,
            device_path: Some("/dev/dri/renderD129".to_string()),
            supported_codecs: codecs,
        });
    }

    if !Path::new("/dev/dri/renderD128").exists() {
        return None;
    }

    let vendor_id = std::fs::read_to_string("/sys/class/drm/renderD128/device/vendor")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let looks_intel = vendor_id.contains("0x8086");
    if !looks_intel {
        return None;
    }

    let mut codecs = Vec::new();
    if check_encoder_support("av1_qsv") {
        codecs.push("av1".to_string());
    }
    if check_encoder_support("hevc_qsv") {
        codecs.push("hevc".to_string());
    }
    if check_encoder_support("h264_qsv") {
        codecs.push("h264".to_string());
    }

    Some(HardwareInfo {
        vendor: Vendor::Intel,
        device_path: Some("/dev/dri/renderD128".to_string()),
        supported_codecs: codecs,
    })
}

fn try_detect_amd() -> Option<HardwareInfo> {
    if !Path::new("/dev/dri/renderD128").exists() {
        return None;
    }

    let vendor_id = std::fs::read_to_string("/sys/class/drm/renderD128/device/vendor")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if !vendor_id.contains("0x1002") {
        return None;
    }

    let mut codecs = Vec::new();
    if check_encoder_support("av1_amf") || check_encoder_support("av1_vaapi") {
        codecs.push("av1".to_string());
    }
    if check_encoder_support("hevc_amf") || check_encoder_support("hevc_vaapi") {
        codecs.push("hevc".to_string());
    }
    if check_encoder_support("h264_amf") || check_encoder_support("h264_vaapi") {
        codecs.push("h264".to_string());
    }

    Some(HardwareInfo {
        vendor: Vendor::Amd,
        device_path: Some("/dev/dri/renderD128".to_string()),
        supported_codecs: codecs,
    })
}

fn detect_preferred_hardware(preferred_vendor: Vendor) -> Option<HardwareInfo> {
    match preferred_vendor {
        Vendor::Nvidia => try_detect_nvidia(),
        Vendor::Amd => try_detect_amd(),
        Vendor::Intel => try_detect_intel(),
        Vendor::Apple => try_detect_apple(),
        Vendor::Cpu => Some(HardwareInfo {
            vendor: Vendor::Cpu,
            device_path: None,
            supported_codecs: vec!["av1".to_string(), "hevc".to_string(), "h264".to_string()],
        }),
    }
}

pub fn detect_hardware_with_preference(
    allow_cpu_fallback: bool,
    preferred_vendor: Option<String>,
) -> Result<HardwareInfo> {
    if let Some(preferred_vendor) = preferred_vendor {
        if let Some(parsed_vendor) = parse_preferred_vendor(&preferred_vendor) {
            if parsed_vendor == Vendor::Cpu && !allow_cpu_fallback {
                warn!(
                    "Preferred vendor '{}' requested but CPU fallback is disabled.",
                    preferred_vendor
                );
            } else if let Some(info) = detect_preferred_hardware(parsed_vendor) {
                info!(
                    "✓ Using preferred vendor '{}' ({})",
                    preferred_vendor, info.vendor
                );
                return Ok(info);
            }
            warn!(
                "Preferred vendor '{}' is unavailable. Falling back to auto detection.",
                preferred_vendor
            );
        } else {
            warn!(
                "Unknown preferred vendor '{}'. Falling back to auto detection.",
                preferred_vendor
            );
        }
    }

    detect_hardware(allow_cpu_fallback)
}

pub fn detect_hardware(allow_cpu_fallback: bool) -> Result<HardwareInfo> {
    info!("=== Hardware Detection Starting ===");
    info!("OS: {}", std::env::consts::OS);
    info!("Architecture: {}", std::env::consts::ARCH);

    // 0. Check for Apple (macOS)
    if cfg!(target_os = "macos") {
        info!("✓ Detected macOS platform");
        let mut codecs = Vec::new();
        if check_encoder_support("hevc_videotoolbox") {
            codecs.push("hevc".to_string());
        }
        if check_encoder_support("h264_videotoolbox") {
            codecs.push("h264".to_string());
        }
        // AV1 VideoToolbox support is newer, check for it
        if check_encoder_support("av1_videotoolbox") {
            codecs.push("av1".to_string());
        }

        info!("✓ Hardware acceleration: VideoToolbox (Apple Silicon/Intel)");
        return Ok(HardwareInfo {
            vendor: Vendor::Apple,
            device_path: None,
            supported_codecs: codecs,
        });
    }

    // 1. Check for NVIDIA
    info!("Checking for NVIDIA GPU...");
    // Only check nvidia-smi on non-linux or if /dev/nvidiactl exists on linux
    let nvidia_likely = if cfg!(target_os = "windows") {
        true
    } else {
        Path::new("/dev/nvidiactl").exists()
    };

    if nvidia_likely {
        if let Ok(output) = Command::new("nvidia-smi").output() {
            if output.status.success() {
                info!("✓ nvidia-smi command successful");

                let mut codecs = Vec::new();
                if check_encoder_support("av1_nvenc") {
                    codecs.push("av1".to_string());
                    info!("  ✓ AV1 (av1_nvenc) supported");
                }
                if check_encoder_support("hevc_nvenc") {
                    codecs.push("hevc".to_string());
                    info!("  ✓ HEVC (hevc_nvenc) supported");
                }
                if check_encoder_support("h264_nvenc") {
                    codecs.push("h264".to_string());
                    info!("  ✓ H.264 (h264_nvenc) supported");
                }

                info!("✓ Hardware acceleration: NVENC");
                return Ok(HardwareInfo {
                    vendor: Vendor::Nvidia,
                    device_path: None,
                    supported_codecs: codecs,
                });
            }
        }
    }
    info!("✗ No NVIDIA GPU detected");

    // 2. Check for Intel (Priority on renderD129 for dGPU Arc)
    info!("Checking for Intel GPU...");
    if Path::new("/dev/dri/renderD129").exists() {
        // Linux specific path, mostly.
        let mut codecs = Vec::new();
        // QSV encoders: av1_qsv, hevc_qsv, h264_qsv
        if check_encoder_support("av1_qsv") {
            codecs.push("av1".to_string());
        }
        if check_encoder_support("hevc_qsv") {
            codecs.push("hevc".to_string());
        }

        info!("✓ Found render node: /dev/dri/renderD129");
        info!("✓ Detected Intel Arc/dGPU (discrete graphics)");
        info!("✓ Hardware acceleration: Intel Quick Sync Video (QSV)");
        return Ok(HardwareInfo {
            vendor: Vendor::Intel,
            device_path: Some("/dev/dri/renderD129".to_string()),
            supported_codecs: codecs,
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

        let mut codecs = Vec::new();
        let vendor;
        let accel_name;

        if vendor_id.contains("0x8086") {
            info!("✓ Detected Intel iGPU (integrated graphics)");
            accel_name = "Intel Quick Sync Video (QSV)";
            vendor = Vendor::Intel;
            if check_encoder_support("av1_qsv") {
                codecs.push("av1".to_string());
            }
            if check_encoder_support("hevc_qsv") {
                codecs.push("hevc".to_string());
            }
        } else if vendor_id.contains("0x1002") {
            info!("✓ Detected AMD GPU");
            accel_name = "VAAPI/AMF";
            vendor = Vendor::Amd;
            if check_encoder_support("av1_amf") || check_encoder_support("av1_vaapi") {
                codecs.push("av1".to_string());
            }

            if check_encoder_support("hevc_amf") || check_encoder_support("hevc_vaapi") {
                codecs.push("hevc".to_string());
            }
        } else {
            // Generic fallback
            vendor = Vendor::Intel;
            accel_name = "Generic VAAPI";
            if check_encoder_support("hevc_vaapi") {
                codecs.push("hevc".to_string());
            }
        }

        info!("✓ Hardware acceleration: {}", accel_name);
        return Ok(HardwareInfo {
            vendor,
            device_path: Some("/dev/dri/renderD128".to_string()),
            supported_codecs: codecs, // Populated above
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
        supported_codecs: vec!["av1".to_string(), "hevc".to_string(), "h264".to_string()], // CPU supports all usually via software
    })
}

/// Async version of detect_hardware that doesn't block the runtime
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

pub async fn detect_hardware_for_config(config: &crate::config::Config) -> Result<HardwareInfo> {
    let info = detect_hardware_async_with_preference(
        config.hardware.allow_cpu_fallback,
        config.hardware.preferred_vendor.clone(),
    )
    .await?;

    if info.vendor == Vendor::Cpu && !config.hardware.allow_cpu_encoding {
        return Err(crate::error::AlchemistError::Config(
            "CPU encoding disabled".into(),
        ));
    }

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hardware_state_updates_snapshot() {
        let state = HardwareState::new(Some(HardwareInfo {
            vendor: Vendor::Nvidia,
            device_path: None,
            supported_codecs: vec!["av1".to_string()],
        }));
        assert_eq!(state.snapshot().await.unwrap().vendor, Vendor::Nvidia);

        state
            .replace(Some(HardwareInfo {
                vendor: Vendor::Cpu,
                device_path: None,
                supported_codecs: vec!["av1".to_string(), "hevc".to_string()],
            }))
            .await;

        assert_eq!(state.snapshot().await.unwrap().vendor, Vendor::Cpu);
    }
}
