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
            // VideoToolbox constant quality. The config supplies a CRF-style value
            // (lower = better quality); clamp it to the 1-51 band we use for
            // x264/x265 so profiles stay comparable across encoders. Constant
            // quality only opens on a real hardware VideoToolbox session — when no
            // session is available the encoder fails to open and the pipeline
            // falls back to CPU (see errors#encoder_open_failed).
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
