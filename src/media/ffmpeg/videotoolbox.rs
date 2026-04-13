use crate::media::pipeline::{Encoder, RateControl};

pub fn append_args(
    args: &mut Vec<String>,
    encoder: Encoder,
    tag_hevc_as_hvc1: bool,
    rate_control: Option<&RateControl>,
) {
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

    match rate_control {
        Some(RateControl::Cq { value }) => {
            // VideoToolbox -q:v: 1 (best) to 100 (worst). Config value is CRF-style
            // where lower = better quality. Clamp to 1-51 range matching x264/x265.
            let q = (*value).clamp(1, 51);
            args.extend(["-q:v".to_string(), q.to_string()]);
        }
        Some(RateControl::Bitrate { kbps, .. }) => {
            args.extend([
                "-b:v".to_string(),
                format!("{}k", kbps),
                "-maxrate".to_string(),
                format!("{}k", kbps * 2),
                "-bufsize".to_string(),
                format!("{}k", kbps * 4),
            ]);
        }
        _ => {
            // Default: constant quality at 28 (HEVC-equivalent mid quality)
            args.extend(["-q:v".to_string(), "28".to_string()]);
        }
    }
}
