use crate::media::pipeline::Encoder;

pub fn append_args(args: &mut Vec<String>, encoder: Encoder, tag_hevc_as_hvc1: bool) {
    // Current FFmpeg VideoToolbox encoders on macOS do not expose qscale-style
    // quality controls, so bitrate mode is handled by the shared builder and
    // CQ-style requests intentionally fall back to the encoder defaults.
    match encoder {
        Encoder::Av1Videotoolbox => {
            args.extend([
                "-c:v".to_string(),
                "av1_videotoolbox".to_string(),
                "-allow_sw".to_string(),
                "1".to_string(),
            ]);
        }
        Encoder::HevcVideotoolbox => {
            args.extend(["-c:v".to_string(), "hevc_videotoolbox".to_string()]);
            if tag_hevc_as_hvc1 {
                args.extend(["-tag:v".to_string(), "hvc1".to_string()]);
            }
            args.extend(["-allow_sw".to_string(), "1".to_string()]);
        }
        Encoder::H264Videotoolbox => {
            args.extend([
                "-c:v".to_string(),
                "h264_videotoolbox".to_string(),
                "-allow_sw".to_string(),
                "1".to_string(),
            ]);
        }
        _ => {}
    }
}
