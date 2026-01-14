use crate::media::pipeline::{Encoder, RateControl};
use crate::system::hardware::HardwareInfo;

pub fn apply(
    cmd: &mut tokio::process::Command,
    encoder: Encoder,
    hw_info: Option<&HardwareInfo>,
    rate_control: Option<RateControl>,
    default_quality: u8,
) {
    if let Some(hw) = hw_info {
        if let Some(ref device_path) = hw.device_path {
            cmd.arg("-init_hw_device")
                .arg(format!("qsv=qsv:{}", device_path));
            cmd.arg("-filter_hw_device").arg("qsv");
        }
    }

    let quality = match rate_control {
        Some(RateControl::QsvQuality { value }) => value,
        _ => default_quality,
    };

    match encoder {
        Encoder::Av1Qsv => {
            cmd.arg("-c:v").arg("av1_qsv");
            cmd.arg("-global_quality").arg(quality.to_string());
            cmd.arg("-look_ahead").arg("1");
        }
        Encoder::HevcQsv => {
            cmd.arg("-c:v").arg("hevc_qsv");
            cmd.arg("-global_quality").arg(quality.to_string());
            cmd.arg("-look_ahead").arg("1");
        }
        Encoder::H264Qsv => {
            cmd.arg("-c:v").arg("h264_qsv");
            cmd.arg("-global_quality").arg(quality.to_string());
            cmd.arg("-look_ahead").arg("1");
        }
        _ => {}
    }
}
