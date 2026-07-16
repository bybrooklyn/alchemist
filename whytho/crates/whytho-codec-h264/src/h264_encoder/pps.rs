//! Picture Parameter Set (PPS) writer for H.264.

use super::nal;
use super::{BitstreamWriter, H264EncoderConfig};

pub fn write_pps(w: &mut BitstreamWriter, _config: &H264EncoderConfig) -> Result<(), String> {
    let mut pps = BitstreamWriter::new();

    pps.write_ue(0); // pic_parameter_set_id
    pps.write_ue(0); // seq_parameter_set_id

    pps.write_bits(0, 1); // entropy_coding_mode_flag = 0 (CAVLC)
    pps.write_bits(0, 1); // bottom_field_pic_order_in_frame_present_flag

    pps.write_ue(0); // num_slice_groups_minus1

    pps.write_ue(0); // num_ref_idx_l0_default_active_minus1
    pps.write_ue(0); // num_ref_idx_l1_default_active_minus1

    pps.write_bits(0, 1); // weighted_pred_flag
    pps.write_bits(0, 2); // weighted_bipred_idc

    pps.write_se(0); // pic_init_qp_minus26
    pps.write_se(0); // pic_init_qs_minus26
    pps.write_se(0); // chroma_qp_index_offset

    // 1: the slice header carries disable_deblocking_filter_idc (set to 1 = disabled).
    // It must be signalled present, or the decoder will not consume that syntax element
    // and the remainder of the slice desyncs.
    pps.write_bits(1, 1); // deblocking_filter_control_present_flag
    pps.write_bits(0, 1); // constrained_intra_pred_flag
    pps.write_bits(0, 1); // redundant_pic_cnt_present_flag

    // High profile extended fields removed — Baseline profile decoder stops here.
    // The decoder uses more_rbsp_data() to detect these; writing them for Baseline
    // would shift all subsequent bit reads and cause decoding failures.

    let pps_bytes = pps.take_rbsp_bytes();
    nal::write_nal(w, nal::NAL_TYPE_PPS, 3, &pps_bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_pps_produces_bytes() {
        let mut w = BitstreamWriter::new();
        let config = H264EncoderConfig::default();
        write_pps(&mut w, &config).unwrap();
        let bytes = w.take_bytes();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..4], &[0x00, 0x00, 0x00, 0x01]);
        assert_eq!(bytes[4] & 0x1F, 8); // NAL type 8 (PPS)
    }
}
