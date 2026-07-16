//! Sequence Parameter Set (SPS) writer for H.264.

use super::nal;
use super::{BitstreamWriter, H264EncoderConfig, H264Profile};

pub fn write_sps(w: &mut BitstreamWriter, config: &H264EncoderConfig) -> Result<(), String> {
    let mut sps = BitstreamWriter::new();

    let profile_idc = match config.profile {
        H264Profile::Baseline => 66,
        H264Profile::Main => 77,
        H264Profile::High => 100,
    };

    sps.write_bits(profile_idc, 8); // profile_idc

    // constraint_set0_flag .. constraint_set5_flag
    match config.profile {
        H264Profile::Baseline => {
            sps.write_bits(1, 1); // constraint_set0_flag
            sps.write_bits(1, 1); // constraint_set1_flag
            sps.write_bits(0, 1); // constraint_set2_flag
            sps.write_bits(0, 1); // constraint_set3_flag
            sps.write_bits(0, 1); // constraint_set4_flag
            sps.write_bits(0, 1); // constraint_set5_flag
        }
        H264Profile::Main => {
            sps.write_bits(0, 1);
            sps.write_bits(1, 1);
            sps.write_bits(0, 1);
            sps.write_bits(0, 1);
            sps.write_bits(0, 1);
            sps.write_bits(0, 1);
        }
        H264Profile::High => {
            sps.write_bits(0, 1);
            sps.write_bits(0, 1);
            sps.write_bits(0, 1);
            sps.write_bits(0, 1);
            sps.write_bits(0, 1);
            sps.write_bits(0, 1);
        }
    }

    sps.write_bits(0, 2); // reserved_zero_2bits

    sps.write_bits(config.level.level_idc() as u32, 8); // level_idc

    sps.write_ue(0); // seq_parameter_set_id

    if profile_idc == 100
        || profile_idc == 110
        || profile_idc == 122
        || profile_idc == 244
        || profile_idc == 44
        || profile_idc == 83
        || profile_idc == 86
        || profile_idc == 118
        || profile_idc == 128
        || profile_idc == 138
        || profile_idc == 139
        || profile_idc == 134
        || profile_idc == 135
    {
        sps.write_ue(1); // chroma_format_idc = 1 (4:2:0)
        sps.write_ue(0); // bit_depth_luma_minus8
        sps.write_ue(0); // bit_depth_chroma_minus8
        sps.write_bits(0, 1); // qpprime_y_zero_transform_bypass_flag
        sps.write_bits(0, 1); // seq_scaling_matrix_present_flag
    }

    sps.write_ue(0); // log2_max_frame_num_minus4

    sps.write_ue(0); // pic_order_cnt_type = 0
    sps.write_ue(4); // log2_max_pic_order_cnt_lsb_minus4

    sps.write_ue(0); // max_num_ref_frames
    sps.write_bits(0, 1); // gaps_in_frame_num_value_allowed_flag

    let (pic_width_in_mbs, pic_height_in_map_units) = config.mb_dims();
    sps.write_ue(pic_width_in_mbs - 1); // pic_width_in_mbs_minus1
    sps.write_ue(pic_height_in_map_units - 1); // pic_height_in_map_units_minus1

    sps.write_bits(1, 1); // frame_mbs_only_flag = 1 (progressive)

    sps.write_bits(0, 1); // direct_8x8_inference_flag

    // frame_cropping_flag
    let crop_right = (pic_width_in_mbs * 16 - config.width) / 2;
    let crop_bottom = (pic_height_in_map_units * 16 - config.height) / 2;
    if crop_right > 0 || crop_bottom > 0 {
        sps.write_bits(1, 1); // frame_cropping_flag
        sps.write_ue(0); // frame_crop_left_offset
        sps.write_ue(crop_right); // frame_crop_right_offset
        sps.write_ue(0); // frame_crop_top_offset
        sps.write_ue(crop_bottom); // frame_crop_bottom_offset
    } else {
        sps.write_bits(0, 1);
    }

    sps.write_bits(0, 1); // vui_parameters_present_flag

    let sps_bytes = sps.take_rbsp_bytes();
    nal::write_nal(w, nal::NAL_TYPE_SPS, 3, &sps_bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_sps_produces_bytes() {
        let mut w = BitstreamWriter::new();
        let config = H264EncoderConfig {
            width: 640,
            height: 480,
            ..Default::default()
        };
        write_sps(&mut w, &config).unwrap();
        let bytes = w.take_bytes();
        assert!(!bytes.is_empty());
        // Should start with Annex B start code
        assert_eq!(&bytes[0..4], &[0x00, 0x00, 0x00, 0x01]);
        // NAL type 7 (SPS)
        assert_eq!(bytes[4] & 0x1F, 7);
    }
}
