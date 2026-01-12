use crate::config::OutputCodec;
use crate::system::hardware::{HardwareInfo, Vendor};
use serde::Serialize;
use tracing::warn;

const DEFAULT_ALEMBIC_INGEST_URL: &str = "http://localhost:3000/v1/event";

#[derive(Debug, Serialize)]
pub struct TelemetryEvent {
    pub app_version: String,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_factor: Option<f64>,
}

pub fn hardware_label(hw: Option<&HardwareInfo>) -> Option<String> {
    let hw = hw?;
    let label = match hw.vendor {
        Vendor::Nvidia => "Nvidia",
        Vendor::Amd => "AMD",
        Vendor::Intel => "Intel",
        Vendor::Apple => "Apple",
        Vendor::Cpu => "CPU",
    };
    Some(label.to_string())
}

pub fn encoder_label(hw: Option<&HardwareInfo>, codec: OutputCodec) -> String {
    let cpu_encoder = match codec {
        OutputCodec::Av1 => "libsvtav1",
        OutputCodec::Hevc => "libx265",
    };

    let Some(hw) = hw else {
        return cpu_encoder.to_string();
    };

    let codec_str = codec.as_str();
    let supports_codec = hw.supported_codecs.iter().any(|c| c == codec_str);
    if !supports_codec {
        return cpu_encoder.to_string();
    }

    match (hw.vendor, codec) {
        (Vendor::Intel, OutputCodec::Av1) => "av1_qsv".to_string(),
        (Vendor::Intel, OutputCodec::Hevc) => "hevc_qsv".to_string(),
        (Vendor::Nvidia, OutputCodec::Av1) => "av1_nvenc".to_string(),
        (Vendor::Nvidia, OutputCodec::Hevc) => "hevc_nvenc".to_string(),
        (Vendor::Apple, OutputCodec::Av1) => "av1_videotoolbox".to_string(),
        (Vendor::Apple, OutputCodec::Hevc) => "hevc_videotoolbox".to_string(),
        (Vendor::Amd, OutputCodec::Av1) => {
            if cfg!(target_os = "windows") {
                "av1_amf".to_string()
            } else {
                "av1_vaapi".to_string()
            }
        }
        (Vendor::Amd, OutputCodec::Hevc) => {
            if cfg!(target_os = "windows") {
                "hevc_amf".to_string()
            } else {
                "hevc_vaapi".to_string()
            }
        }
        (Vendor::Cpu, _) => cpu_encoder.to_string(),
    }
}

pub fn resolution_bucket(width: u32, height: u32) -> Option<String> {
    let pixel_height = if height > 0 { height } else { width };
    if pixel_height == 0 {
        return None;
    }

    let bucket = if pixel_height >= 2160 {
        "2160p"
    } else if pixel_height >= 1440 {
        "1440p"
    } else if pixel_height >= 1080 {
        "1080p"
    } else if pixel_height >= 720 {
        "720p"
    } else if pixel_height >= 480 {
        "480p"
    } else {
        return Some(format!("{}p", pixel_height));
    };
    Some(bucket.to_string())
}

pub async fn send_event(event: TelemetryEvent) {
    let endpoint =
        std::env::var("ALEMBIC_INGEST_URL").unwrap_or_else(|_| DEFAULT_ALEMBIC_INGEST_URL.into());

    let client = reqwest::Client::new();
    match client.post(&endpoint).json(&event).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                warn!(
                    "Telemetry ingest failed with status {} from {}",
                    resp.status(),
                    endpoint
                );
            }
        }
        Err(e) => {
            warn!("Telemetry ingest error to {}: {}", endpoint, e);
        }
    }
}
