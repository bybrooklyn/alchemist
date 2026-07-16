//! In-house AV1 decoder.
//!
//! Pure-Rust implementation of the AV1 bitstream decoding process.
//! Current status: scaffold — OBU parsing and sequence header decode.

pub mod block;
pub mod cdef;
pub mod cdf;
pub mod deblock;
pub mod frame_header;
pub mod inter;
pub mod intra;
pub mod loop_rest;
pub mod obu;
pub mod ref_frames;
pub mod sequence;
pub mod simple_decode;
pub mod symbol;
pub mod tile_group;
pub mod transform;

use whytho_types::VideoCodec;

use crate::{DecodedFrame, VideoDecoder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Av1Profile {
    Main,
    High,
    Professional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Av1Level {
    L2_0,
    L2_1,
    L2_2,
    L2_3,
    L3_0,
    L3_1,
    L3_2,
    L3_3,
    L4_0,
    L4_1,
    L4_2,
    L4_3,
    L5_0,
    L5_1,
    L5_2,
    L5_3,
    L6_0,
    L6_1,
    L6_2,
    L6_3,
    L7_0,
    L7_1,
    L7_2,
    L7_3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    KeyFrame,
    InterFrame,
    IntraOnlyFrame,
    SwitchFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxType {
    DctDct,
    AdstAdst,
    DctAdst,
    AdstDct,
    IdentityIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictionMode {
    DcPred,
    VPred,
    HPred,
    D45Pred,
    D135Pred,
    D113Pred,
    D157Pred,
    D203Pred,
    D67Pred,
    SmoothPred,
    SmoothVPred,
    SmoothHPred,
    PaethPred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxSize {
    Tx4x4,
    Tx8x8,
    Tx16x16,
    Tx32x32,
    Tx64x64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSize {
    Bs4x4,
    Bs4x8,
    Bs8x4,
    Bs8x8,
    Bs8x16,
    Bs16x8,
    Bs16x16,
    Bs16x32,
    Bs32x16,
    Bs32x32,
    Bs32x64,
    Bs64x32,
    Bs64x64,
    Bs128x128,
    Bs128x64,
    Bs64x128,
}

#[derive(Debug, Clone)]
pub struct SequenceHeader {
    pub seq_profile: Av1Profile,
    pub still_picture: bool,
    pub reduced_still_picture_header: bool,
    pub max_frame_width: u32,
    pub max_frame_height: u32,
    pub frame_id_numbers_present: bool,
    pub delta_frame_id_length: u32,
    pub additional_frame_id_length: u32,
    pub seq_force_integer_mv: bool,
    pub seq_force_screen_content_tools: bool,
    pub enable_superres: bool,
    pub enable_cdef: bool,
    pub enable_restoration: bool,
    pub color_config: ColorConfig,
    pub film_grain_params_present: bool,
}

#[derive(Debug, Clone)]
pub struct ColorConfig {
    pub bit_depth: u8,
    pub subsampling_x: bool,
    pub subsampling_y: bool,
    pub mono_chrome: bool,
    pub color_primaries: u8,
    pub transfer_characteristics: u8,
    pub matrix_coefficients: u8,
    pub color_range: bool,
    pub chroma_sample_position: u8,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            bit_depth: 8,
            subsampling_x: true,
            subsampling_y: true,
            mono_chrome: false,
            color_primaries: 1,          // BT.709
            transfer_characteristics: 1, // BT.709
            matrix_coefficients: 1,      // BT.709
            color_range: false,          // limited
            chroma_sample_position: 0,   // unknown
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameHeader {
    pub frame_type: FrameType,
    pub show_existing_frame: bool,
    pub frame_to_show: u32,
    pub show_frame: bool,
    pub showable_frame: bool,
    pub error_resilient_mode: bool,
    pub width: u32,
    pub height: u32,
    pub render_width: u32,
    pub render_height: u32,
    pub superres_denom: u32,
    pub upscaled_width: u32,
    pub use_superres: bool,
    pub frame_offset: u32,
    pub quantization_params: QuantizationParams,
    pub segmentation_params: SegmentationParams,
    pub delta_q_params: DeltaQParams,
    pub loop_filter_params: LoopFilterParams,
    pub cdef_params: CdefParams,
    pub lr_params: LrParams,
    pub tile_info: TileInfo,
}

#[derive(Debug, Clone)]
pub struct QuantizationParams {
    pub base_q_idx: u8,
    pub delta_q_y_dc: i8,
    pub delta_q_u_dc: i8,
    pub delta_q_u_ac: i8,
    pub delta_q_v_dc: i8,
    pub delta_q_v_ac: i8,
    pub using_qmatrix: bool,
    pub qm_y: u8,
    pub qm_u: u8,
    pub qm_v: u8,
}

impl Default for QuantizationParams {
    fn default() -> Self {
        Self {
            base_q_idx: 0,
            delta_q_y_dc: 0,
            delta_q_u_dc: 0,
            delta_q_u_ac: 0,
            delta_q_v_dc: 0,
            delta_q_v_ac: 0,
            using_qmatrix: false,
            qm_y: 0,
            qm_u: 0,
            qm_v: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SegmentationParams {
    pub segmentation_enabled: bool,
    pub segmentation_update_map: bool,
    pub segmentation_temporal_update: bool,
    pub segmentation_update_data: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DeltaQParams {
    pub delta_q_present: bool,
    pub delta_q_res: u8,
}

#[derive(Debug, Clone)]
pub struct LoopFilterParams {
    pub loop_filter_level_0: u8,
    pub loop_filter_level_1: u8,
    pub loop_filter_ref_deltas: [i8; 8],
    pub loop_filter_mode_deltas: [i8; 2],
    pub loop_filter_sharpness: u8,
}

impl Default for LoopFilterParams {
    fn default() -> Self {
        Self {
            loop_filter_level_0: 0,
            loop_filter_level_1: 0,
            loop_filter_ref_deltas: [1, 0, 0, 0, -1, 0, -1, -1],
            loop_filter_mode_deltas: [0, 0],
            loop_filter_sharpness: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CdefParams {
    pub cdef_damping: u8,
    pub cdef_bits: u8,
    pub cdef_y_strength: [u8; 8],
    pub cdef_uv_strength: [u8; 8],
}

impl Default for CdefParams {
    fn default() -> Self {
        Self {
            cdef_damping: 3,
            cdef_bits: 0,
            cdef_y_strength: [0; 8],
            cdef_uv_strength: [0; 8],
        }
    }
}

#[derive(Debug, Clone)]
pub struct LrParams {
    pub lr_type: [u8; 3], // 0=none, 1=wiener, 2=sgproj
    pub lr_unit_shift: u8,
}

impl Default for LrParams {
    fn default() -> Self {
        Self {
            lr_type: [0; 3],
            lr_unit_shift: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TileInfo {
    pub tile_cols_log2: u8,
    pub tile_rows_log2: u8,
}

impl Default for TileInfo {
    fn default() -> Self {
        Self {
            tile_cols_log2: 0,
            tile_rows_log2: 0,
        }
    }
}

#[allow(dead_code)]
pub struct Av1Decoder {
    seq_header: Option<SequenceHeader>,
    frame_count: u64,
    reference_frames: Vec<Option<DecodedFrame>>,
}

impl Av1Decoder {
    pub fn new() -> Self {
        Self {
            seq_header: None,
            frame_count: 0,
            reference_frames: vec![None; 8],
        }
    }

    pub fn sequence_header(&self) -> Option<&SequenceHeader> {
        self.seq_header.as_ref()
    }
}

impl VideoDecoder for Av1Decoder {
    fn name(&self) -> &str {
        "whytho-av1"
    }

    fn codec(&self) -> VideoCodec {
        VideoCodec::Av1
    }

    fn decode_nal(&mut self, data: &[u8]) -> Result<Vec<DecodedFrame>, String> {
        let obus = obu::parse_obus(data)?;

        let mut frames = Vec::new();

        for obu in &obus {
            match obu.obu_type {
                obu::OBUType::SequenceHeader => {
                    let sh = sequence::decode_sequence_header(&obu.payload)?;
                    self.seq_header = Some(sh);
                }
                obu::OBUType::Frame => {
                    let sh = self
                        .seq_header
                        .as_ref()
                        .ok_or("no sequence header decoded yet")?;
                    let fh = frame_header::decode_frame_header(&obu.payload, sh)?;

                    if fh.show_frame {
                        let width = fh.width;
                        let height = fh.height;

                        // Use the simple I-frame decoder for actual decoding
                        let mut decoder = simple_decode::SimpleIFrameDecoder::new(width, height);

                        // Extract tile data from the OBU payload
                        // For Frame OBUs, tile data follows the frame header
                        // We need to find where the frame header ends and tile data begins
                        // For now, use a simplified approach: decode with DC prediction
                        let tile_data = &obu.payload[obu.payload.len().min(20)..]; // skip header bytes
                        let _ = decoder.decode_simple(tile_data, &fh, sh);

                        let frame = decoder.into_frame();
                        frames.push(DecodedFrame {
                            width,
                            height,
                            y: frame.y,
                            u: frame.u,
                            v: frame.v,
                            pixel_format: whytho_types::PixelFormat::Yuv420,
                            pts: std::time::Duration::from_secs_f64(self.frame_count as f64 / 24.0),
                        });
                        self.frame_count += 1;
                    }
                }
                obu::OBUType::TileGroup => {
                    // Tile group OBU: contains tile data for a previously parsed frame header
                    // For now, skip tile data parsing
                }
                _ => {} // Skip other OBU types for now
            }
        }

        Ok(frames)
    }

    fn flush(&mut self) -> Vec<DecodedFrame> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn av1_decoder_new() {
        let dec = Av1Decoder::new();
        assert_eq!(dec.name(), "whytho-av1");
        assert_eq!(dec.codec(), VideoCodec::Av1);
        assert!(dec.sequence_header().is_none());
    }
}
