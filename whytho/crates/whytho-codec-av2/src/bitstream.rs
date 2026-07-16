//! OBU framing and header writers (sequence header, uncompressed frame header, tile group).
//!
//! Reference: `avm/av2/encoder/bitstream.c` (`av2_write_obu_header`, `write_sequence_header`,
//! `write_uncompressed_header`), `avm/common/obudec.c`, and
//! `avm/av2/common/obu_util.{h,c}`.

use core::fmt;

/// AV2 OBU types (subset). Reference: `avm/avm/avm_codec.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ObuType {
    SequenceHeader = 1,
    TemporalDelimiter = 2,
    ClosedLoopKey = 4,
    OpenLoopKey = 5,
    RegularTileGroup = 7,
}

impl ObuType {
    const fn raw(self) -> u8 {
        self as u8
    }
}

/// Bitstream construction errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BitstreamError {
    /// An OBU is too large for the AVM-compatible 32-bit size field.
    ObuTooLarge { size: usize },
    /// A literal was requested with more than 64 bits.
    LiteralTooWide { bits: u8 },
    /// A value does not fit in the requested literal width.
    LiteralOutOfRange { value: u64, bits: u8 },
    /// The sequence-header writer only supports the current bootstrap profile.
    UnsupportedSequenceHeader {
        width: u32,
        height: u32,
        bit_depth: u8,
    },
    /// The frame-header writer only supports the current bootstrap profile.
    UnsupportedFrameHeader {
        width: u32,
        height: u32,
        bit_depth: u8,
    },
}

impl fmt::Display for BitstreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObuTooLarge { size } => write!(f, "OBU is too large: {size} bytes"),
            Self::LiteralTooWide { bits } => {
                write!(f, "literal width {bits} exceeds the 64-bit writer limit")
            }
            Self::LiteralOutOfRange { value, bits } => {
                write!(f, "literal value {value} does not fit in {bits} bits")
            }
            Self::UnsupportedSequenceHeader {
                width,
                height,
                bit_depth,
            } => write!(
                f,
                "unsupported sequence header {width}x{height} {bit_depth}-bit; skeleton supports 128x128 8-bit only"
            ),
            Self::UnsupportedFrameHeader {
                width,
                height,
                bit_depth,
            } => write!(
                f,
                "unsupported frame header {width}x{height} {bit_depth}-bit; skeleton supports 128x128 8-bit only"
            ),
        }
    }
}

impl std::error::Error for BitstreamError {}

/// MSB-first byte writer for uncompressed syntax.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BitWriter {
    data: Vec<u8>,
    bit_offset: u8,
}

impl BitWriter {
    /// Create an empty writer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of bytes touched by written bits.
    pub fn byte_len(&self) -> usize {
        self.data.len()
    }

    /// Whether the current position is byte-aligned.
    pub fn is_byte_aligned(&self) -> bool {
        self.bit_offset == 0
    }

    /// Write one bit.
    pub fn write_bit(&mut self, bit: bool) {
        if self.bit_offset == 0 {
            self.data.push(0);
        }
        if bit {
            let shift = 7 - self.bit_offset;
            let last = self.data.len() - 1;
            self.data[last] |= 1 << shift;
        }
        self.bit_offset = (self.bit_offset + 1) & 7;
    }

    /// Write a fixed-width literal, MSB first.
    pub fn write_literal(&mut self, value: u64, bits: u8) -> Result<(), BitstreamError> {
        if bits > 64 {
            return Err(BitstreamError::LiteralTooWide { bits });
        }
        if bits < 64 && value >= (1u64 << bits) {
            return Err(BitstreamError::LiteralOutOfRange { value, bits });
        }
        for shift in (0..bits).rev() {
            self.write_bit(((value >> shift) & 1) != 0);
        }
        Ok(())
    }

    /// Write unsigned UVLC as used by AVM's uncompressed headers.
    pub fn write_uvlc(&mut self, value: u32) -> Result<(), BitstreamError> {
        let code_num = value as u64 + 1;
        let bits = 64 - code_num.leading_zeros() as u8;
        for _ in 1..bits {
            self.write_bit(false);
        }
        self.write_literal(code_num, bits)
    }

    /// Append AV2 trailing bits: either a full `0x80` byte or a single `1` bit.
    pub fn add_trailing_bits(&mut self) {
        if self.is_byte_aligned() {
            self.write_literal(0x80, 8).expect("8-bit literal fits");
        } else {
            self.write_bit(true);
        }
    }

    /// Finish and return written bytes.
    pub fn finish(self) -> Vec<u8> {
        self.data
    }
}

/// Raw OBU header. In AVM bit order this is extension flag, 5-bit type, then 2-bit tlayer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObuHeader {
    /// OBU type.
    pub obu_type: ObuType,
    /// Temporal layer id.
    pub temporal_id: u8,
    /// Optional embedded layer id.
    pub layer_id: u8,
}

impl ObuHeader {
    /// Create a layer-zero OBU header.
    pub const fn new(obu_type: ObuType) -> Self {
        Self {
            obu_type,
            temporal_id: 0,
            layer_id: 0,
        }
    }

    /// Encode the OBU header bytes.
    pub fn encode(self) -> Vec<u8> {
        let extension = self.obu_type != ObuType::TemporalDelimiter && self.layer_id != 0;
        let mut out =
            vec![((extension as u8) << 7) | (self.obu_type.raw() << 2) | (self.temporal_id & 0x03)];
        if extension {
            out.push(self.layer_id);
        }
        out
    }
}

/// Write unsigned LEB128.
pub fn write_uleb128(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Write one Annex-B-style OBU as accepted by AVM raw `.obu` files.
///
/// Raw files prefix each complete OBU with the ULEB size of `header + payload`.
pub fn write_annexb_obu(
    out: &mut Vec<u8>,
    header: ObuHeader,
    payload: &[u8],
) -> Result<(), BitstreamError> {
    let header_bytes = header.encode();
    let obu_size =
        header_bytes
            .len()
            .checked_add(payload.len())
            .ok_or(BitstreamError::ObuTooLarge {
                size: payload.len(),
            })?;
    if obu_size > u32::MAX as usize {
        return Err(BitstreamError::ObuTooLarge { size: obu_size });
    }
    write_uleb128(obu_size as u64, out);
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(payload);
    Ok(())
}

/// Configuration for the current still-picture sequence header writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceHeaderConfig {
    /// Maximum coded luma width.
    pub width: u32,
    /// Maximum coded luma height.
    pub height: u32,
    /// Coded bit depth.
    pub bit_depth: u8,
}

impl SequenceHeaderConfig {
    /// Create a sequence-header config for the current 4:2:0 still-picture path.
    pub const fn new(width: u32, height: u32, bit_depth: u8) -> Self {
        Self {
            width,
            height,
            bit_depth,
        }
    }
}

/// Write the minimal 4:2:0 still-picture AV2 sequence-header payload.
///
/// This replaces the previous fixed byte array for the sequence header. Tool groups that do
/// not yet have Rust-side owners are still emitted as the validated fixed bootstrap profile.
pub fn write_sequence_header_payload(cfg: SequenceHeaderConfig) -> Result<Vec<u8>, BitstreamError> {
    if cfg.width != 128 || cfg.height != 128 || cfg.bit_depth != 8 {
        return Err(BitstreamError::UnsupportedSequenceHeader {
            width: cfg.width,
            height: cfg.height,
            bit_depth: cfg.bit_depth,
        });
    }

    let mut wb = BitWriter::new();

    wb.write_uvlc(0)?; // seq_header_id
    wb.write_literal(0, 5)?; // seq_profile_idc: MAIN_420_10_IP0-compatible profile.
    wb.write_bit(true); // single_picture_header_flag
    wb.write_literal(0b000001010, 9)?; // level + current 8-bit 4:2:0 chroma/bitdepth fields.
    write_frame_size(&mut wb, cfg.width, cfg.height)?;
    wb.write_literal(0x70e77791b808, 50)?; // validated bootstrap sequence tool flags.
    wb.add_trailing_bits();

    Ok(wb.finish())
}

fn write_frame_size(wb: &mut BitWriter, width: u32, height: u32) -> Result<(), BitstreamError> {
    let width_bits = bits_for_minus_one(width);
    let height_bits = bits_for_minus_one(height);
    wb.write_literal((width_bits - 1) as u64, 4)?;
    wb.write_literal((height_bits - 1) as u64, 4)?;
    wb.write_literal((width - 1) as u64, width_bits)?;
    wb.write_literal((height - 1) as u64, height_bits)?;
    Ok(())
}

fn bits_for_minus_one(value: u32) -> u8 {
    let minus_one = value.saturating_sub(1);
    (u32::BITS - minus_one.leading_zeros()).max(1) as u8
}

/// Configuration for the current still-picture key-frame header writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameHeaderConfig {
    /// Coded luma width.
    pub width: u32,
    /// Coded luma height.
    pub height: u32,
    /// Coded bit depth.
    pub bit_depth: u8,
}

impl FrameHeaderConfig {
    /// Create a frame-header config for the current 4:2:0 still-picture key frame.
    pub const fn new(width: u32, height: u32, bit_depth: u8) -> Self {
        Self {
            width,
            height,
            bit_depth,
        }
    }
}

/// Write the minimal 4:2:0 still-picture AV2 key-frame OBU payload: the uncompressed frame
/// header followed by the byte-aligned range-coded tile data.
///
/// This replaces the previous fixed `MINIMAL_CLOSED_LOOP_KEY_PAYLOAD` byte array. As with the
/// sequence header, only the fields that have a Rust-side owner are structured here; the rest
/// stay the validated bootstrap constant.
///
/// For an `OBU_CLOSED_LOOP_KEY` with `single_picture_header_flag = 1` (see
/// `write_uncompressed_header` in `avm/av2/encoder/bitstream.c`), the leading uncompressed
/// fields are two `uvlc` values — `cur_mfh_id = 0` then `seq_header_id = 0` (written because
/// `cur_mfh_id == 0`) — and, on the still-picture non-override path, `write_frame_size` emits
/// no bits. The remaining frame-header flags (screen-content / intrabc / quantization /
/// loop-filter / transform / tile) and the byte-aligned range-coded tile data are emitted as
/// the validated constant tail until each group gains its own Rust writer.
pub fn write_frame_header_payload(cfg: FrameHeaderConfig) -> Result<Vec<u8>, BitstreamError> {
    if cfg.width != 128 || cfg.height != 128 || cfg.bit_depth != 8 {
        return Err(BitstreamError::UnsupportedFrameHeader {
            width: cfg.width,
            height: cfg.height,
            bit_depth: cfg.bit_depth,
        });
    }

    let mut wb = BitWriter::new();

    wb.write_uvlc(0)?; // cur_mfh_id
    wb.write_uvlc(0)?; // seq_header_id (written because cur_mfh_id == 0)
    // KEY_FRAME, single-picture, frame_size_override = 0: write_frame_size emits no bits here.
    wb.write_literal(0x0022_400f_002f_2f5c, 54)?; // validated bootstrap frame-header flags + tile data.

    Ok(wb.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_writer_writes_msb_first_literals() {
        let mut wb = BitWriter::new();
        wb.write_literal(0b101, 3).unwrap();
        wb.write_literal(0b10, 2).unwrap();
        wb.add_trailing_bits();
        assert_eq!(wb.finish(), vec![0b1011_0100]);
    }

    #[test]
    fn byte_aligned_trailing_bits_append_0x80() {
        let mut wb = BitWriter::new();
        wb.write_literal(0xaa, 8).unwrap();
        wb.add_trailing_bits();
        assert_eq!(wb.finish(), vec![0xaa, 0x80]);
    }

    #[test]
    fn uvlc_codes_boundaries() {
        let mut wb = BitWriter::new();
        wb.write_uvlc(0).unwrap();
        wb.write_uvlc(1).unwrap();
        wb.write_uvlc(2).unwrap();
        wb.add_trailing_bits();
        assert_eq!(wb.finish(), vec![0b1010_0111]);
    }

    #[test]
    fn literal_width_is_validated() {
        let mut wb = BitWriter::new();
        assert_eq!(
            wb.write_literal(4, 2),
            Err(BitstreamError::LiteralOutOfRange { value: 4, bits: 2 })
        );
        assert_eq!(
            wb.write_literal(0, 65),
            Err(BitstreamError::LiteralTooWide { bits: 65 })
        );
    }

    #[test]
    fn annexb_obu_prefixes_complete_obu_size() {
        let mut out = Vec::new();
        write_annexb_obu(
            &mut out,
            ObuHeader::new(ObuType::TemporalDelimiter),
            &[0xde, 0xad],
        )
        .unwrap();
        assert_eq!(out, vec![3, 0x08, 0xde, 0xad]);
    }

    #[test]
    fn sequence_header_writer_reproduces_validated_bootstrap_payload() {
        let payload = write_sequence_header_payload(SequenceHeaderConfig::new(128, 128, 8))
            .expect("supported sequence header");
        assert_eq!(
            payload,
            [
                0x82, 0x0a, 0x66, 0xff, 0xfc, 0x70, 0xe7, 0x77, 0x91, 0xb8, 0x08, 0x80
            ]
        );
    }

    #[test]
    fn sequence_header_writer_rejects_unsupported_values() {
        assert!(matches!(
            write_sequence_header_payload(SequenceHeaderConfig::new(64, 128, 8)),
            Err(BitstreamError::UnsupportedSequenceHeader { .. })
        ));
        assert!(matches!(
            write_sequence_header_payload(SequenceHeaderConfig::new(128, 128, 10)),
            Err(BitstreamError::UnsupportedSequenceHeader { .. })
        ));
    }

    #[test]
    fn frame_header_writer_reproduces_validated_bootstrap_payload() {
        let payload = write_frame_header_payload(FrameHeaderConfig::new(128, 128, 8))
            .expect("supported frame header");
        assert_eq!(payload, [0xe2, 0x40, 0x0f, 0x00, 0x2f, 0x2f, 0x5c]);
    }

    #[test]
    fn frame_header_writer_rejects_unsupported_values() {
        assert!(matches!(
            write_frame_header_payload(FrameHeaderConfig::new(64, 128, 8)),
            Err(BitstreamError::UnsupportedFrameHeader { .. })
        ));
        assert!(matches!(
            write_frame_header_payload(FrameHeaderConfig::new(128, 128, 10)),
            Err(BitstreamError::UnsupportedFrameHeader { .. })
        ));
    }
}
