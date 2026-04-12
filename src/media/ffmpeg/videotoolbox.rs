use crate::media::pipeline::{Encoder, RateControl};

pub fn append_args(
    args: &mut Vec<String>,
    encoder: Encoder,
    tag_hevc_as_hvc1: bool,
    rate_control: Option<&RateControl>,
) {
    // VideoToolbox quality is controlled via -global_quality (0–100, 100=best).
    // The config uses CQ-style semantics where lower value = better quality,
    // so we invert: global_quality = 100 - cq_value.
    // Bitrate mode is handled by the shared builder in mod.rs.
    match encoder {
        Encoder::Av1Videotoolbox => {
            args.extend(["-c:v".to_string(), "av1_videotoolbox".to_string()]);
        }
        Encoder::HevcVideotoolbox => {
            args.extend(["-c:v".to_string(), "hevc_videotoolbox".to_string()]);
            if tag_hevc_as_hvc1 {
                args.extend(["-tag:v".to_string(), "hvc1".to_string()]);
            }
        }
        Encoder::H264Videotoolbox => {
            args.extend(["-c:v".to_string(), "h264_videotoolbox".to_string()]);
        }
        _ => {}
    }
    if let Some(RateControl::Cq { value }) = rate_control {
        let global_quality = 100u8.saturating_sub(*value);
        args.extend(["-global_quality".to_string(), global_quality.to_string()]);
    }
}
