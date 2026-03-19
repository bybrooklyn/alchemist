use crate::media::pipeline::{Encoder, RateControl};
use crate::system::hardware::HardwareInfo;

pub fn append_args(
    args: &mut Vec<String>,
    encoder: Encoder,
    hw_info: Option<&HardwareInfo>,
    rate_control: Option<RateControl>,
    default_quality: u8,
) {
    if let Some(hw) = hw_info {
        if let Some(ref device_path) = hw.device_path {
            args.extend([
                "-init_hw_device".to_string(),
                format!("qsv=qsv:{device_path}"),
                "-filter_hw_device".to_string(),
                "qsv".to_string(),
            ]);
        }
    }

    let quality = match rate_control {
        Some(RateControl::QsvQuality { value }) => value,
        _ => default_quality,
    };

    match encoder {
        Encoder::Av1Qsv => {
            args.extend([
                "-c:v".to_string(),
                "av1_qsv".to_string(),
                "-global_quality".to_string(),
                quality.to_string(),
                "-look_ahead".to_string(),
                "1".to_string(),
            ]);
        }
        Encoder::HevcQsv => {
            args.extend([
                "-c:v".to_string(),
                "hevc_qsv".to_string(),
                "-global_quality".to_string(),
                quality.to_string(),
                "-look_ahead".to_string(),
                "1".to_string(),
            ]);
        }
        Encoder::H264Qsv => {
            args.extend([
                "-c:v".to_string(),
                "h264_qsv".to_string(),
                "-global_quality".to_string(),
                quality.to_string(),
                "-look_ahead".to_string(),
                "1".to_string(),
            ]);
        }
        _ => {}
    }
}
