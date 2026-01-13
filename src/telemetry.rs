use crate::config::OutputCodec;
use crate::system::hardware::{HardwareInfo, Vendor};
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::warn;

const DEFAULT_ALEMBIC_INGEST_URL: &str = "https://alembic.alchemist-project.org/v1/event";
const TELEMETRY_TIMEOUT_SECS: u64 = 4;
const TELEMETRY_MAX_RETRIES: usize = 2;
const TELEMETRY_BACKOFF_MS: [u64; TELEMETRY_MAX_RETRIES] = [200, 800];

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
        OutputCodec::H264 => "libx264",
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
        (Vendor::Intel, OutputCodec::H264) => "h264_qsv".to_string(),
        (Vendor::Nvidia, OutputCodec::Av1) => "av1_nvenc".to_string(),
        (Vendor::Nvidia, OutputCodec::Hevc) => "hevc_nvenc".to_string(),
        (Vendor::Nvidia, OutputCodec::H264) => "h264_nvenc".to_string(),
        (Vendor::Apple, OutputCodec::Av1) => "av1_videotoolbox".to_string(),
        (Vendor::Apple, OutputCodec::Hevc) => "hevc_videotoolbox".to_string(),
        (Vendor::Apple, OutputCodec::H264) => "h264_videotoolbox".to_string(),
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
        (Vendor::Amd, OutputCodec::H264) => {
            if cfg!(target_os = "windows") {
                "h264_amf".to_string()
            } else {
                "h264_vaapi".to_string()
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

fn telemetry_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(TELEMETRY_TIMEOUT_SECS))
            .user_agent(format!("alchemist/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

fn sanitize_speed(speed: Option<f64>) -> Option<f64> {
    speed.filter(|value| value.is_finite())
}

fn should_retry_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

pub async fn send_event(event: TelemetryEvent) {
    let endpoint =
        std::env::var("ALEMBIC_INGEST_URL").unwrap_or_else(|_| DEFAULT_ALEMBIC_INGEST_URL.into());

    let mut event = event;
    event.speed_factor = sanitize_speed(event.speed_factor);

    let client = telemetry_client();
    let backoff_iter = TELEMETRY_BACKOFF_MS
        .iter()
        .copied()
        .chain(std::iter::once(
            *TELEMETRY_BACKOFF_MS.last().unwrap_or(&0),
        ))
        .enumerate()
        .take(TELEMETRY_MAX_RETRIES + 1);
    for (attempt, backoff_ms) in backoff_iter {
        match client.post(&endpoint).json(&event).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    return;
                }
                if !should_retry_status(resp.status()) {
                    warn!(
                        "Telemetry ingest failed with status {} from {}",
                        resp.status(),
                        endpoint
                    );
                    return;
                }
                if attempt == TELEMETRY_MAX_RETRIES {
                    warn!(
                        "Telemetry ingest failed after retries with status {} from {}",
                        resp.status(),
                        endpoint
                    );
                    return;
                }
            }
            Err(e) => {
                if attempt == TELEMETRY_MAX_RETRIES {
                    warn!("Telemetry ingest error to {}: {}", endpoint, e);
                    return;
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
    }
}
