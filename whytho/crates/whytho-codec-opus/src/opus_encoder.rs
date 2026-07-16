use whytho_types::AudioCodec;

use opus_rs::OpusEncoder;

use super::{AudioEncoder, AudioEncoderConfig};

pub struct WhythoOpusEncoder {
    encoder: Option<OpusEncoder>,
    config: Option<AudioEncoderConfig>,
}

impl WhythoOpusEncoder {
    pub fn new() -> Self {
        Self {
            encoder: None,
            config: None,
        }
    }
}

impl AudioEncoder for WhythoOpusEncoder {
    fn name(&self) -> &str {
        "opus-rs"
    }

    fn codec(&self) -> AudioCodec {
        AudioCodec::Opus
    }

    fn configure(&mut self, config: &AudioEncoderConfig) -> Result<(), String> {
        let app = opus_rs::Application::Audio;

        let encoder = OpusEncoder::new(config.sample_rate as i32, config.channels, app)
            .map_err(|e| format!("opus encoder error: {e}"))?;

        // apply configured bitrate (opus-rs exposes bitrate_bps directly; default is 64000)
        let mut encoder = encoder;
        encoder.bitrate_bps = config.bitrate as i32;

        self.encoder = Some(encoder);
        self.config = Some(config.clone());
        Ok(())
    }

    fn encode(&mut self, samples: &[f32]) -> Result<Vec<Vec<u8>>, String> {
        let encoder = self.encoder.as_mut().ok_or("encoder not configured")?;
        let config = self.config.as_ref().ok_or("encoder not configured")?;
        let frame_size = (config.sample_rate as usize * 20) / 1000;
        let channels = config.channels;
        let samples_per_frame = frame_size * channels;
        let mut output_buf = vec![0u8; 4000];
        let mut packets = Vec::new();

        for chunk in samples.chunks(samples_per_frame) {
            let input = if chunk.len() < samples_per_frame {
                let mut padded = chunk.to_vec();
                padded.resize(samples_per_frame, 0.0);
                padded
            } else {
                chunk.to_vec()
            };

            let n = encoder
                .encode(&input, frame_size, &mut output_buf)
                .map_err(|e| format!("opus encode error: {e}"))?;
            packets.push(output_buf[..n].to_vec());
        }

        Ok(packets)
    }

    fn flush(&mut self) -> Result<Vec<Vec<u8>>, String> {
        Ok(Vec::new())
    }
}
