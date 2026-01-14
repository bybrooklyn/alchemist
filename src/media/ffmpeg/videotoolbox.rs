use crate::media::pipeline::{Encoder, RateControl};

pub fn apply(
    cmd: &mut tokio::process::Command,
    encoder: Encoder,
    rate_control: Option<RateControl>,
    default_quality: u8,
) {
    let cq = match rate_control {
        Some(RateControl::Cq { value }) => value,
        _ => default_quality,
    };

    match encoder {
        Encoder::Av1Videotoolbox => {
            cmd.arg("-c:v").arg("av1_videotoolbox");
            cmd.arg("-b:v").arg("0");
            cmd.arg("-q:v").arg(cq.to_string());
        }
        Encoder::HevcVideotoolbox => {
            cmd.arg("-c:v").arg("hevc_videotoolbox");
            cmd.arg("-b:v").arg("0");
            cmd.arg("-q:v").arg(cq.to_string());
            cmd.arg("-tag:v").arg("hvc1");
        }
        Encoder::H264Videotoolbox => {
            cmd.arg("-c:v").arg("h264_videotoolbox");
            cmd.arg("-b:v").arg("0");
            cmd.arg("-q:v").arg(cq.to_string());
        }
        _ => {}
    }
}
