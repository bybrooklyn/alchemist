//! AV1 Frame Header decoder.
//!
//! Parses the uncompressed header from a frame OBU.
//! Reference: AV1 spec section 6.8 (Uncompressed header syntax)

use super::sequence::BitReader;
use super::{
    CdefParams, DeltaQParams, FrameHeader, FrameType, LoopFilterParams, LrParams,
    QuantizationParams, SegmentationParams, SequenceHeader, TileInfo,
};

pub fn decode_frame_header(data: &[u8], seq: &SequenceHeader) -> Result<FrameHeader, String> {
    if data.is_empty() {
        return Err("empty frame header".into());
    }

    let mut r = BitReader::new(data);

    // show_existing_frame
    let show_existing_frame = r.read_bit()? != 0;
    if show_existing_frame {
        let frame_to_show = r.read_bits(3)?;
        return Ok(FrameHeader {
            frame_type: FrameType::KeyFrame,
            show_existing_frame: true,
            frame_to_show,
            show_frame: true,
            showable_frame: true,
            error_resilient_mode: false,
            width: seq.max_frame_width,
            height: seq.max_frame_height,
            render_width: seq.max_frame_width,
            render_height: seq.max_frame_height,
            superres_denom: 8,
            upscaled_width: seq.max_frame_width,
            use_superres: false,
            frame_offset: 0,
            quantization_params: Default::default(),
            segmentation_params: Default::default(),
            delta_q_params: Default::default(),
            loop_filter_params: Default::default(),
            cdef_params: Default::default(),
            lr_params: Default::default(),
            tile_info: Default::default(),
        });
    }

    // frame_type
    let frame_type = match r.read_bits(2)? {
        0 => FrameType::KeyFrame,
        1 => FrameType::InterFrame,
        2 => FrameType::IntraOnlyFrame,
        3 => FrameType::SwitchFrame,
        _ => return Err("invalid frame_type".into()),
    };

    let show_frame = r.read_bit()? != 0;

    // For keyframes and show_frame, error resilient mode is implied
    let error_resilient_mode = if frame_type == FrameType::KeyFrame && show_frame {
        true
    } else {
        r.read_bit()? != 0
    };

    // disable_cdf_update
    let _disable_cdf_update = r.read_bit()? != 0;

    // allow_screen_content_tools
    let allow_screen_content_tools = if seq.seq_force_screen_content_tools {
        true
    } else {
        r.read_bit()? != 0
    };

    // force_integer_mv
    let force_integer_mv = if !allow_screen_content_tools {
        false
    } else if seq.seq_force_integer_mv {
        true
    } else {
        r.read_bit()? != 0
    };

    // frame_size
    let (width, height) = if frame_type == FrameType::KeyFrame {
        // Use sequence header size for keyframes
        (seq.max_frame_width, seq.max_frame_height)
    } else {
        // frame_size_override_flag
        let override_flag = r.read_bit()? != 0;
        if override_flag {
            let w = r.read_bits(16)? + 1;
            let h = r.read_bits(16)? + 1;
            (w, h)
        } else {
            (seq.max_frame_width, seq.max_frame_height)
        }
    };

    // Superres
    let use_superres = if seq.enable_superres {
        r.read_bit()? != 0
    } else {
        false
    };
    let superres_denom = if use_superres { r.read_bits(3)? + 9 } else { 8 };
    let upscaled_width = width;
    let render_width = upscaled_width;

    // frame_offset
    let frame_offset = if !show_frame && frame_type != FrameType::KeyFrame {
        r.read_bits(4)?
    } else {
        0
    };

    // Quantization params
    let quantization_params = decode_quantization_params(&mut r)?;

    // Segmentation params
    let segmentation_params = decode_segmentation_params(&mut r)?;

    // Delta Q params
    let delta_q_params = decode_delta_q_params(&mut r)?;

    // Loop filter params
    let loop_filter_params = decode_loop_filter_params(&mut r, frame_type)?;

    // CDEF params
    let cdef_params = decode_cdef_params(&mut r, seq)?;

    // Loop restoration params
    let lr_params = decode_lr_params(&mut r, seq)?;

    // Tile info
    let tile_info = decode_tile_info(&mut r, width, height)?;

    Ok(FrameHeader {
        frame_type,
        show_existing_frame,
        frame_to_show: 0,
        show_frame,
        showable_frame: show_frame,
        error_resilient_mode,
        width,
        height,
        render_width,
        render_height: height,
        superres_denom,
        upscaled_width,
        use_superres,
        frame_offset,
        quantization_params,
        segmentation_params,
        delta_q_params,
        loop_filter_params,
        cdef_params,
        lr_params,
        tile_info,
    })
}

fn decode_quantization_params(r: &mut BitReader) -> Result<QuantizationParams, String> {
    let base_q_idx = r.read_bits(8)? as u8;
    let delta_q_y_dc = read_delta_q(r)?;
    let using_qmatrix = r.read_bit()? != 0;

    let (delta_q_u_dc, delta_q_u_ac, delta_q_v_dc, delta_q_v_ac, qm_y, qm_u, qm_v) =
        if using_qmatrix {
            let udc = read_delta_q(r)?;
            let uac = read_delta_q(r)?;
            let vdc = read_delta_q(r)?;
            let vac = read_delta_q(r)?;
            let qm_y = r.read_bits(4)? as u8;
            let qm_u = r.read_bits(4)? as u8;
            let qm_v = r.read_bits(4)? as u8;
            (udc, uac, vdc, vac, qm_y, qm_u, qm_v)
        } else {
            (0, 0, 0, 0, 0, 0, 0)
        };

    Ok(QuantizationParams {
        base_q_idx,
        delta_q_y_dc,
        delta_q_u_dc,
        delta_q_u_ac,
        delta_q_v_dc,
        delta_q_v_ac,
        using_qmatrix,
        qm_y,
        qm_u,
        qm_v,
    })
}

fn read_delta_q(r: &mut BitReader) -> Result<i8, String> {
    let delta_coded = r.read_bit()? != 0;
    if !delta_coded {
        return Ok(0);
    }
    let delta_q = r.read_bits(6)? as i8;
    let sign = r.read_bit()? != 0;
    Ok(if sign { -delta_q } else { delta_q })
}

fn decode_segmentation_params(r: &mut BitReader) -> Result<SegmentationParams, String> {
    let segmentation_enabled = r.read_bit()? != 0;
    if !segmentation_enabled {
        return Ok(SegmentationParams {
            segmentation_enabled: false,
            segmentation_update_map: false,
            segmentation_temporal_update: false,
            segmentation_update_data: false,
        });
    }

    let segmentation_update_map = r.read_bit()? != 0;
    let segmentation_temporal_update = if segmentation_update_map {
        r.read_bit()? != 0
    } else {
        false
    };
    let segmentation_update_data = r.read_bit()? != 0;

    // Skip segmentation data for now
    if segmentation_update_data {
        // Skip 8 segments * 6 features * bits
        // Simplified: just advance the reader
    }

    Ok(SegmentationParams {
        segmentation_enabled,
        segmentation_update_map,
        segmentation_temporal_update,
        segmentation_update_data,
    })
}

fn decode_delta_q_params(r: &mut BitReader) -> Result<DeltaQParams, String> {
    let delta_q_present = r.read_bit()? != 0;
    let delta_q_res = if delta_q_present {
        r.read_bits(2)? as u8
    } else {
        0
    };
    Ok(DeltaQParams {
        delta_q_present,
        delta_q_res,
    })
}

fn decode_loop_filter_params(
    r: &mut BitReader,
    frame_type: FrameType,
) -> Result<LoopFilterParams, String> {
    if frame_type == FrameType::KeyFrame || frame_type == FrameType::IntraOnlyFrame {
        // For intra frames, loop filter is typically disabled
        return Ok(LoopFilterParams::default());
    }

    let loop_filter_level_0 = r.read_bits(6)? as u8;
    let loop_filter_level_1 = r.read_bits(6)? as u8;

    let loop_filter_sharpness = r.read_bits(3)? as u8;

    let loop_filter_ref_deltas = if r.read_bit()? != 0 {
        // loop_filter_delta_enabled
        let mut deltas = [0i8; 8];
        for i in 0..8 {
            if r.read_bit()? != 0 {
                // delta_update
                let delta = r.read_bits(6)? as i8;
                let sign = r.read_bit()? != 0;
                deltas[i] = if sign { -delta } else { delta };
            }
        }
        deltas
    } else {
        [0; 8]
    };

    Ok(LoopFilterParams {
        loop_filter_level_0,
        loop_filter_level_1,
        loop_filter_ref_deltas,
        loop_filter_mode_deltas: [0, 0],
        loop_filter_sharpness,
    })
}

fn decode_cdef_params(r: &mut BitReader, seq: &SequenceHeader) -> Result<CdefParams, String> {
    if !seq.enable_cdef {
        return Ok(CdefParams::default());
    }

    let cdef_damping = r.read_bits(2)? as u8 + 3;
    let cdef_bits = r.read_bits(2)? as u8;
    let num_cdef = 1 << cdef_bits;

    let mut cdef_y_strength = [0u8; 8];
    let mut cdef_uv_strength = [0u8; 8];

    for i in 0..num_cdef {
        cdef_y_strength[i] = r.read_bits(4)? as u8;
        cdef_uv_strength[i] = r.read_bits(4)? as u8;
    }

    Ok(CdefParams {
        cdef_damping,
        cdef_bits,
        cdef_y_strength,
        cdef_uv_strength,
    })
}

fn decode_lr_params(r: &mut BitReader, seq: &SequenceHeader) -> Result<LrParams, String> {
    if !seq.enable_restoration {
        return Ok(LrParams::default());
    }

    let lr_type = [
        r.read_bits(2)? as u8,
        r.read_bits(2)? as u8,
        r.read_bits(2)? as u8,
    ];
    let lr_unit_shift = r.read_bits(2)? as u8;

    Ok(LrParams {
        lr_type,
        lr_unit_shift,
    })
}

fn decode_tile_info(r: &mut BitReader, width: u32, height: u32) -> Result<TileInfo, String> {
    // Simplified tile info: single tile
    let _increment_tile_cols_log2 = r.read_bit()? != 0;
    let tile_cols_log2 = if _increment_tile_cols_log2 {
        // For now, just use 1 tile column
        0
    } else {
        0
    };

    let _increment_tile_rows_log2 = r.read_bit()? != 0;
    let tile_rows_log2 = if _increment_tile_rows_log2 { 0 } else { 0 };

    Ok(TileInfo {
        tile_cols_log2,
        tile_rows_log2,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_header_empty_data() {
        let seq = SequenceHeader {
            seq_profile: super::super::Av1Profile::Main,
            still_picture: false,
            reduced_still_picture_header: true,
            max_frame_width: 1920,
            max_frame_height: 1080,
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
        let result = decode_frame_header(&[], &seq);
        assert!(result.is_err());
    }
}
