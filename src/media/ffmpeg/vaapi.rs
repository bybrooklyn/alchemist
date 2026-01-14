use crate::media::pipeline::Encoder;
use crate::system::hardware::HardwareInfo;

pub fn apply(
    cmd: &mut tokio::process::Command,
    encoder: Encoder,
    hw_info: Option<&HardwareInfo>,
) {
    if let Some(hw) = hw_info {
        if let Some(ref device_path) = hw.device_path {
            cmd.arg("-vaapi_device").arg(device_path);
        }
    }

    match encoder {
        Encoder::Av1Vaapi => {
            cmd.arg("-c:v").arg("av1_vaapi");
        }
        Encoder::HevcVaapi => {
            cmd.arg("-c:v").arg("hevc_vaapi");
        }
        Encoder::H264Vaapi => {
            cmd.arg("-c:v").arg("h264_vaapi");
        }
        _ => {}
    }
}
