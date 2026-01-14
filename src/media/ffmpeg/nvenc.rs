use crate::media::pipeline::{Encoder, RateControl};

pub fn apply(
    cmd: &mut tokio::process::Command,
    encoder: Encoder,
    rate_control: Option<RateControl>,
    preset: &str,
) {
    let cq = match rate_control {
        Some(RateControl::Cq { value }) => value,
        _ => 25,
    };

    match encoder {
        Encoder::Av1Nvenc => {
            cmd.arg("-c:v").arg("av1_nvenc");
            cmd.arg("-preset").arg(preset);
            cmd.arg("-cq").arg(cq.to_string());
        }
        Encoder::HevcNvenc => {
            cmd.arg("-c:v").arg("hevc_nvenc");
            cmd.arg("-preset").arg(preset);
            cmd.arg("-cq").arg(cq.to_string());
        }
        Encoder::H264Nvenc => {
            cmd.arg("-c:v").arg("h264_nvenc");
            cmd.arg("-preset").arg(preset);
            cmd.arg("-cq").arg(cq.to_string());
        }
        _ => {}
    }
}
