//! In-house H.264/AVC encoder.
//!
//! Pure-Rust implementation targeting Baseline and Main profiles.
//! Current status: scaffold — I-frame encoding with CAVLC.

pub mod cavlc;
pub mod deblock;
pub mod dpb;
pub mod intra;
pub mod me;
pub mod nal;
pub mod pps;
pub mod quantize;
pub mod rate_control;
pub mod slice;
pub mod sps;
pub mod transform;

use std::time::Duration;

use whytho_types::VideoCodec;

use crate::{DecodedFrame, EncodedPacket, VideoEncoder, VideoEncoderConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Profile {
    Baseline,
    Main,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Level {
    L1_0,
    L1_1,
    L1_2,
    L1_3,
    L2_0,
    L2_1,
    L2_2,
    L3_0,
    L3_1,
    L3_2,
    L4_0,
    L4_1,
    L4_2,
    L5_0,
    L5_1,
    L5_2,
}

impl H264Level {
    pub fn level_idc(self) -> u8 {
        match self {
            Self::L1_0 => 10,
            Self::L1_1 => 11,
            Self::L1_2 => 12,
            Self::L1_3 => 13,
            Self::L2_0 => 20,
            Self::L2_1 => 21,
            Self::L2_2 => 22,
            Self::L3_0 => 30,
            Self::L3_1 => 31,
            Self::L3_2 => 32,
            Self::L4_0 => 40,
            Self::L4_1 => 41,
            Self::L4_2 => 42,
            Self::L5_0 => 50,
            Self::L5_1 => 51,
            Self::L5_2 => 52,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MbType {
    I4x4,
    I16x16(Intra16x16Mode, usize, usize),
    I8x8,
    IPcm,
    PSkip,
    P16x16,
    P16x8,
    P8x16,
    P8x8,
    P8x8Ref0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intra4x4Mode {
    Vertical,
    Horizontal,
    Dc,
    DiagonalDownLeft,
    DiagonalDownRight,
    VerticalRight,
    HorizontalDown,
    VerticalLeft,
    HorizontalUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intra16x16Mode {
    Vertical,
    Horizontal,
    Dc,
    Plane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntraChromaMode {
    Dc,
    Horizontal,
    Vertical,
    Plane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Macroblock {
    pub mb_type: MbType,
    pub mb_x: u32,
    pub mb_y: u32,
    pub qp: i8,
    pub y: [[u8; 16]; 16],
    pub cb: [[u8; 8]; 8],
    pub cr: [[u8; 8]; 8],
}

#[derive(Debug, Clone)]
pub struct H264EncoderConfig {
    pub profile: H264Profile,
    pub level: H264Level,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub bitrate: u32,
    pub keyframe_interval: u64,
    pub qp: i8,
}

impl Default for H264EncoderConfig {
    fn default() -> Self {
        Self {
            profile: H264Profile::Baseline,
            level: H264Level::L3_1,
            width: 1920,
            height: 1080,
            fps: 24.0,
            bitrate: 2_000_000,
            keyframe_interval: 240,
            qp: 26,
        }
    }
}

impl H264EncoderConfig {
    /// Picture size in macroblocks (16x16 luma each), rounding up for non-16-multiple
    /// dimensions (the SPS signals frame cropping to trim the padding — see `sps.rs`).
    pub fn mb_dims(&self) -> (u32, u32) {
        (self.width.div_ceil(16), self.height.div_ceil(16))
    }
}

pub struct H264Encoder {
    config: Option<H264EncoderConfig>,
    frame_count: u64,
    frame_num: u32,
    bitstream: BitstreamWriter,
    dpb: dpb::Dpb,
    rate_ctrl: Option<rate_control::RateController>,
}

impl Default for H264Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl H264Encoder {
    pub fn new() -> Self {
        Self {
            config: None,
            frame_count: 0,
            frame_num: 0,
            bitstream: BitstreamWriter::new(),
            dpb: dpb::Dpb::new(1),
            rate_ctrl: None,
        }
    }
}

impl VideoEncoder for H264Encoder {
    fn name(&self) -> &str {
        "whytho-h264"
    }

    fn codec(&self) -> VideoCodec {
        VideoCodec::H264
    }

    fn configure(&mut self, config: &VideoEncoderConfig) -> Result<(), String> {
        if config.width == 0 || config.height == 0 {
            return Err("invalid dimensions".into());
        }
        if config.width % 2 != 0 || config.height % 2 != 0 {
            return Err("dimensions must be even (4:2:0 requirement)".into());
        }
        let initial_qp =
            rate_control::initial_qp_for_bitrate(config.bitrate, config.width, config.height);
        self.config = Some(H264EncoderConfig {
            width: config.width,
            height: config.height,
            fps: config.fps,
            bitrate: config.bitrate,
            keyframe_interval: config.keyframe_interval,
            qp: initial_qp,
            ..H264EncoderConfig::default()
        });
        self.rate_ctrl = Some(rate_control::RateController::new(
            config.bitrate,
            config.fps,
            initial_qp,
        ));
        self.frame_count = 0;
        self.frame_num = 0;
        self.dpb.clear();
        Ok(())
    }

    fn encode(&mut self, frame: &DecodedFrame) -> Result<Vec<EncodedPacket>, String> {
        let mut cfg = self
            .config
            .as_ref()
            .ok_or("encoder not configured")?
            .clone();

        // Use rate controller QP if available
        if let Some(ref rc) = self.rate_ctrl {
            cfg.qp = rc.qp();
        }

        let is_keyframe = self.frame_count % cfg.keyframe_interval == 0;
        self.bitstream.clear();

        if is_keyframe {
            self.dpb.clear();
            self.frame_num = 0;
            sps::write_sps(&mut self.bitstream, &cfg)?;
            pps::write_pps(&mut self.bitstream, &cfg)?;
        }

        let ref_frame = self.dpb.last_ref();

        match (is_keyframe, ref_frame) {
            (false, Some(_)) if self.dpb.count() >= 2 && self.frame_count % 3 == 2 => {
                // B-slice: needs two references (L0 = past, L1 = future)
                // For now, use the two most recent references
                let ref_l0 = self.dpb.get_by_index(1).unwrap(); // older
                let ref_l1 = self.dpb.get_by_index(0).unwrap(); // newer
                slice::write_b_slice(
                    &mut self.bitstream,
                    &cfg,
                    frame,
                    ref_l0,
                    ref_l1,
                    self.frame_num,
                )?;
            }
            (false, Some(ref_frame)) => {
                // P-slice
                slice::write_p_slice(&mut self.bitstream, &cfg, frame, ref_frame, self.frame_num)?;
            }
            _ => {
                // I-slice (keyframe, or no reference available yet)
                slice::write_slice(&mut self.bitstream, &cfg, frame, is_keyframe)?;
            }
        }

        let recon = dpb::ReferenceFrame::from_data(
            self.frame_num,
            cfg.width,
            cfg.height,
            &frame.y,
            &frame.u,
            &frame.v,
        );
        self.dpb.store(recon);

        let data = self.bitstream.take_bytes();

        // Update rate controller with actual frame size
        if let Some(ref mut rc) = self.rate_ctrl {
            rc.update(data.len() as u32 * 8); // convert bytes to bits
        }

        let pts = Duration::from_secs_f64(self.frame_count as f64 / cfg.fps);
        self.frame_count += 1;
        self.frame_num += 1;

        Ok(vec![EncodedPacket {
            data,
            pts,
            is_keyframe,
        }])
    }

    fn flush(&mut self) -> Result<Vec<EncodedPacket>, String> {
        Ok(Vec::new())
    }
}

pub struct BitstreamWriter {
    buffer: Vec<u8>,
    current_byte: u8,
    bit_pos: u8,
}

impl Default for BitstreamWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl BitstreamWriter {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            current_byte: 0,
            bit_pos: 0,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.current_byte = 0;
        self.bit_pos = 0;
    }

    pub fn write_bits(&mut self, value: u32, num_bits: u8) {
        for i in (0..num_bits).rev() {
            let bit = (value >> i) & 1;
            self.current_byte = (self.current_byte << 1) | bit as u8;
            self.bit_pos += 1;
            if self.bit_pos == 8 {
                self.buffer.push(self.current_byte);
                self.current_byte = 0;
                self.bit_pos = 0;
            }
        }
    }

    pub fn write_ue(&mut self, value: u32) {
        let value = value + 1;
        let num_bits = 32 - value.leading_zeros() as u8;
        self.write_bits(0, num_bits - 1);
        self.write_bits(value, num_bits);
    }

    pub fn write_se(&mut self, value: i32) {
        let unsigned = if value > 0 {
            (value as u32) * 2 - 1
        } else {
            (-value as u32) * 2
        };
        self.write_ue(unsigned);
    }

    pub fn write_bytes(&mut self, data: &[u8]) {
        self.align();
        self.buffer.extend_from_slice(data);
    }

    pub fn align(&mut self) {
        if self.bit_pos > 0 {
            self.current_byte <<= 8 - self.bit_pos;
            self.buffer.push(self.current_byte);
            self.current_byte = 0;
            self.bit_pos = 0;
        }
    }

    pub fn take_bytes(&mut self) -> Vec<u8> {
        self.align();
        std::mem::take(&mut self.buffer)
    }

    /// Take bytes with RBSP trailing bits appended.
    pub fn take_rbsp_bytes(&mut self) -> Vec<u8> {
        self.write_rbsp_trailing_bits();
        self.align();
        std::mem::take(&mut self.buffer)
    }

    /// Write RBSP trailing bits: one 1-bit followed by zero bits to byte-align.
    pub fn write_rbsp_trailing_bits(&mut self) {
        self.write_bits(1, 1);
        if self.bit_pos > 0 {
            self.write_bits(0, 8 - self.bit_pos);
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len() + if self.bit_pos > 0 { 1 } else { 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty() && self.bit_pos == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitstream_writer_basic() {
        let mut w = BitstreamWriter::new();
        w.write_bits(0xFF, 8);
        let bytes = w.take_bytes();
        assert_eq!(bytes, vec![0xFF]);
    }

    #[test]
    fn bitstream_writer_ue() {
        let mut w = BitstreamWriter::new();
        w.write_ue(0);
        let bytes = w.take_bytes();
        assert_eq!(bytes, vec![0x80]);
    }

    #[test]
    fn bitstream_writer_se_positive() {
        let mut w = BitstreamWriter::new();
        w.write_se(1);
        let bytes = w.take_bytes();
        // se(1) -> unsigned 1 -> ue(1) -> value=2, num_bits=2
        // bits: 0, 1, 0 -> 0b01000000 = 0x40
        assert_eq!(bytes, vec![0x40]);
    }

    #[test]
    fn bitstream_writer_se_negative() {
        let mut w = BitstreamWriter::new();
        w.write_se(-1);
        let bytes = w.take_bytes();
        // se(-1) -> unsigned 2 -> ue(2) -> value=3, num_bits=2
        // bits: 0, 1, 1 -> 0b01100000 = 0x60
        assert_eq!(bytes, vec![0x60]);
    }

    #[test]
    fn bitstream_writer_align() {
        let mut w = BitstreamWriter::new();
        w.write_bits(1, 1);
        w.align();
        assert_eq!(w.take_bytes(), vec![0x80]);
    }

    #[test]
    fn h264_encoder_debug_bitstream() {
        use super::nal;
        use super::pps;
        use super::sps;

        let config = H264EncoderConfig {
            width: 16,
            height: 16,
            ..Default::default()
        };

        // Hand-craft a minimal valid H.264 bitstream
        let mut manual = BitstreamWriter::new();
        sps::write_sps(&mut manual, &config).unwrap();
        pps::write_pps(&mut manual, &config).unwrap();

        let mut slice = BitstreamWriter::new();
        slice.write_ue(0); // first_mb_in_slice
        slice.write_ue(2); // slice_type = I
        slice.write_ue(0); // pic_parameter_set_id
        slice.write_bits(0, 4); // frame_num
        slice.write_ue(0); // idr_pic_id
        slice.write_bits(0, 4); // pic_order_cnt_lsb
        slice.write_se(0); // slice_qp_delta
        slice.write_ue(1); // disable_deblocking_filter_idc
        slice.write_ue(1); // mb_type = I16x16 DC, no residual

        let slice_bytes = slice.take_rbsp_bytes();
        nal::write_nal_with_emulation_prevention(&mut manual, nal::NAL_TYPE_IDR, 3, &slice_bytes);
        let manual_bytes = manual.take_bytes();
        eprintln!(
            "manual: {} bytes: {:02x?}",
            manual_bytes.len(),
            &manual_bytes[..40.min(manual_bytes.len())]
        );

        // Encode with the encoder
        let mut enc = H264Encoder::new();
        enc.configure(&VideoEncoderConfig {
            width: 16,
            height: 16,
            fps: 24.0,
            ..Default::default()
        })
        .unwrap();

        let frame = DecodedFrame {
            width: 16,
            height: 16,
            y: vec![128u8; 256],
            u: vec![128u8; 64],
            v: vec![128u8; 64],
            pixel_format: crate::PixelFormat::Yuv420,
            pts: Duration::ZERO,
        };
        let packets = enc.encode(&frame).unwrap();
        eprintln!(
            "encoder: {} bytes: {:02x?}",
            packets[0].data.len(),
            &packets[0].data[..40.min(packets[0].data.len())]
        );

        // Try decoding both
        use crate::VideoDecoder;
        use crate::h264_decoder::H264Decoder;

        let mut dec1 = H264Decoder::new();
        let mut frames1 = dec1.decode_nal(&manual_bytes).unwrap_or_default();
        frames1.extend(dec1.flush());
        eprintln!("manual decoded: {} frames", frames1.len());

        let mut dec2 = H264Decoder::new();
        let mut frames2 = dec2.decode_nal(&packets[0].data).unwrap_or_default();
        frames2.extend(dec2.flush());
        eprintln!("encoder decoded: {} frames", frames2.len());
    }

    #[test]
    fn h264_encoder_rejects_odd_dimensions() {
        let mut enc = H264Encoder::new();
        let config = VideoEncoderConfig {
            width: 641,
            height: 480,
            ..Default::default()
        };
        assert!(enc.configure(&config).is_err());
    }

    #[test]
    fn h264_encoder_rejects_unconfigured() {
        let mut enc = H264Encoder::new();
        let frame = DecodedFrame {
            width: 16,
            height: 16,
            y: vec![0u8; 256],
            u: vec![0u8; 64],
            v: vec![0u8; 64],
            pixel_format: crate::PixelFormat::Yuv420,
            pts: Duration::ZERO,
        };
        assert!(enc.encode(&frame).is_err());
    }

    #[test]
    fn h264_encoder_produces_decodable_output() {
        use crate::VideoDecoder;
        use crate::h264_decoder::H264Decoder;

        let mut enc = H264Encoder::new();
        let config = VideoEncoderConfig {
            width: 16,
            height: 16,
            fps: 24.0,
            ..Default::default()
        };
        enc.configure(&config).unwrap();

        let frame = DecodedFrame {
            width: 16,
            height: 16,
            y: vec![128u8; 256],
            u: vec![128u8; 64],
            v: vec![128u8; 64],
            pixel_format: crate::PixelFormat::Yuv420,
            pts: Duration::ZERO,
        };

        let packets = enc.encode(&frame).unwrap();
        assert!(!packets.is_empty());
        assert!(packets[0].is_keyframe);
        assert!(packets[0].data.len() > 10);

        let mut dec = H264Decoder::new();
        let mut all_frames = Vec::new();
        for pkt in &packets {
            match dec.decode_nal(&pkt.data) {
                Ok(frames) => all_frames.extend(frames),
                Err(e) => panic!("decode_nal error: {}", e),
            }
        }
        all_frames.extend(dec.flush());
        assert_eq!(all_frames.len(), 1);
        assert_eq!(all_frames[0].width, 16);
        assert_eq!(all_frames[0].height, 16);
    }

    #[test]
    fn h264_encoder_320x240_decodable() {
        let mut enc = H264Encoder::new();
        let config = VideoEncoderConfig {
            width: 320,
            height: 240,
            fps: 24.0,
            ..Default::default()
        };
        enc.configure(&config).unwrap();

        let frame = DecodedFrame {
            width: 320,
            height: 240,
            y: vec![128u8; 320 * 240],
            u: vec![128u8; 160 * 120],
            v: vec![128u8; 160 * 120],
            pixel_format: crate::PixelFormat::Yuv420,
            pts: Duration::ZERO,
        };

        let packets = enc.encode(&frame).unwrap();
        assert!(packets[0].data.len() > 10);
        assert!(packets[0].is_keyframe);
    }

    #[test]
    fn h264_encoder_roundtrip_gradient() {
        use crate::VideoDecoder;
        use crate::h264_decoder::H264Decoder;

        let width = 64u32;
        let height = 64u32;

        let mut enc = H264Encoder::new();
        let config = VideoEncoderConfig {
            width,
            height,
            fps: 24.0,
            ..Default::default()
        };
        enc.configure(&config).unwrap();

        let mut y = vec![0u8; (width * height) as usize];
        for row in 0..height {
            for col in 0..width {
                let idx = (row * width + col) as usize;
                y[idx] = ((row * 4 + col * 2) % 256) as u8;
            }
        }
        let u = vec![128u8; ((width / 2) * (height / 2)) as usize];
        let v = vec![128u8; ((width / 2) * (height / 2)) as usize];

        let frame = DecodedFrame {
            width,
            height,
            y: y.clone(),
            u,
            v,
            pixel_format: crate::PixelFormat::Yuv420,
            pts: Duration::ZERO,
        };

        let packets = enc.encode(&frame).unwrap();
        assert!(packets[0].is_keyframe);
        assert!(
            packets[0].data.len() > 100,
            "gradient frame should produce non-trivial bitstream"
        );

        let mut dec = H264Decoder::new();
        let mut all_frames = Vec::new();
        for pkt in &packets {
            match dec.decode_nal(&pkt.data) {
                Ok(frames) => {
                    eprintln!("decode_nal returned {} frames", frames.len());
                    all_frames.extend(frames);
                }
                Err(e) => panic!("decode_nal error: {:?}", e),
            }
        }
        eprintln!("flushing...");
        let flushed = dec.flush();
        eprintln!("flush returned {} frames", flushed.len());
        all_frames.extend(flushed);
        assert_eq!(
            all_frames.len(),
            1,
            "gradient frame must decode to exactly 1 frame, got {}",
            all_frames.len()
        );
        assert_eq!(all_frames[0].width, width);
        assert_eq!(all_frames[0].height, height);
    }
}
