use crate::media::pipeline::Encoder;

pub fn append_args(args: &mut Vec<String>, encoder: Encoder) {
    match encoder {
        Encoder::Av1Amf => {
            args.extend(["-c:v".to_string(), "av1_amf".to_string()]);
        }
        Encoder::HevcAmf => {
            args.extend(["-c:v".to_string(), "hevc_amf".to_string()]);
        }
        Encoder::H264Amf => {
            args.extend(["-c:v".to_string(), "h264_amf".to_string()]);
        }
        _ => {}
    }
}
