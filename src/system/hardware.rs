use crate::error::Result;
use std::path::Path;
use std::process::Command;
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

fn check_encoder_support(encoder: &str) -> bool {
    let null_output = if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    };

    // Attempt a tiny 1-frame encode
    let status = Command::new("ffmpeg")
        .args(&[
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
            if check_encoder_support("av1_amf") {
                codecs.push("av1".to_string());
            } else if check_encoder_support("av1_vaapi") {
                codecs.push("av1".to_string());
            }

            if check_encoder_support("hevc_amf") {
                codecs.push("hevc".to_string());
            } else if check_encoder_support("hevc_vaapi") {
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
