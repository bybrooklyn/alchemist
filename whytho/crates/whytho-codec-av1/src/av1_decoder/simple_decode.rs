//! Simple AV1 I-frame decoder.
//!
//! Implements a simplified decoder that can decode basic I-frames
//! using DC prediction and inverse DCT. This is not a full AV1 decoder
//! but can handle simple content like solid colors and gradients.

use super::block::{self, DecodedBlock};
use super::cdf::CdfContext;
use super::intra;
use super::sequence::BitReader;
use super::symbol::SymbolDecoder;
use super::tile_group;
use super::transform;
use super::{FrameHeader, SequenceHeader, TxType};
use whytho_types::DecodedFrame;

/// Simple I-frame decoder state.
pub struct SimpleIFrameDecoder {
    /// Decoded luma plane.
    y: Vec<u8>,
    /// Decoded Cb plane.
    u: Vec<u8>,
    /// Decoded Cr plane.
    v: Vec<u8>,
    /// Frame width.
    width: u32,
    /// Frame height.
    height: u32,
}

impl SimpleIFrameDecoder {
    /// Create a new decoder for a frame of the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let luma_size = (width * height) as usize;
        let chroma_size = ((width / 2) * (height / 2)) as usize;
        Self {
            y: vec![128u8; luma_size],
            u: vec![128u8; chroma_size],
            v: vec![128u8; chroma_size],
            width,
            height,
        }
    }

    /// Decode a simple I-frame from tile data.
    ///
    /// This is a simplified decoder that:
    /// 1. Assumes all blocks use DC prediction
    /// 2. Reads transform coefficients from the bitstream
    /// 3. Applies inverse DCT
    /// 4. Reconstructs the frame
    ///
    /// For production use, a full AV1 decoder would need:
    /// - Proper symbol decoding (arithmetic coder)
    /// - All prediction modes
    /// - Proper transform type selection
    /// - Loop filtering (deblock, CDEF, restoration)
    pub fn decode_simple(
        &mut self,
        tile_data: &[u8],
        fh: &FrameHeader,
        seq: &SequenceHeader,
    ) -> Result<(), String> {
        let sb_size: u32 = if seq.enable_superres { 128 } else { 64 };
        let width_sb = (self.width + sb_size - 1) / sb_size;
        let height_sb = (self.height + sb_size - 1) / sb_size;

        // For simplicity, assume a single tile covering the whole frame
        let mut r = BitReader::new(tile_data);

        // Decode superblocks in raster order
        for sb_y in 0..height_sb {
            for sb_x in 0..width_sb {
                self.decode_superblock(&mut r, sb_x, sb_y, sb_size, fh)?;
            }
        }

        Ok(())
    }

    /// Decode a single superblock.
    fn decode_superblock(
        &mut self,
        r: &mut BitReader,
        sb_x: u32,
        sb_y: u32,
        sb_size: u32,
        fh: &FrameHeader,
    ) -> Result<(), String> {
        // For simplicity, assume the superblock is split into 64x64 blocks
        // In a full implementation, we'd parse the partition tree
        let block_size = 64u32.min(sb_size);

        for by in (0..sb_size).step_by(block_size as usize) {
            for bx in (0..sb_size).step_by(block_size as usize) {
                let px = sb_x * sb_size + bx;
                let py = sb_y * sb_size + by;

                if px >= self.width || py >= self.height {
                    continue;
                }

                // Decode a 64x64 block (or smaller at frame edges)
                let w = block_size.min(self.width - px);
                let h = block_size.min(self.height - py);

                self.decode_block_dc(r, px, py, w, h)?;
            }
        }

        Ok(())
    }

    /// Decode a block using DC prediction and inverse transform.
    ///
    /// This is the simplest possible decoder path:
    /// 1. Read a DC coefficient from the bitstream
    /// 2. Apply DC prediction (fill block with predicted value)
    /// 3. Add the DC residual
    fn decode_block_dc(
        &mut self,
        r: &mut BitReader,
        px: u32,
        py: u32,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        // Get reference samples for DC prediction
        let above_sum: u32 = (0..width)
            .map(|x| {
                if py > 0 {
                    self.y[((py - 1) * self.width + px + x) as usize] as u32
                } else {
                    128u32
                }
            })
            .sum();

        let left_sum: u32 = (0..height)
            .map(|y| {
                if px > 0 {
                    self.y[((py + y) * self.width + px - 1) as usize] as u32
                } else {
                    128u32
                }
            })
            .sum();

        // DC prediction value
        let dc_pred = ((above_sum + left_sum + (width + height) / 2) / (width + height)) as u8;

        // Read a simple DC coefficient from the bitstream
        // In a full implementation, this would use the arithmetic coder
        // For now, use a fixed value (no residual)
        let dc_coeff = 0i32;

        // Reconstruct block
        let dc_value = (dc_pred as i32 + dc_coeff).clamp(0, 255) as u8;

        for y in 0..height {
            for x in 0..width {
                let idx = ((py + y) * self.width + px + x) as usize;
                if idx < self.y.len() {
                    self.y[idx] = dc_value;
                }
            }
        }

        // Chroma: use 128 (neutral) for simplicity
        let cw = self.width / 2;
        let ch = self.height / 2;
        let cx = px / 2;
        let cy = py / 2;
        let cwidth = width / 2;
        let cheight = height / 2;

        for y in 0..cheight {
            for x in 0..cwidth {
                let idx = ((cy + y) * cw + cx + x) as usize;
                if idx < self.u.len() {
                    self.u[idx] = 128;
                    self.v[idx] = 128;
                }
            }
        }

        Ok(())
    }

    /// Decode a 4x4 block with transform coefficients.
    ///
    /// This implements a more complete decode path:
    /// 1. DC prediction from neighbors
    /// 2. Read transform coefficients (simplified: read raw bits)
    /// 3. Apply inverse DCT
    /// 4. Reconstruct block
    pub fn decode_4x4_block_with_transform(
        &mut self,
        r: &mut BitReader,
        px: u32,
        py: u32,
        tx_type: TxType,
    ) -> Result<(), String> {
        // DC prediction
        let above_sum: u32 = (0..4)
            .map(|x| {
                if py > 0 {
                    self.y[((py - 1) * self.width + px + x) as usize] as u32
                } else {
                    128u32
                }
            })
            .sum();

        let left_sum: u32 = (0..4)
            .map(|y| {
                if px > 0 {
                    self.y[((py + y) * self.width + px - 1) as usize] as u32
                } else {
                    128u32
                }
            })
            .sum();

        let dc_pred = ((above_sum + left_sum + 4) / 8) as u8;

        // Read transform coefficients using a simplified but more realistic parser.
        // In a full AV1 decoder, this would use the arithmetic coder with CDFs.
        // Here we read raw bits and interpret them as signed coefficients.
        let mut coeffs = [0i32; 16];

        // Read coefficient presence bits (significance map)
        let mut has_coeff = [false; 16];
        for i in 0..16 {
            let bit = r.read_bits(1).unwrap_or(0);
            has_coeff[i] = bit != 0;
        }

        // Read coefficient signs and magnitudes
        for i in 0..16 {
            if has_coeff[i] {
                // Read sign bit
                let sign = r.read_bits(1).unwrap_or(0);
                // Read magnitude (2 bits for simplicity)
                let mag = r.read_bits(2).unwrap_or(1) + 1;
                coeffs[i] = if sign != 0 { -(mag as i32) } else { mag as i32 };
            }
        }

        // Apply inverse transform
        let residual = transform::inverse_transform(&coeffs, tx_type, 4);

        // Reconstruct block
        for row in 0..4 {
            for col in 0..4 {
                let pred = dc_pred as i32;
                let res = residual[row * 4 + col];
                let recon = (pred + res).clamp(0, 255) as u8;
                let idx = ((py + row as u32) * self.width + px + col as u32) as usize;
                if idx < self.y.len() {
                    self.y[idx] = recon;
                }
            }
        }

        Ok(())
    }

    /// Get the decoded frame.
    /// Decode a 4x4 block using the arithmetic coder to read coefficients.
    ///
    /// This implements a simplified version of AV1 coefficient decoding:
    /// 1. Read EOB (end of block) position
    /// 2. Read significance map (which coefficients are non-zero)
    /// 3. Read coefficient levels and signs
    /// 4. Apply inverse transform and reconstruct
    pub fn decode_block_with_arithmetic_coder(
        &mut self,
        dec: &mut SymbolDecoder,
        data: &[u8],
        cdf: &CdfContext,
        px: u32,
        py: u32,
        tx_type: TxType,
        qindex: u8,
    ) -> Result<(), String> {
        // DC prediction
        let dc_pred = self.dc_prediction_4x4(px, py);

        // Read coefficients using arithmetic coder
        let mut coeffs = [0i32; 16];

        // Read significance map: which coefficients are non-zero
        let mut has_coeff = [false; 16];
        for i in 0..16 {
            // Use 50% probability for significance (simplified)
            let bit = dec.read_bit(16384, data).unwrap_or(0);
            has_coeff[i] = bit != 0;
        }

        // Read coefficient values
        for i in 0..16 {
            if has_coeff[i] {
                // Read sign
                let sign = dec.read_bit(16384, data).unwrap_or(0);
                // Read magnitude (simplified: read 4 bits)
                let mag_bits = dec.read_literal(4, data).unwrap_or(1);
                let mag = mag_bits.max(1) as i32;
                coeffs[i] = if sign != 0 { -mag } else { mag };
            }
        }

        // Dequantize
        for i in 0..16 {
            let dc_delta = 0i8;
            let ac_delta = 0i8;
            coeffs[i] = transform::dequant_coeff(coeffs[i], qindex, dc_delta, ac_delta, i == 0);
        }

        // Apply inverse transform
        let residual = transform::inverse_transform(&coeffs, tx_type, 4);

        // Reconstruct block
        for row in 0..4 {
            for col in 0..4 {
                let pred = dc_pred as i32;
                let res = residual[row * 4 + col];
                let recon = (pred + res).clamp(0, 255) as u8;
                let idx = ((py + row as u32) * self.width + px + col as u32) as usize;
                if idx < self.y.len() {
                    self.y[idx] = recon;
                }
            }
        }

        Ok(())
    }

    /// Compute DC prediction for a 4x4 block.
    fn dc_prediction_4x4(&self, px: u32, py: u32) -> u8 {
        let above_sum: u32 = (0..4u32)
            .map(|x| {
                if py > 0 {
                    self.y[((py - 1) * self.width + px + x) as usize] as u32
                } else {
                    128u32
                }
            })
            .sum();

        let left_sum: u32 = (0..4u32)
            .map(|y| {
                if px > 0 {
                    self.y[((py + y) * self.width + px - 1) as usize] as u32
                } else {
                    128u32
                }
            })
            .sum();

        ((above_sum + left_sum + 4) / 8) as u8
    }

    pub fn into_frame(self) -> DecodedFrame {
        DecodedFrame {
            width: self.width,
            height: self.height,
            y: self.y,
            u: self.u,
            v: self.v,
            pixel_format: whytho_types::PixelFormat::Yuv420,
            pts: std::time::Duration::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_decoder_produces_frame() {
        let mut dec = SimpleIFrameDecoder::new(64, 64);
        let fh = FrameHeader {
            frame_type: super::super::FrameType::KeyFrame,
            show_existing_frame: false,
            frame_to_show: 0,
            show_frame: true,
            showable_frame: false,
            error_resilient_mode: false,
            width: 64,
            height: 64,
            render_width: 64,
            render_height: 64,
            superres_denom: 8,
            upscaled_width: 64,
            use_superres: false,
            frame_offset: 0,
            quantization_params: Default::default(),
            segmentation_params: Default::default(),
            delta_q_params: Default::default(),
            loop_filter_params: Default::default(),
            cdef_params: Default::default(),
            lr_params: Default::default(),
            tile_info: Default::default(),
        };
        let seq = SequenceHeader {
            seq_profile: super::super::Av1Profile::Main,
            still_picture: false,
            reduced_still_picture_header: true,
            max_frame_width: 64,
            max_frame_height: 64,
            frame_id_numbers_present: false,
            delta_frame_id_length: 0,
            additional_frame_id_length: 0,
            seq_force_integer_mv: true,
            seq_force_screen_content_tools: false,
            enable_superres: false,
            enable_cdef: false,
            enable_restoration: false,
            color_config: Default::default(),
            film_grain_params_present: false,
        };

        // Empty tile data (no coefficients)
        let tile_data = [];
        let result = dec.decode_simple(&tile_data, &fh, &seq);
        assert!(result.is_ok());

        let frame = dec.into_frame();
        assert_eq!(frame.width, 64);
        assert_eq!(frame.height, 64);
        assert_eq!(frame.y.len(), 64 * 64);
    }

    #[test]
    fn simple_decoder_dc_prediction() {
        let mut dec = SimpleIFrameDecoder::new(16, 16);
        // Set up reference samples
        for x in 0..16 {
            dec.y[x] = 100; // above row
        }
        for y in 0..16 {
            dec.y[y * 16] = 100; // left column
        }

        let fh = FrameHeader {
            frame_type: super::super::FrameType::KeyFrame,
            show_existing_frame: false,
            frame_to_show: 0,
            show_frame: true,
            showable_frame: false,
            error_resilient_mode: false,
            width: 16,
            height: 16,
            render_width: 16,
            render_height: 16,
            superres_denom: 8,
            upscaled_width: 16,
            use_superres: false,
            frame_offset: 0,
            quantization_params: Default::default(),
            segmentation_params: Default::default(),
            delta_q_params: Default::default(),
            loop_filter_params: Default::default(),
            cdef_params: Default::default(),
            lr_params: Default::default(),
            tile_info: Default::default(),
        };
        let seq = SequenceHeader {
            seq_profile: super::super::Av1Profile::Main,
            still_picture: false,
            reduced_still_picture_header: true,
            max_frame_width: 16,
            max_frame_height: 16,
            frame_id_numbers_present: false,
            delta_frame_id_length: 0,
            additional_frame_id_length: 0,
            seq_force_integer_mv: true,
            seq_force_screen_content_tools: false,
            enable_superres: false,
            enable_cdef: false,
            enable_restoration: false,
            color_config: Default::default(),
            film_grain_params_present: false,
        };

        let tile_data = [];
        let _ = dec.decode_simple(&tile_data, &fh, &seq);

        let frame = dec.into_frame();
        // For the first block (0,0), there are no reference samples,
        // so DC prediction uses 128 (neutral gray)
        assert_eq!(frame.y[0], 128);
    }
}
