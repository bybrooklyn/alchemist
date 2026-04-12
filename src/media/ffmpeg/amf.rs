use crate::media::pipeline::{Encoder, RateControl};

pub fn append_args(args: &mut Vec<String>, encoder: Encoder, rate_control: Option<&RateControl>) {
    // AMF quality: CQP mode uses -rc cqp with -qp_i and -qp_p.
    // The config uses CQ-style semantics (lower value = better quality).
    let (use_cqp, qp_value) = match rate_control {
        Some(RateControl::Cq { value }) => (true, *value),
        _ => (false, 25),
    };

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

    if use_cqp {
        args.extend([
            "-rc".to_string(),
            "cqp".to_string(),
            "-qp_i".to_string(),
            qp_value.to_string(),
            "-qp_p".to_string(),
            qp_value.to_string(),
        ]);
    }
}
