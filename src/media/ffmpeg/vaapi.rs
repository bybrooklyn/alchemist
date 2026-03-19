use crate::media::pipeline::Encoder;
use crate::system::hardware::HardwareInfo;

pub fn append_args(args: &mut Vec<String>, encoder: Encoder, hw_info: Option<&HardwareInfo>) {
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
}
