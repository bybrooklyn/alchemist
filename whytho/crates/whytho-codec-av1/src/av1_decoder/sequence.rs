//! AV1 Sequence Header decoder.

use super::{Av1Profile, ColorConfig, SequenceHeader};

#[allow(dead_code)]
pub struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

#[allow(dead_code)]
impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    pub fn read_bit(&mut self) -> Result<u8, String> {
        if self.byte_pos >= self.data.len() {
            return Err("unexpected end of data".into());
        }
        let bit = (self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Ok(bit)
    }

    pub fn read_bits(&mut self, n: u32) -> Result<u32, String> {
        let mut value = 0u32;
        for _ in 0..n {
            value = (value << 1) | self.read_bit()? as u32;
        }
        Ok(value)
    }

    pub fn read_f(&mut self, n: u32) -> Result<u32, String> {
        self.read_bits(n)
    }

    pub fn read_leb128(&mut self) -> Result<u64, String> {
        let mut value = 0u64;
        let mut i = 0;
        loop {
            let byte = self.read_bits(8)? as u64;
            value |= (byte & 0x7F) << (i * 7);
            i += 1;
            if byte & 0x80 == 0 {
                break;
            }
            if i >= 8 {
                return Err("leb128 too long".into());
            }
        }
        Ok(value)
    }

    pub fn read_uvlc(&mut self) -> Result<u32, String> {
        let mut leading_zeros = 0u32;
        while self.read_bit()? == 0 {
            leading_zeros += 1;
            if leading_zeros > 32 {
                return Err("uvlc overflow".into());
            }
        }
        if leading_zeros == 0 {
            return Ok(0);
        }
        let value = self.read_bits(leading_zeros)?;
        Ok(value + (1u32 << leading_zeros) - 1)
    }

    pub fn read_ns(&mut self, n: u32) -> Result<u32, String> {
        let w = (32 - n.leading_zeros()) as u32;
        let m = (1u32 << w) - n;
        let v = self.read_bits(w - 1)?;
        if v < m {
            Ok(v)
        } else {
            let extra = self.read_bits(1)?;
            Ok((v << 1) - m + extra)
        }
    }

    pub fn bytes_remaining(&self) -> usize {
        if self.byte_pos >= self.data.len() {
            0
        } else {
            self.data.len() - self.byte_pos
        }
    }

    pub fn bits_consumed(&self) -> u32 {
        self.byte_pos as u32 * 8 + self.bit_pos as u32
    }
}

pub fn decode_sequence_header(data: &[u8]) -> Result<SequenceHeader, String> {
    let mut r = BitReader::new(data);

    let seq_profile = match r.read_bits(3)? {
        0 => Av1Profile::Main,
        1 => Av1Profile::High,
        2 => Av1Profile::Professional,
        _ => return Err("invalid seq_profile".into()),
    };

    let still_picture = r.read_bit()? != 0;
    let reduced_still_picture_header = r.read_bit()? != 0;

    let (
        max_frame_width,
        max_frame_height,
        frame_id_numbers_present,
        delta_frame_id_length,
        additional_frame_id_length,
        seq_force_integer_mv,
        seq_force_screen_content_tools,
    );

    if reduced_still_picture_header {
        max_frame_width = r.read_bits(16)? + 1;
        max_frame_height = r.read_bits(16)? + 1;
        frame_id_numbers_present = false;
        delta_frame_id_length = 0;
        additional_frame_id_length = 0;
        seq_force_integer_mv = true;
        seq_force_screen_content_tools = false;
    } else {
        let _timing_info_present = r.read_bit()?;
        let _decoder_model_info_present = r.read_bit()?;
        let _initial_display_delay_present = r.read_bits(5)?;
        let _operating_points_cnt_minus_1 = r.read_bits(5)?;

        // Simplified: skip operating point parameters
        // In full impl, each operating point gets level/tier
        for _ in 0..=0 {
            let _ = r.read_bits(12)?; // operating_point_idc
            let _seq_level_idx = r.read_bits(7)?;
        }

        max_frame_width = r.read_bits(16)? + 1;
        max_frame_height = r.read_bits(16)? + 1;

        frame_id_numbers_present = r.read_bit()? != 0;
        if frame_id_numbers_present {
            delta_frame_id_length = r.read_bits(4)? + 2;
            additional_frame_id_length = r.read_bits(3)? + 1;
        } else {
            delta_frame_id_length = 0;
            additional_frame_id_length = 0;
        }

        let _use_128x128_superblock = r.read_bit()?;
        let _enable_filter_intra = r.read_bit()?;
        let _enable_intra_edge_filter = r.read_bit()?;

        seq_force_integer_mv = r.read_bit()? != 0;
        seq_force_screen_content_tools = r.read_bit()? != 0;
    }

    let color_config = decode_color_config(&mut r, seq_profile)?;

    let film_grain_params_present = r.read_bit()? != 0;

    Ok(SequenceHeader {
        seq_profile,
        still_picture,
        reduced_still_picture_header,
        max_frame_width,
        max_frame_height,
        frame_id_numbers_present,
        delta_frame_id_length,
        additional_frame_id_length,
        seq_force_integer_mv,
        seq_force_screen_content_tools,
        enable_superres: false,
        enable_cdef: false,
        enable_restoration: false,
        color_config,
        film_grain_params_present,
    })
}

fn decode_color_config(r: &mut BitReader, profile: Av1Profile) -> Result<ColorConfig, String> {
    let high_bitdepth = r.read_bit()? != 0;

    let bit_depth = match profile {
        Av1Profile::Professional => {
            let twelve_bit = r.read_bit()? != 0;
            if high_bitdepth {
                if twelve_bit { 12 } else { 10 }
            } else {
                8
            }
        }
        _ => {
            if high_bitdepth {
                10
            } else {
                8
            }
        }
    };

    let mono_chrome = if profile == Av1Profile::Main {
        false
    } else {
        r.read_bit()? != 0
    };

    let (subsampling_x, subsampling_y) = if mono_chrome {
        (true, true)
    } else {
        match profile {
            Av1Profile::Main => (true, true),
            Av1Profile::High => {
                let sx = r.read_bit()? != 0;
                let sy = if sx { r.read_bit()? != 0 } else { false };
                (sx, sy)
            }
            Av1Profile::Professional => {
                let sx = r.read_bit()? != 0;
                let sy = if sx { r.read_bit()? != 0 } else { false };
                (sx, sy)
            }
        }
    };

    let chroma_sample_position = if !mono_chrome && subsampling_x && subsampling_y {
        r.read_bits(2)? as u8
    } else {
        0
    };

    let separate_uv_delta_q = r.read_bit()? != 0;
    let _ = separate_uv_delta_q;

    Ok(ColorConfig {
        bit_depth,
        subsampling_x,
        subsampling_y,
        mono_chrome,
        color_primaries: 1,
        transfer_characteristics: 1,
        matrix_coefficients: 1,
        color_range: false,
        chroma_sample_position,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitreader_basic() {
        let data = [0b10110100];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 0);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bit().unwrap(), 0);
        assert_eq!(r.read_bits(3).unwrap(), 0b100);
    }

    #[test]
    fn bitreader_leb128() {
        let data = [0x81, 0x01];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read_leb128().unwrap(), 0x81);
    }
}
