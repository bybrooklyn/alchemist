use crate::media::pipeline::Encoder;

pub fn apply(cmd: &mut tokio::process::Command, encoder: Encoder) {
    match encoder {
        Encoder::Av1Amf => {
            cmd.arg("-c:v").arg("av1_amf");
        }
        Encoder::HevcAmf => {
            cmd.arg("-c:v").arg("hevc_amf");
        }
        Encoder::H264Amf => {
            cmd.arg("-c:v").arg("h264_amf");
        }
        _ => {}
    }
}
