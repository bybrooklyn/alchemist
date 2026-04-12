use crate::media::pipeline::{Encoder, RateControl};
use crate::system::hardware::HardwareInfo;

pub fn append_args(
    args: &mut Vec<String>,
    encoder: Encoder,
    hw_info: Option<&HardwareInfo>,
    rate_control: Option<&RateControl>,
) {
    if let Some(hw) = hw_info {
        if let Some(ref device_path) = hw.device_path {
            args.extend(["-vaapi_device".to_string(), device_path.to_string()]);
        }
    }

    match encoder {
        Encoder::Av1Vaapi => {
            args.extend(["-c:v".to_string(), "av1_vaapi".to_string()]);
        }
        Encoder::HevcVaapi => {
            args.extend(["-c:v".to_string(), "hevc_vaapi".to_string()]);
        }
        Encoder::H264Vaapi => {
            args.extend(["-c:v".to_string(), "h264_vaapi".to_string()]);
        }
        _ => {}
    }

    // VAAPI quality is set via -global_quality (0–100, higher = better).
    // The config uses CQ-style semantics where lower value = better quality,
    // so we invert: global_quality = 100 - cq_value.
    if let Some(RateControl::Cq { value }) = rate_control {
        let global_quality = 100u8.saturating_sub(*value);
        args.extend(["-global_quality".to_string(), global_quality.to_string()]);
    }
}
