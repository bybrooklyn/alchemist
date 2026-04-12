use crate::media::pipeline::{Encoder, RateControl};

pub fn append_args(
    args: &mut Vec<String>,
    encoder: Encoder,
    rate_control: Option<RateControl>,
    preset: Option<&str>,
) {
    let cq = match rate_control {
        Some(RateControl::Cq { value }) => value,
        _ => 25,
    };
    let preset = preset.unwrap_or("p4").to_string();

    match encoder {
        Encoder::Av1Nvenc => {
            args.extend([
                "-c:v".to_string(),
                "av1_nvenc".to_string(),
                "-preset".to_string(),
                preset.clone(),
                "-rc".to_string(),
                "vbr".to_string(),
                "-cq".to_string(),
                cq.to_string(),
            ]);
        }
        Encoder::HevcNvenc => {
            args.extend([
                "-c:v".to_string(),
                "hevc_nvenc".to_string(),
                "-preset".to_string(),
                preset.clone(),
                "-rc".to_string(),
                "vbr".to_string(),
                "-cq".to_string(),
                cq.to_string(),
            ]);
        }
        Encoder::H264Nvenc => {
            args.extend([
                "-c:v".to_string(),
                "h264_nvenc".to_string(),
                "-preset".to_string(),
                preset,
                "-rc".to_string(),
                "vbr".to_string(),
                "-cq".to_string(),
                cq.to_string(),
            ]);
        }
        _ => {}
    }
}
