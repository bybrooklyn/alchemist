use crate::media::pipeline::{Encoder, RateControl};

pub fn append_args(
    args: &mut Vec<String>,
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
            args.extend([
                "-c:v".to_string(),
                "av1_videotoolbox".to_string(),
                "-b:v".to_string(),
                "0".to_string(),
                "-q:v".to_string(),
                cq.to_string(),
                "-allow_sw".to_string(),
                "1".to_string(),
            ]);
        }
        Encoder::HevcVideotoolbox => {
            args.extend([
                "-c:v".to_string(),
                "hevc_videotoolbox".to_string(),
                "-b:v".to_string(),
                "0".to_string(),
                "-q:v".to_string(),
                cq.to_string(),
                "-tag:v".to_string(),
                "hvc1".to_string(),
                "-allow_sw".to_string(),
                "1".to_string(),
            ]);
        }
        Encoder::H264Videotoolbox => {
            args.extend([
                "-c:v".to_string(),
                "h264_videotoolbox".to_string(),
                "-b:v".to_string(),
                "0".to_string(),
                "-q:v".to_string(),
                cq.to_string(),
                "-allow_sw".to_string(),
                "1".to_string(),
            ]);
        }
        _ => {}
    }
}
