use crate::config::CpuPreset;
use crate::media::pipeline::{Encoder, RateControl};

pub fn append_args(
    args: &mut Vec<String>,
    encoder: Encoder,
    rate_control: Option<RateControl>,
    preset: Option<&str>,
) {
    match encoder {
        Encoder::Av1Svt => {
            let crf = match rate_control {
                Some(RateControl::Crf { value }) => value.to_string(),
                _ => CpuPreset::Medium.params().1.to_string(),
            };
            args.extend([
                "-c:v".to_string(),
                "libsvtav1".to_string(),
                "-preset".to_string(),
                preset.unwrap_or(CpuPreset::Medium.params().0).to_string(),
                "-crf".to_string(),
                crf,
            ]);
        }
        Encoder::Av1Aom => {
            let crf = match rate_control {
                Some(RateControl::Crf { value }) => value.to_string(),
                _ => "32".to_string(),
            };
            args.extend([
                "-c:v".to_string(),
                "libaom-av1".to_string(),
                "-crf".to_string(),
                crf,
                "-cpu-used".to_string(),
                preset.unwrap_or("6").to_string(),
            ]);
        }
        Encoder::HevcX265 => {
            let crf = match rate_control {
                Some(RateControl::Crf { value }) => value.to_string(),
                _ => "24".to_string(),
            };
            args.extend([
                "-c:v".to_string(),
                "libx265".to_string(),
                "-preset".to_string(),
                preset.unwrap_or(CpuPreset::Medium.as_str()).to_string(),
                "-crf".to_string(),
                crf,
                "-tag:v".to_string(),
                "hvc1".to_string(),
            ]);
        }
        Encoder::H264X264 => {
            let crf = match rate_control {
                Some(RateControl::Crf { value }) => value.to_string(),
                _ => "21".to_string(),
            };
            args.extend([
                "-c:v".to_string(),
                "libx264".to_string(),
                "-preset".to_string(),
                preset.unwrap_or(CpuPreset::Medium.as_str()).to_string(),
                "-crf".to_string(),
                crf,
            ]);
        }
        _ => {}
    }
}
