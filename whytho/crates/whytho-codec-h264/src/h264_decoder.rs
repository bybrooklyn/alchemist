use std::time::Duration;

use rust_h264::decoder::Decoder;
use rust_h264::nal::{parse_annex_b, parse_avcc, parse_avcc_config};

use whytho_types::VideoCodec;

use super::{DecodedFrame, PixelFormat, VideoDecoder};

pub struct H264Decoder {
    decoder: Decoder,
    frame_count: u64,
    fps: f64,
    length_size: usize,
}

impl Default for H264Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl H264Decoder {
    pub fn new() -> Self {
        Self {
            decoder: Decoder::new(),
            frame_count: 0,
            fps: 24.0,
            length_size: 4,
        }
    }

    pub fn with_fps(fps: f64) -> Self {
        Self {
            decoder: Decoder::new(),
            frame_count: 0,
            fps,
            length_size: 4,
        }
    }

    pub fn init_avcc(&mut self, codec_private: &[u8]) -> Result<(), String> {
        let config =
            parse_avcc_config(codec_private).map_err(|e| format!("AVCC config error: {e}"))?;
        self.length_size = config.length_size;

        for sps in &config.sps_nals {
            if let Err(e) = self.decoder.decode_nal(sps) {
                return Err(format!("SPS decode error: {e:?}"));
            }
        }
        for pps in &config.pps_nals {
            if let Err(e) = self.decoder.decode_nal(pps) {
                return Err(format!("PPS decode error: {e:?}"));
            }
        }
        Ok(())
    }

    pub fn decode_sample(&mut self, data: &[u8]) -> Result<Vec<DecodedFrame>, String> {
        let nals = parse_avcc(data, self.length_size);
        let mut frames = Vec::new();
        for nal in &nals {
            if let Some(frame) = self
                .decoder
                .decode_nal(nal)
                .map_err(|e| format!("H.264 decode error: {e:?}"))?
            {
                frames.push(self.convert_frame(frame));
            }
        }
        Ok(frames)
    }

    fn convert_frame(&mut self, frame: rust_h264::decoder::Frame) -> DecodedFrame {
        let pts = Duration::from_secs_f64(self.frame_count as f64 / self.fps);
        self.frame_count += 1;

        DecodedFrame {
            width: frame.width,
            height: frame.height,
            y: frame.y,
            u: frame.u,
            v: frame.v,
            pixel_format: PixelFormat::Yuv420,
            pts,
        }
    }
}

impl VideoDecoder for H264Decoder {
    fn name(&self) -> &str {
        "rust_h264"
    }

    fn codec(&self) -> VideoCodec {
        VideoCodec::H264
    }

    fn decode_nal(&mut self, data: &[u8]) -> Result<Vec<DecodedFrame>, String> {
        let nals = parse_annex_b(data);
        let mut frames = Vec::new();
        for nal in &nals {
            if let Some(frame) = self
                .decoder
                .decode_nal(nal)
                .map_err(|e| format!("H.264 decode error: {e:?}"))?
            {
                frames.push(self.convert_frame(frame));
            }
        }
        Ok(frames)
    }

    fn flush(&mut self) -> Vec<DecodedFrame> {
        let mut frames = Vec::new();
        if let Some(frame) = self.decoder.flush() {
            frames.push(self.convert_frame(frame));
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A slice NAL (nal_unit_type=1) with no SPS/PPS registered first. The inner
    // decoder rejects this with `InvalidSyntax("no PPS available")`; both decode
    // entry points must surface that as `Err` rather than silently reporting zero
    // frames, since callers (whytho-cli verify/executor) treat `Ok` as "decoded
    // successfully" and rely on `Err` to detect a genuinely corrupt/unsupported
    // sample.
    const SLICE_NAL_NO_PARAMS: [u8; 3] = [0x01, 0x00, 0x00];

    #[test]
    fn decode_sample_surfaces_decode_errors() {
        let mut dec = H264Decoder::new();
        let mut data = Vec::new();
        data.extend_from_slice(&(SLICE_NAL_NO_PARAMS.len() as u32).to_be_bytes());
        data.extend_from_slice(&SLICE_NAL_NO_PARAMS);

        let result = dec.decode_sample(&data);
        assert!(
            result.is_err(),
            "decode_sample must return Err for a slice with no SPS/PPS, not silently drop it"
        );
    }

    #[test]
    fn video_decoder_decode_nal_surfaces_decode_errors() {
        let mut dec = H264Decoder::new();
        let mut data = vec![0x00, 0x00, 0x00, 0x01]; // Annex B start code
        data.extend_from_slice(&SLICE_NAL_NO_PARAMS);

        let result = VideoDecoder::decode_nal(&mut dec, &data);
        assert!(
            result.is_err(),
            "VideoDecoder::decode_nal must return Err for a slice with no SPS/PPS, not silently drop it"
        );
    }
}
