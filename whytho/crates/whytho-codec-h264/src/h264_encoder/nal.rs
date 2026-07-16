//! NAL unit packaging for H.264 bitstream.

use super::BitstreamWriter;

pub const NAL_TYPE_SLICE: u8 = 1;
pub const NAL_TYPE_IDR: u8 = 5;
pub const NAL_TYPE_SPS: u8 = 7;
pub const NAL_TYPE_PPS: u8 = 8;

pub fn write_nal_start_code(w: &mut BitstreamWriter) {
    w.write_bytes(&[0x00, 0x00, 0x00, 0x01]);
}

pub fn write_nal_header(w: &mut BitstreamWriter, nal_type: u8, nal_ref_idc: u8) {
    w.write_bits(0, 1); // forbidden_zero_bit
    w.write_bits(nal_ref_idc as u32, 2); // nal_ref_idc
    w.write_bits(nal_type as u32, 5); // nal_unit_type
}

pub fn write_nal(w: &mut BitstreamWriter, nal_type: u8, nal_ref_idc: u8, payload: &[u8]) {
    write_nal_start_code(w);
    write_nal_header(w, nal_type, nal_ref_idc);
    w.write_bytes(payload);
}

pub fn write_nal_with_emulation_prevention(
    w: &mut BitstreamWriter,
    nal_type: u8,
    nal_ref_idc: u8,
    payload: &[u8],
) {
    write_nal_start_code(w);
    write_nal_header(w, nal_type, nal_ref_idc);

    let mut zero_count = 0u32;
    for &byte in payload {
        if zero_count >= 2 && byte <= 3 {
            w.align();
            w.write_bytes(&[0x03]); // emulation prevention byte
            zero_count = 0;
        }
        if byte == 0 {
            zero_count += 1;
        } else {
            zero_count = 0;
        }
        w.align();
        w.write_bytes(&[byte]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_code_is_annexb() {
        let mut w = BitstreamWriter::new();
        write_nal_start_code(&mut w);
        assert_eq!(w.take_bytes(), vec![0x00, 0x00, 0x00, 0x01]);
    }

    #[test]
    fn header_idr_byte() {
        // forbidden_zero(0) | nal_ref_idc(3) << 5 | nal_unit_type(5) = 0x65
        let mut w = BitstreamWriter::new();
        write_nal_header(&mut w, NAL_TYPE_IDR, 3);
        assert_eq!(w.take_bytes(), vec![0x65]);
    }

    #[test]
    fn header_sps_byte() {
        // nal_ref_idc(3) << 5 | nal_unit_type(7) = 0x67, the canonical SPS NAL byte.
        let mut w = BitstreamWriter::new();
        write_nal_header(&mut w, NAL_TYPE_SPS, 3);
        assert_eq!(w.take_bytes(), vec![0x67]);
    }

    #[test]
    fn write_nal_wraps_payload() {
        let mut w = BitstreamWriter::new();
        write_nal(&mut w, NAL_TYPE_SLICE, 0, &[0xAA, 0xBB]);
        // start code + header(type=1, ref_idc=0 => 0x01) + payload
        assert_eq!(
            w.take_bytes(),
            vec![0x00, 0x00, 0x00, 0x01, 0x01, 0xAA, 0xBB]
        );
    }

    #[test]
    fn emulation_prevention_inserts_03() {
        // Three consecutive zero bytes must get a 0x03 emulation byte before the third.
        let mut w = BitstreamWriter::new();
        write_nal_with_emulation_prevention(&mut w, NAL_TYPE_SLICE, 0, &[0x00, 0x00, 0x00]);
        assert_eq!(
            w.take_bytes(),
            vec![0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x03, 0x00]
        );
    }

    #[test]
    fn emulation_prevention_00_00_01() {
        // The classic 00 00 01 (start-code-alike) must become 00 00 03 01.
        let mut w = BitstreamWriter::new();
        write_nal_with_emulation_prevention(&mut w, NAL_TYPE_SLICE, 0, &[0x00, 0x00, 0x01]);
        assert_eq!(
            w.take_bytes(),
            vec![0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x03, 0x01]
        );
    }

    #[test]
    fn emulation_prevention_passthrough() {
        // Payload with no 00 00 0x pattern is emitted unchanged.
        let mut w = BitstreamWriter::new();
        write_nal_with_emulation_prevention(&mut w, NAL_TYPE_SLICE, 0, &[0xAA, 0xBB, 0xCC]);
        assert_eq!(
            w.take_bytes(),
            vec![0x00, 0x00, 0x00, 0x01, 0x01, 0xAA, 0xBB, 0xCC]
        );
    }
}
