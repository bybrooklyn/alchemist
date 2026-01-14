use crate::config::CpuPreset;
use crate::media::pipeline::{Encoder, RateControl};

pub fn apply(
    cmd: &mut tokio::process::Command,
    encoder: Encoder,
    cpu_preset: CpuPreset,
    rate_control: Option<RateControl>,
) {
    match encoder {
        Encoder::Av1Svt => {
            let (preset_str, crf_str) = cpu_preset.params();
            let crf = match rate_control {
                Some(RateControl::Crf { value }) => value.to_string(),
                _ => crf_str.to_string(),
            };
            cmd.arg("-c:v").arg("libsvtav1");
            cmd.arg("-preset").arg(preset_str);
            cmd.arg("-crf").arg(crf);
        }
        Encoder::Av1Aom => {
            let crf = match rate_control {
                Some(RateControl::Crf { value }) => value.to_string(),
                _ => "32".to_string(),
            };
            cmd.arg("-c:v").arg("libaom-av1");
            cmd.arg("-crf").arg(crf);
            cmd.arg("-cpu-used").arg("6");
        }
        Encoder::HevcX265 => {
            let preset = cpu_preset.as_str();
            let default_crf = match cpu_preset {
                CpuPreset::Slow => "20",
                CpuPreset::Medium => "24",
                CpuPreset::Fast => "26",
                CpuPreset::Faster => "28",
            };
            let crf = match rate_control {
                Some(RateControl::Crf { value }) => value.to_string(),
                _ => default_crf.to_string(),
            };
            cmd.arg("-c:v").arg("libx265");
            cmd.arg("-preset").arg(preset);
            cmd.arg("-crf").arg(crf);
            cmd.arg("-tag:v").arg("hvc1");
        }
        Encoder::H264X264 => {
            let preset = cpu_preset.as_str();
            let default_crf = match cpu_preset {
                CpuPreset::Slow => "18",
                CpuPreset::Medium => "21",
                CpuPreset::Fast => "23",
                CpuPreset::Faster => "25",
            };
            let crf = match rate_control {
                Some(RateControl::Crf { value }) => value.to_string(),
                _ => default_crf.to_string(),
            };
            cmd.arg("-c:v").arg("libx264");
            cmd.arg("-preset").arg(preset);
            cmd.arg("-crf").arg(crf);
        }
        _ => {}
    }
}
