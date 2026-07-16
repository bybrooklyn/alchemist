//! Slice encoding for H.264.
//!
//! Implements I-slice encoding with SATD-based mode decision,
//! I16x16/I4x4 intra prediction, and CAVLC residual coding.
//!
//! Residual syntax follows H.264 spec section 7.3.5.3:
//! - I16x16: luma DC (4x4 Hadamard) + luma AC (16x 4x4 DCT) + chroma DC (2x2 Hadamard interleaved)
//! - I4x4: luma (16x 4x4 DCT, gated by cbp_luma) + chroma DC (2x2 Hadamard interleaved)

use super::cavlc;
use super::intra;
use super::nal;
use super::quantize;
use super::transform;
use super::{BitstreamWriter, H264EncoderConfig, Intra4x4Mode, Intra16x16Mode, Macroblock, MbType};
use crate::DecodedFrame;

pub fn write_slice(
    w: &mut BitstreamWriter,
    config: &H264EncoderConfig,
    frame: &DecodedFrame,
    is_idr: bool,
) -> Result<(), String> {
    let mut slice_data = BitstreamWriter::new();

    let nal_type = if is_idr {
        nal::NAL_TYPE_IDR
    } else {
        nal::NAL_TYPE_SLICE
    };
    let nal_ref_idc = if is_idr { 3 } else { 0 };

    slice_data.write_ue(0); // first_mb_in_slice
    slice_data.write_ue(2); // slice_type = I (2)
    slice_data.write_ue(0); // pic_parameter_set_id
    slice_data.write_bits(0, 4); // frame_num

    if is_idr {
        slice_data.write_ue(0); // idr_pic_id
    }

    slice_data.write_bits(0, 8); // pic_order_cnt_lsb (8 bits per SPS: log2_max_pic_order_cnt_lsb_minus4=4)

    // dec_ref_pic_marking (spec 7.3.3.3) — present for every reference picture and
    // read by the decoder immediately before slice_qp_delta. Omitting it shifts the
    // rest of the slice header by these bits and desyncs the whole slice.
    if is_idr {
        slice_data.write_bits(0, 1); // no_output_of_prior_pics_flag = 0
        slice_data.write_bits(0, 1); // long_term_reference_flag = 0
    } else if nal_ref_idc != 0 {
        slice_data.write_bits(0, 1); // adaptive_ref_pic_marking_mode_flag = 0
    }

    let qp = config.qp as i32;
    slice_data.write_se(qp - 26); // slice_qp_delta

    slice_data.write_ue(1); // disable_deblocking_filter_idc = 1 (disabled)

    let (mb_width, mb_height) = config.mb_dims();

    // Per-block total_coeff for every luma 4x4 block in the frame, indexed exactly
    // as the decoder's `nc_luma` (mb_idx * 16 + scan_block_index). Drives the CAVLC
    // coeff_token table selection (nC) so encoder and decoder agree bit-for-bit.
    let mut nc_luma = vec![0u8; (mb_width * mb_height) as usize * 16];

    // Per-block I4x4 prediction mode for the entire frame (mb_idx * 16 + block_index).
    // Initialized to DC (2) matching the decoder's default for unavailable neighbors.
    // Only updated when an MB is encoded as I4x4.
    let mut i4x4_modes = vec![2u8; (mb_width * mb_height) as usize * 16];

    for mb_y in 0..mb_height {
        for mb_x in 0..mb_width {
            let mb = extract_macroblock(frame, mb_x, mb_y, config.qp);
            let mb_idx = (mb_y * mb_width + mb_x) as usize;
            encode_macroblock(
                &mut slice_data,
                &mb,
                config,
                frame,
                &mut nc_luma,
                &mut i4x4_modes,
                mb_idx,
                mb_width as usize,
            )?;
        }
    }

    let slice_bytes = slice_data.take_rbsp_bytes();
    nal::write_nal_with_emulation_prevention(w, nal_type, nal_ref_idc, &slice_bytes);
    Ok(())
}

fn extract_macroblock(frame: &DecodedFrame, mb_x: u32, mb_y: u32, qp: i8) -> Macroblock {
    let mut y = [[0u8; 16]; 16];
    let mut cb = [[0u8; 8]; 8];
    let mut cr = [[0u8; 8]; 8];

    let x = mb_x * 16;
    let y_pos = mb_y * 16;

    for row in 0..16usize {
        for col in 0..16usize {
            let px = (x + col as u32).min(frame.width - 1) as usize;
            let py = (y_pos + row as u32).min(frame.height - 1) as usize;
            let idx = py * frame.width as usize + px;
            y[row][col] = frame.y.get(idx).copied().unwrap_or(0);
        }
    }

    let cx = mb_x * 8;
    let cy = mb_y * 8;
    let cw = frame.width.div_ceil(2);
    let ch = frame.height.div_ceil(2);

    for row in 0..8usize {
        for col in 0..8usize {
            let px = (cx + col as u32).min(cw - 1) as usize;
            let py = (cy + row as u32).min(ch - 1) as usize;
            let idx = py * cw as usize + px;
            cb[row][col] = frame.u.get(idx).copied().unwrap_or(128);
            cr[row][col] = frame.v.get(idx).copied().unwrap_or(128);
        }
    }

    Macroblock {
        mb_type: MbType::I16x16(Intra16x16Mode::Dc, 0, 0),
        mb_x,
        mb_y,
        qp,
        y,
        cb,
        cr,
    }
}

/// Scan-order luma 4x4 block index -> raster (by*4+bx) index. The CAVLC bitstream
/// orders blocks as 8x8 groups in raster, then 4x4 within each 8x8 in raster, which
/// the decoder maps back to pixel offsets via BLOCK_INDEX_TO_OFFSET.
const SCAN_TO_RASTER: [usize; 16] = [0, 1, 4, 5, 2, 3, 6, 7, 8, 9, 12, 13, 10, 11, 14, 15];

/// Compute nC for a luma 4x4 block exactly as the decoder does (H.264 9.2.1), for the
/// non-MBAFF single-slice case: average the left/above neighbour total_coeff counts.
fn compute_nc_luma(nc_luma: &[u8], mb_idx: usize, mb_width: usize, blk: usize) -> i32 {
    let (left_blk, left_in_mb) = match blk {
        0 => (5usize, false),
        2 => (7, false),
        8 => (13, false),
        10 => (15, false),
        4 => (1, true),
        6 => (3, true),
        12 => (9, true),
        14 => (11, true),
        1 => (0, true),
        3 => (2, true),
        5 => (4, true),
        7 => (6, true),
        9 => (8, true),
        11 => (10, true),
        13 => (12, true),
        15 => (14, true),
        _ => unreachable!(),
    };
    let nc_a: Option<i32> = if left_in_mb {
        Some(nc_luma[mb_idx * 16 + left_blk] as i32)
    } else if mb_idx % mb_width != 0 {
        Some(nc_luma[(mb_idx - 1) * 16 + left_blk] as i32)
    } else {
        None
    };

    let (above_blk, above_in_mb) = match blk {
        0 => (10usize, false),
        1 => (11, false),
        4 => (14, false),
        5 => (15, false),
        2 => (0, true),
        3 => (1, true),
        6 => (4, true),
        7 => (5, true),
        8 => (2, true),
        9 => (3, true),
        10 => (8, true),
        11 => (9, true),
        12 => (6, true),
        13 => (7, true),
        14 => (12, true),
        15 => (13, true),
        _ => unreachable!(),
    };
    let nc_b: Option<i32> = if above_in_mb {
        Some(nc_luma[mb_idx * 16 + above_blk] as i32)
    } else if mb_idx >= mb_width {
        Some(nc_luma[(mb_idx - mb_width) * 16 + above_blk] as i32)
    } else {
        None
    };

    match (nc_a, nc_b) {
        (Some(a), Some(b)) => (a + b + 1) >> 1,
        (Some(n), None) | (None, Some(n)) => n,
        (None, None) => 0,
    }
}

/// Gather the source-frame neighbour samples for the macroblock at (mb_x, mb_y),
/// clamped to the frame edge (matching `extract_macroblock`'s padding). Availability
/// follows the frame boundary so the first row/column predict from 128 exactly like
/// the decoder. These are *source* (not reconstructed) neighbours, so reconstruction
/// drifts by at most the quantiser step — acceptable for intra-only output.
fn extract_luma_neighbors(frame: &DecodedFrame, mb_x: u32, mb_y: u32) -> intra::LumaNeighbors {
    let w = frame.width as usize;
    let h = frame.height as usize;
    let x0 = mb_x as usize * 16;
    let y0 = mb_y as usize * 16;
    let px = |x: usize, y: usize| -> u8 {
        let xx = x.min(w - 1);
        let yy = y.min(h - 1);
        frame.y.get(yy * w + xx).copied().unwrap_or(0)
    };

    let above = if mb_y > 0 {
        let mut a = [0u8; 16];
        for (i, s) in a.iter_mut().enumerate() {
            *s = px(x0 + i, y0 - 1);
        }
        Some(a)
    } else {
        None
    };
    let left = if mb_x > 0 {
        let mut l = [0u8; 16];
        for (i, s) in l.iter_mut().enumerate() {
            *s = px(x0 - 1, y0 + i);
        }
        Some(l)
    } else {
        None
    };
    let above_left = if mb_x > 0 && mb_y > 0 {
        Some(px(x0 - 1, y0 - 1))
    } else {
        None
    };
    let above_right = if mb_y > 0 {
        let mut ar = [0u8; 16];
        for (i, s) in ar.iter_mut().enumerate() {
            *s = px(x0 + 16 + i, y0 - 1);
        }
        Some(ar)
    } else {
        None
    };

    intra::LumaNeighbors {
        above,
        left,
        above_left,
        above_right,
    }
}

fn encode_macroblock(
    w: &mut BitstreamWriter,
    mb: &Macroblock,
    config: &H264EncoderConfig,
    frame: &DecodedFrame,
    nc_luma: &mut [u8],
    i4x4_modes: &mut [u8],
    mb_idx: usize,
    mb_width: usize,
) -> Result<(), String> {
    let neighbors = extract_luma_neighbors(frame, mb.mb_x, mb.mb_y);
    let (i16x16_mode, i16x16_pred) = intra::choose_best_i16x16_n(mb, &neighbors);

    // Compute I16x16 cost with the same per-4x4 SATD metric used for I4x4 below, so the
    // two modes are compared apples-to-apples. Comparing raw whole-block SAD (pixel
    // domain) against I4x4's summed SATD (transform domain) structurally biased mode
    // selection, since the two metrics aren't on the same scale.
    let i16x16_cost: u32 = (0..4)
        .flat_map(|by| (0..4).map(move |bx| (by, bx)))
        .map(|(by, bx)| {
            let mut original = [[0u8; 4]; 4];
            let mut pred = [[0u8; 4]; 4];
            for r in 0..4 {
                for c in 0..4 {
                    original[r][c] = mb.y[by * 4 + r][bx * 4 + c];
                    pred[r][c] = i16x16_pred[by * 4 + r][bx * 4 + c];
                }
            }
            intra::satd_4x4(&original, &pred)
        })
        .sum();

    // Compute I4x4 cost: SATD-based mode decision per 4x4 block
    let (i4x4_cost, i4x4_modes_local) = compute_i4x4_cost(mb, frame);

    if i4x4_cost < i16x16_cost {
        encode_i4x4_macroblock(
            w,
            mb,
            config,
            frame,
            i4x4_modes_local,
            nc_luma,
            i4x4_modes,
            mb_idx,
            mb_width,
        )?;
    } else {
        encode_i16x16_macroblock(
            w,
            mb,
            i16x16_mode,
            i16x16_pred,
            config.qp,
            nc_luma,
            mb_idx,
            mb_width,
        )?;
    }
    Ok(())
}

fn compute_i4x4_cost(mb: &Macroblock, frame: &DecodedFrame) -> (u32, [[Intra4x4Mode; 4]; 4]) {
    let mut total_cost = 0u32;
    let mut modes = [[Intra4x4Mode::Dc; 4]; 4];

    for by in 0..4 {
        for bx in 0..4 {
            let mut original = [[0u8; 4]; 4];
            for r in 0..4 {
                for c in 0..4 {
                    original[r][c] = mb.y[by * 4 + r][bx * 4 + c];
                }
            }

            let neighbors = intra::extract_neighbors_4x4(frame, mb.mb_x, mb.mb_y, bx, by);

            let (best_mode, best_pred) = intra::choose_best_i4x4_mode(&original, &neighbors);
            modes[by][bx] = best_mode;

            let satd = intra::satd_4x4(&original, &best_pred);
            total_cost += satd;
        }
    }

    (total_cost, modes)
}

/// Compute chroma DC residual: DC prediction → residual → 2x2 Hadamard.
/// Returns (cb_dc_transformed, cr_dc_transformed).
fn compute_chroma_dc(mb: &Macroblock) -> ([[i16; 2]; 2], [[i16; 2]; 2]) {
    let cb_pred = intra::predict_dc_chroma(&mb.cb, 8, 8);
    let cr_pred = intra::predict_dc_chroma(&mb.cr, 8, 8);

    let mut cb_dc = [[0i16; 2]; 2];
    let mut cr_dc = [[0i16; 2]; 2];

    for by in 0..2 {
        for bx in 0..2 {
            let mut dc_sum_cb = 0i16;
            let mut dc_sum_cr = 0i16;
            for r in 0..4 {
                for c in 0..4 {
                    dc_sum_cb += mb.cb[by * 4 + r][bx * 4 + c] as i16
                        - cb_pred[by * 4 + r][bx * 4 + c] as i16;
                    dc_sum_cr += mb.cr[by * 4 + r][bx * 4 + c] as i16
                        - cr_pred[by * 4 + r][bx * 4 + c] as i16;
                }
            }
            cb_dc[by][bx] = dc_sum_cb / 4;
            cr_dc[by][bx] = dc_sum_cr / 4;
        }
    }

    (
        transform::hadamard_2x2(cb_dc),
        transform::hadamard_2x2(cr_dc),
    )
}

/// Build the interleaved chroma DC residual block as the decoder expects:
/// [Cb[0][0], Cr[0][0], Cb[0][1], Cr[0][1]]
fn interleave_chroma_dc(cb_dc: &[[i16; 2]; 2], cr_dc: &[[i16; 2]; 2]) -> [i16; 4] {
    [cb_dc[0][0], cr_dc[0][0], cb_dc[0][1], cr_dc[0][1]]
}

fn encode_i4x4_macroblock(
    w: &mut BitstreamWriter,
    mb: &Macroblock,
    config: &H264EncoderConfig,
    frame: &DecodedFrame,
    modes: [[Intra4x4Mode; 4]; 4],
    nc_luma: &mut [u8],
    i4x4_modes: &mut [u8],
    mb_idx: usize,
    mb_width: usize,
) -> Result<(), String> {
    w.write_ue(0); // mb_type = 0 (I4x4)

    let qp = config.qp;

    // Quantize all 16 luma 4x4 blocks and compute cbp_luma.
    // cbp_luma bit i (0..4) = 1 if 8x8 group i has any non-zero coefficients.
    // 8x8 group mapping: group = by/2 * 2 + bx/2
    let mut cbp_luma = 0u32;
    let mut luma_coeffs = Vec::new();

    for by in 0..4 {
        for bx in 0..4 {
            let neighbors = intra::extract_neighbors_4x4(frame, mb.mb_x, mb.mb_y, bx, by);

            let mode = modes[by][bx];
            let pred = intra::predict_4x4(mode, &neighbors);

            let mut residual = [[0i16; 4]; 4];
            for r in 0..4 {
                for c in 0..4 {
                    residual[r][c] = mb.y[by * 4 + r][bx * 4 + c] as i16 - pred[r][c] as i16;
                }
            }

            let transformed = transform::forward_4x4(residual);
            let quantized = quantize::quantize_4x4(transformed, qp);

            let group = (by / 2) * 2 + (bx / 2);
            if !quantized.iter().all(|row| row.iter().all(|&c| c == 0)) {
                cbp_luma |= 1 << group;
            }

            luma_coeffs.push(quantized);
        }
    }

    // Write I4x4 prediction modes in scan order (matching decoder's luma4x4BlkIdx 0..15)
    write_i4x4_mode(w, &modes, i4x4_modes, mb_idx, mb_width);

    // intra_chroma_pred_mode (always DC = 0 for now) — must come before
    // coded_block_pattern per H.264 spec 7.3.5.
    w.write_ue(0);

    // Compute chroma DC
    let (cb_dc, cr_dc) = compute_chroma_dc(mb);

    let has_chroma_dc = !cb_dc.iter().all(|row| row.iter().all(|&c| c == 0))
        || !cr_dc.iter().all(|row| row.iter().all(|&c| c == 0));

    let cbp_chroma = if has_chroma_dc { 1 } else { 0 };

    // Write coded_block_pattern using the proper I-slice CBP table
    let cbp_code = cavlc::find_cbp_code(cbp_luma, cbp_chroma, true);
    w.write_ue(cbp_code);

    // Write mb_qp_delta when there are non-zero coefficients
    if cbp_luma > 0 || cbp_chroma > 0 {
        w.write_se(0); // mb_qp_delta = 0 (same as slice QP)
    }

    // Write luma residual blocks in scan order. Only blocks in an 8x8 group whose cbp
    // bit is set are present in the bitstream (matching the decoder, which skips the
    // rest); skipped blocks contribute nC = 0 to their neighbours.
    let zigzag_4x4: [(usize, usize); 16] = [
        (0, 0),
        (0, 1),
        (1, 0),
        (2, 0),
        (1, 1),
        (0, 2),
        (0, 3),
        (1, 2),
        (2, 1),
        (3, 0),
        (3, 1),
        (2, 2),
        (1, 3),
        (2, 3),
        (3, 2),
        (3, 3),
    ];
    for scan_blk in 0..16 {
        if cbp_luma & (1 << (scan_blk / 4)) == 0 {
            continue;
        }
        let coeffs = &luma_coeffs[SCAN_TO_RASTER[scan_blk]];
        let mut zigzagged = [0i16; 16];
        for i in 0..16 {
            let (r, c) = zigzag_4x4[i];
            zigzagged[i] = coeffs[r][c];
        }
        let nc = compute_nc_luma(nc_luma, mb_idx, mb_width, scan_blk);
        let tc = cavlc::write_residual_block(w, &zigzagged, cavlc::BlockType::Luma4x4, 16, nc);
        nc_luma[mb_idx * 16 + scan_blk] = tc as u8;
    }

    // Write chroma DC as a single interleaved block
    if has_chroma_dc {
        let interleaved = interleave_chroma_dc(&cb_dc, &cr_dc);
        cavlc::write_residual_block(w, &interleaved, cavlc::BlockType::ChromaDC, 4, -1);
    }

    Ok(())
}

fn write_i4x4_mode(
    w: &mut BitstreamWriter,
    modes: &[[Intra4x4Mode; 4]; 4],
    i4x4_modes: &mut [u8],
    mb_idx: usize,
    mb_width: usize,
) {
    let mode_to_val = |m: Intra4x4Mode| -> u8 {
        match m {
            Intra4x4Mode::Vertical => 0,
            Intra4x4Mode::Horizontal => 1,
            Intra4x4Mode::Dc => 2,
            Intra4x4Mode::DiagonalDownLeft => 3,
            Intra4x4Mode::DiagonalDownRight => 4,
            Intra4x4Mode::VerticalRight => 5,
            Intra4x4Mode::HorizontalDown => 6,
            Intra4x4Mode::VerticalLeft => 7,
            Intra4x4Mode::HorizontalUp => 8,
        }
    };

    // Left (A) and above (B) neighbor block indices per H.264 spec Table 6-4
    // and the decoder's get_neighbor_i4x4_mode().
    // Values are block/scan indices (0..15), NOT raster indices.
    // bool = true if same-MB neighbor, false if cross-MB.
    const LEFT: [(usize, bool); 16] = [
        (5, false),
        (0, true),
        (7, false),
        (2, true),
        (1, true),
        (4, true),
        (3, true),
        (6, true),
        (13, false),
        (8, true),
        (15, false),
        (10, true),
        (9, true),
        (12, true),
        (11, true),
        (14, true),
    ];
    const ABOVE: [(usize, bool); 16] = [
        (10, false),
        (11, false),
        (0, true),
        (1, true),
        (14, false),
        (15, false),
        (4, true),
        (5, true),
        (2, true),
        (3, true),
        (8, true),
        (9, true),
        (6, true),
        (7, true),
        (12, true),
        (13, true),
    ];

    for scan_idx in 0..16 {
        let raster = SCAN_TO_RASTER[scan_idx];
        let by = raster / 4;
        let bx = raster % 4;
        let actual = mode_to_val(modes[by][bx]);

        let (left_blk, left_same) = LEFT[scan_idx];
        let left_mode = if left_same {
            i4x4_modes[mb_idx * 16 + left_blk]
        } else if mb_idx % mb_width > 0 {
            i4x4_modes[(mb_idx - 1) * 16 + left_blk]
        } else {
            2
        };

        let (above_blk, above_same) = ABOVE[scan_idx];
        let above_mode = if above_same {
            i4x4_modes[mb_idx * 16 + above_blk]
        } else if mb_idx >= mb_width {
            i4x4_modes[(mb_idx - mb_width) * 16 + above_blk]
        } else {
            2
        };

        let predicted = left_mode.min(above_mode);

        if actual == predicted {
            w.write_bits(1, 1);
        } else {
            w.write_bits(0, 1);
            let rem = if actual < predicted {
                actual
            } else {
                actual - 1
            };
            w.write_bits(rem as u32, 3);
        }

        i4x4_modes[mb_idx * 16 + scan_idx] = actual;
    }
}

fn encode_i16x16_macroblock(
    w: &mut BitstreamWriter,
    mb: &Macroblock,
    mode: Intra16x16Mode,
    pred: [[u8; 16]; 16],
    qp: i8,
    nc_luma: &mut [u8],
    mb_idx: usize,
    mb_width: usize,
) -> Result<(), String> {
    // Compute luma residual
    let mut residual = [[0i16; 16]; 16];
    for row in 0..16 {
        for col in 0..16 {
            residual[row][col] = mb.y[row][col] as i16 - pred[row][col] as i16;
        }
    }

    // DCT each 4x4 block to get DC values and AC coefficients.
    // DC values are collected BEFORE quantization for the Hadamard step.
    let mut dc_values = [[0i16; 4]; 4]; // unquantized DCT DC from each 4x4 block
    let mut luma_ac = Vec::new(); // quantized AC coefficients (15 per block)
    let mut cbp_luma = 0u32;

    for by in 0..4 {
        for bx in 0..4 {
            let mut block = [[0i16; 4]; 4];
            for r in 0..4 {
                for c in 0..4 {
                    block[r][c] = residual[by * 4 + r][bx * 4 + c];
                }
            }
            let transformed = transform::forward_4x4(block);

            // Save the DC coefficient for Hadamard (already includes >> 1 from DCT)
            dc_values[by][bx] = transformed[0][0];

            // Quantize the AC coefficients (positions 1..15 in zigzag scan)
            let zigzag_4x4: [(usize, usize); 16] = [
                (0, 0),
                (0, 1),
                (1, 0),
                (2, 0),
                (1, 1),
                (0, 2),
                (0, 3),
                (1, 2),
                (2, 1),
                (3, 0),
                (3, 1),
                (2, 2),
                (1, 3),
                (2, 3),
                (3, 2),
                (3, 3),
            ];
            let mut ac_block = [0i16; 15];
            for i in 1..16 {
                let (r, c) = zigzag_4x4[i];
                // Quantize individual AC coefficient. The position class is
                // (r&1)+(c&1) ∈ {0=even-even, 1=mixed, 2=odd-odd}, matching the
                // decoder's dequant LEVEL_SCALE indexing; the MF rows are ordered to
                // pair with it. (The previous (r+c).min(3) classing mis-scaled most
                // AC positions.)
                let level = transformed[r][c] as i32;
                let pc = (r & 1) + (c & 1);
                let rem = (qp % 6) as usize;
                let div = (qp / 6) as i32;
                const MF: [[i32; 6]; 3] = [
                    [13107, 11916, 10082, 9362, 8192, 7282], // even-even
                    [8066, 7490, 6554, 5825, 5243, 4559],    // mixed
                    [5243, 4660, 4194, 3647, 3355, 2893],    // odd-odd
                ];
                let mf = MF[pc][rem];
                let shift = 15 + div;
                let offset = 1i32 << (shift - 1);
                let mag = (level.unsigned_abs() as i32 * mf + offset) >> shift;
                ac_block[i - 1] = if level < 0 { -mag } else { mag } as i16;
            }

            if !ac_block.iter().all(|&c| c == 0) {
                let group = (by / 2) * 2 + (bx / 2);
                cbp_luma |= 1 << group;
            }

            luma_ac.push(ac_block);
        }
    }

    // 4x4 Hadamard on the unquantized DC values (spec 8.5.5)
    let luma_dc_hadamard = transform::hadamard_4x4(dc_values);

    // Quantize the Hadamard DC coefficients with DC-specific quantizer
    let luma_dc_quantized = quantize::quantize_luma_dc_i16x16(luma_dc_hadamard, qp);

    // Compute chroma DC
    let (cb_dc, cr_dc) = compute_chroma_dc(mb);

    let has_chroma_dc = !cb_dc.iter().all(|row| row.iter().all(|&c| c == 0))
        || !cr_dc.iter().all(|row| row.iter().all(|&c| c == 0));

    let cbp_chroma = if has_chroma_dc { 1 } else { 0 };

    // I16x16 mb_type: mode + cbp_chroma * 4 + (cbp_luma ? 12 : 0) + 1
    let mode_idx = match mode {
        Intra16x16Mode::Vertical => 0u32,
        Intra16x16Mode::Horizontal => 1,
        Intra16x16Mode::Dc => 2,
        Intra16x16Mode::Plane => 3,
    };
    let mb_type = mode_idx + cbp_chroma * 4 + if cbp_luma > 0 { 12 } else { 0 } + 1;
    w.write_ue(mb_type);

    // intra_chroma_pred_mode (always DC = 0 for now)
    w.write_ue(0);

    // mb_qp_delta
    w.write_se(0);

    // Luma DC: 4x4 Hadamard-transformed DC values, written as a single block. Its nC
    // comes from block 0's neighbours but its own total_coeff is not stored (the DC
    // block does not participate in nC derivation for neighbouring AC blocks).
    let flat_dc: Vec<i16> = luma_dc_quantized
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect();
    let nc_dc = compute_nc_luma(nc_luma, mb_idx, mb_width, 0);
    cavlc::write_residual_block(w, &flat_dc, cavlc::BlockType::LumaDC16x16, 16, nc_dc);

    // Luma AC: 16 blocks of 15 coefficients in scan order, only when cbp_luma != 0.
    if cbp_luma > 0 {
        for scan_blk in 0..16 {
            let nc = compute_nc_luma(nc_luma, mb_idx, mb_width, scan_blk);
            let block = &luma_ac[SCAN_TO_RASTER[scan_blk]];
            let tc = cavlc::write_residual_block(w, block, cavlc::BlockType::Luma4x4, 15, nc);
            nc_luma[mb_idx * 16 + scan_blk] = tc as u8;
        }
    }

    // Chroma DC: interleaved Cb/Cr as a single 4-element block
    if has_chroma_dc {
        let interleaved = interleave_chroma_dc(&cb_dc, &cr_dc);
        cavlc::write_residual_block(w, &interleaved, cavlc::BlockType::ChromaDC, 4, -1);
    }

    Ok(())
}

/// Write a P-slice with inter prediction.
///
/// For each macroblock, chooses between P16x16 inter (motion-compensated)
/// and I16x16 intra based on SAD cost. Single reference frame (ref0).
pub fn write_p_slice(
    w: &mut BitstreamWriter,
    config: &H264EncoderConfig,
    frame: &DecodedFrame,
    ref_frame: &super::dpb::ReferenceFrame,
    frame_num: u32,
) -> Result<(), String> {
    use super::me;

    let mut slice_data = BitstreamWriter::new();

    // P-slice header (H.264 spec 7.3.3)
    slice_data.write_ue(0); // first_mb_in_slice
    slice_data.write_ue(0); // slice_type = P (0)
    slice_data.write_ue(0); // pic_parameter_set_id
    slice_data.write_bits(frame_num, 4); // frame_num (4 bits per SPS)
    // frame_mbs_only_flag=1, so field_pic_flag/bottom_field_flag are not present
    // Not IDR, so idr_pic_id is not present
    slice_data.write_bits(0, 8); // pic_order_cnt_lsb (8 bits per SPS)
    // bottom_field_pic_order_in_frame_present_flag=0, so delta_pic_order_cnt_bottom not present
    // Not B-slice, so direct_spatial_mv_pred_flag not present
    slice_data.write_bits(0, 1); // num_ref_idx_active_override_flag = 0 (use PPS defaults)
    slice_data.write_bits(0, 1); // ref_pic_list_modification_flag_l0 = 0 (no modifications)
    // weighted_pred_flag=0, so pred_weight_table not present
    // dec_ref_pic_marking: only present when nal_ref_idc != 0
    // For P-frames, nal_ref_idc = 0 in our encoder, so this is not written.
    // entropy_coding_mode_flag=0 (CAVLC), so cabac_init_idc not present

    let qp = config.qp as i32;
    slice_data.write_se(qp - 26); // slice_qp_delta
    slice_data.write_ue(1); // disable_deblocking_filter_idc = 1 (disabled)

    let (mb_width, mb_height) = config.mb_dims();
    let search_range = 16i16;

    // nC tracking for CAVLC
    let mut nc_luma = vec![0u8; (mb_width * mb_height) as usize * 16];

    // MV tracking for prediction
    let mut mb_mvs: Vec<me::MotionVector> =
        vec![me::MotionVector::ZERO; (mb_width * mb_height) as usize];

    for mb_y in 0..mb_height {
        for mb_x in 0..mb_width {
            let mb_idx = (mb_y * mb_width + mb_x) as usize;
            let mb = extract_macroblock(frame, mb_x, mb_y, config.qp);
            let src_x = mb_x as usize * 16;
            let src_y = mb_y as usize * 16;

            // Try P16x16 inter with half-pel refinement
            let search_result = me::search_p16x16_subpel(
                &frame.y,
                src_x,
                src_y,
                frame.width as usize,
                ref_frame,
                search_range,
            );

            // Predict MV from neighbors
            let left_mv = if mb_x > 0 {
                Some(mb_mvs[mb_idx - 1])
            } else {
                None
            };
            let top_mv = if mb_y > 0 {
                Some(mb_mvs[mb_idx - mb_width as usize])
            } else {
                None
            };
            let pred_mv = me::predict_mv(left_mv, top_mv, None);

            // Compute inter cost
            let inter_cost = search_result.cost + me::mv_cost(search_result.mv, pred_mv, 4);

            // Try I16x16 intra
            let neighbors = extract_luma_neighbors(frame, mb_x, mb_y);
            let (intra_mode, intra_pred) = intra::choose_best_i16x16_n(&mb, &neighbors);
            let intra_cost: u32 = (0..16)
                .flat_map(|row| (0..16).map(move |col| (row, col)))
                .map(|(row, col)| {
                    (mb.y[row][col] as i32 - intra_pred[row][col] as i32).unsigned_abs()
                })
                .sum();

            if inter_cost < intra_cost {
                // P16x16 inter MB
                mb_mvs[mb_idx] = search_result.mv;
                let ref_x = src_x as i32 + search_result.mv.dx as i32;
                let ref_y = src_y as i32 + search_result.mv.dy as i32;

                // Compute residual
                let mut residual = [[0i16; 16]; 16];
                for row in 0..16usize {
                    for col in 0..16usize {
                        let pred_px = ref_frame.luma(ref_x + col as i32, ref_y + row as i32);
                        residual[row][col] = mb.y[row][col] as i16 - pred_px as i16;
                    }
                }

                // DCT + quantize residual
                let mut luma_ac = Vec::new();
                let mut cbp_luma = 0u32;
                for by in 0..4 {
                    for bx in 0..4 {
                        let mut block = [[0i16; 4]; 4];
                        for r in 0..4 {
                            for c in 0..4 {
                                block[r][c] = residual[by * 4 + r][bx * 4 + c];
                            }
                        }
                        let transformed = transform::forward_4x4(block);
                        let quantized = quantize::quantize_4x4(transformed, config.qp);
                        if !quantized.iter().all(|row| row.iter().all(|&c| c == 0)) {
                            let group = (by / 2) * 2 + (bx / 2);
                            cbp_luma |= 1 << group;
                        }
                        luma_ac.push(quantized);
                    }
                }

                // Chroma residual: compute DC prediction and residual
                let mv = search_result.mv;

                let mut cb_dc = [[0i16; 2]; 2];
                let mut cr_dc = [[0i16; 2]; 2];
                for by in 0..2 {
                    for bx in 0..2 {
                        let mut dc_sum_cb = 0i16;
                        let mut dc_sum_cr = 0i16;
                        for r in 0..4 {
                            for c in 0..4 {
                                let ref_cx = (src_x / 2 + bx * 4 + c) as i32 + mv.dx as i32 / 2;
                                let ref_cy = (src_y / 2 + by * 4 + r) as i32 + mv.dy as i32 / 2;
                                let cb_src = mb.cb[by * 4 + r][bx * 4 + c] as i16;
                                let cr_src = mb.cr[by * 4 + r][bx * 4 + c] as i16;
                                let cb_ref = ref_frame.chroma(0, ref_cx, ref_cy) as i16;
                                let cr_ref = ref_frame.chroma(1, ref_cx, ref_cy) as i16;
                                dc_sum_cb += cb_src - cb_ref;
                                dc_sum_cr += cr_src - cr_ref;
                            }
                        }
                        cb_dc[by][bx] = dc_sum_cb / 4;
                        cr_dc[by][bx] = dc_sum_cr / 4;
                    }
                }

                let (cb_dc_t, cr_dc_t) = (
                    transform::hadamard_2x2(cb_dc),
                    transform::hadamard_2x2(cr_dc),
                );
                let has_chroma_dc = !cb_dc_t.iter().all(|row| row.iter().all(|&c| c == 0))
                    || !cr_dc_t.iter().all(|row| row.iter().all(|&c| c == 0));
                let cbp_chroma = if has_chroma_dc { 1 } else { 0 };

                // Write P16x16 MB
                // mb_type = 0 (P_L0_16x16) in P-slice: ref_idx_l0 + mvd_l0 + residual
                slice_data.write_ue(0); // mb_type = P_L0_16x16

                // ref_idx_l0 is only present when num_ref_idx_l0_active > 1
                // For single reference (default), it's not written.

                // mvd_l0
                let mvd_x = search_result.mv.dx as i32 - pred_mv.dx as i32;
                let mvd_y = search_result.mv.dy as i32 - pred_mv.dy as i32;
                slice_data.write_se(mvd_x);
                slice_data.write_se(mvd_y);

                // coded_block_pattern
                let cbp_code = cavlc::find_cbp_code(cbp_luma, cbp_chroma, false);
                slice_data.write_ue(cbp_code);

                // mb_qp_delta
                if cbp_luma > 0 {
                    slice_data.write_se(0);
                }

                // Write luma residual
                if cbp_luma > 0 {
                    for scan_blk in 0..16 {
                        if cbp_luma & (1 << (scan_blk / 4)) == 0 {
                            continue;
                        }
                        let coeffs = &luma_ac[SCAN_TO_RASTER[scan_blk]];
                        let zigzag_4x4: [(usize, usize); 16] = [
                            (0, 0),
                            (0, 1),
                            (1, 0),
                            (2, 0),
                            (1, 1),
                            (0, 2),
                            (0, 3),
                            (1, 2),
                            (2, 1),
                            (3, 0),
                            (3, 1),
                            (2, 2),
                            (1, 3),
                            (2, 3),
                            (3, 2),
                            (3, 3),
                        ];
                        let mut zigzagged = [0i16; 16];
                        for i in 0..16 {
                            let (r, c) = zigzag_4x4[i];
                            zigzagged[i] = coeffs[r][c];
                        }
                        let nc = compute_nc_luma(&nc_luma, mb_idx, mb_width as usize, scan_blk);
                        let tc = cavlc::write_residual_block(
                            &mut slice_data,
                            &zigzagged,
                            cavlc::BlockType::Luma4x4,
                            16,
                            nc,
                        );
                        nc_luma[mb_idx * 16 + scan_blk] = tc as u8;
                    }
                }
            } else {
                // I16x16 intra MB within P-slice
                let mode_idx = match intra_mode {
                    Intra16x16Mode::Vertical => 0u32,
                    Intra16x16Mode::Horizontal => 1,
                    Intra16x16Mode::Dc => 2,
                    Intra16x16Mode::Plane => 3,
                };

                let mut residual = [[0i16; 16]; 16];
                for row in 0..16 {
                    for col in 0..16 {
                        residual[row][col] = mb.y[row][col] as i16 - intra_pred[row][col] as i16;
                    }
                }

                let mut dc_values = [[0i16; 4]; 4];
                let mut luma_ac = Vec::new();
                let mut cbp_luma = 0u32;

                for by in 0..4 {
                    for bx in 0..4 {
                        let mut block = [[0i16; 4]; 4];
                        for r in 0..4 {
                            for c in 0..4 {
                                block[r][c] = residual[by * 4 + r][bx * 4 + c];
                            }
                        }
                        let transformed = transform::forward_4x4(block);
                        dc_values[by][bx] = transformed[0][0];

                        let zigzag_4x4: [(usize, usize); 16] = [
                            (0, 0),
                            (0, 1),
                            (1, 0),
                            (2, 0),
                            (1, 1),
                            (0, 2),
                            (0, 3),
                            (1, 2),
                            (2, 1),
                            (3, 0),
                            (3, 1),
                            (2, 2),
                            (1, 3),
                            (2, 3),
                            (3, 2),
                            (3, 3),
                        ];
                        let mut ac_block = [0i16; 15];
                        for i in 1..16 {
                            let (r, c) = zigzag_4x4[i];
                            let level = transformed[r][c] as i32;
                            let pc = (r & 1) + (c & 1);
                            let rem = (config.qp % 6) as usize;
                            let div = (config.qp / 6) as i32;
                            const MF: [[i32; 6]; 3] = [
                                [13107, 11916, 10082, 9362, 8192, 7282],
                                [8066, 7490, 6554, 5825, 5243, 4559],
                                [5243, 4660, 4194, 3647, 3355, 2893],
                            ];
                            let mf = MF[pc][rem];
                            let shift = 15 + div;
                            let offset = 1i32 << (shift - 1);
                            let mag = (level.unsigned_abs() as i32 * mf + offset) >> shift;
                            ac_block[i - 1] = if level < 0 { -mag } else { mag } as i16;
                        }
                        if !ac_block.iter().all(|&c| c == 0) {
                            let group = (by / 2) * 2 + (bx / 2);
                            cbp_luma |= 1 << group;
                        }
                        luma_ac.push(ac_block);
                    }
                }

                let luma_dc_hadamard = transform::hadamard_4x4(dc_values);
                let luma_dc_quantized =
                    quantize::quantize_luma_dc_i16x16(luma_dc_hadamard, config.qp);

                let (cb_dc, cr_dc) = compute_chroma_dc(&mb);
                let has_chroma_dc = !cb_dc.iter().all(|row| row.iter().all(|&c| c == 0))
                    || !cr_dc.iter().all(|row| row.iter().all(|&c| c == 0));
                let cbp_chroma = if has_chroma_dc { 1 } else { 0 };

                // I16x16 mb_type in P-slice: starts at 6
                let mb_type = 6 + mode_idx + cbp_chroma * 4 + if cbp_luma > 0 { 12 } else { 0 };
                slice_data.write_ue(mb_type);
                slice_data.write_ue(0); // intra_chroma_pred_mode = DC
                slice_data.write_se(0); // mb_qp_delta

                // Write luma DC
                let flat_dc: Vec<i16> = luma_dc_quantized
                    .iter()
                    .flat_map(|row| row.iter().copied())
                    .collect();
                let nc_dc = compute_nc_luma(&nc_luma, mb_idx, mb_width as usize, 0);
                cavlc::write_residual_block(
                    &mut slice_data,
                    &flat_dc,
                    cavlc::BlockType::LumaDC16x16,
                    16,
                    nc_dc,
                );

                // Write luma AC
                if cbp_luma > 0 {
                    for scan_blk in 0..16 {
                        let nc = compute_nc_luma(&nc_luma, mb_idx, mb_width as usize, scan_blk);
                        let block = &luma_ac[SCAN_TO_RASTER[scan_blk]];
                        let tc = cavlc::write_residual_block(
                            &mut slice_data,
                            block,
                            cavlc::BlockType::Luma4x4,
                            15,
                            nc,
                        );
                        nc_luma[mb_idx * 16 + scan_blk] = tc as u8;
                    }
                }

                // Write chroma DC
                if has_chroma_dc {
                    let interleaved = interleave_chroma_dc(&cb_dc, &cr_dc);
                    cavlc::write_residual_block(
                        &mut slice_data,
                        &interleaved,
                        cavlc::BlockType::ChromaDC,
                        4,
                        -1,
                    );
                }

                mb_mvs[mb_idx] = me::MotionVector::ZERO;
            }
        }
    }

    let slice_bytes = slice_data.take_rbsp_bytes();
    nal::write_nal_with_emulation_prevention(w, nal::NAL_TYPE_SLICE, 0, &slice_bytes);
    Ok(())
}

/// Write a B-slice with bi-directional prediction.
///
/// For each macroblock, chooses between:
/// - B_L0_16x16 (forward prediction from past reference)
/// - B_L1_16x16 (backward prediction from future reference)
/// - B_Bi_16x16 (bi-directional prediction)
/// - I16x16 intra (fallback)
pub fn write_b_slice(
    w: &mut BitstreamWriter,
    config: &H264EncoderConfig,
    frame: &DecodedFrame,
    ref_l0: &super::dpb::ReferenceFrame,
    ref_l1: &super::dpb::ReferenceFrame,
    frame_num: u32,
) -> Result<(), String> {
    use super::me;

    let mut slice_data = BitstreamWriter::new();

    // B-slice header
    slice_data.write_ue(0); // first_mb_in_slice
    slice_data.write_ue(1); // slice_type = B (1)
    slice_data.write_ue(0); // pic_parameter_set_id
    slice_data.write_bits(frame_num, 4); // frame_num

    // dec_ref_pic_marking: not an IDR
    slice_data.write_bits(0, 1); // adaptive_ref_pic_marking_mode_flag = 0

    let qp = config.qp as i32;
    slice_data.write_se(qp - 26); // slice_qp_delta
    slice_data.write_ue(1); // disable_deblocking_filter_idc = 1 (disabled)

    let (mb_width, mb_height) = config.mb_dims();
    let search_range = 16i16;

    // nC tracking for CAVLC
    let mut nc_luma = vec![0u8; (mb_width * mb_height) as usize * 16];

    // MV tracking for prediction
    let mut mb_mvs_l0: Vec<me::MotionVector> =
        vec![me::MotionVector::ZERO; (mb_width * mb_height) as usize];
    let mut mb_mvs_l1: Vec<me::MotionVector> =
        vec![me::MotionVector::ZERO; (mb_width * mb_height) as usize];

    for mb_y in 0..mb_height {
        for mb_x in 0..mb_width {
            let mb_idx = (mb_y * mb_width + mb_x) as usize;
            let mb = extract_macroblock(frame, mb_x, mb_y, config.qp);
            let src_x = mb_x as usize * 16;
            let src_y = mb_y as usize * 16;

            // Search for best B-frame prediction
            let b_result = me::search_bframe_p16x16(
                &frame.y,
                src_x,
                src_y,
                frame.width as usize,
                ref_l0,
                ref_l1,
                search_range,
            );

            // Predict MVs from neighbors
            let left_mv_l0 = if mb_x > 0 {
                Some(mb_mvs_l0[mb_idx - 1])
            } else {
                None
            };
            let top_mv_l0 = if mb_y > 0 {
                Some(mb_mvs_l0[mb_idx - mb_width as usize])
            } else {
                None
            };
            let pred_mv_l0 = me::predict_mv(left_mv_l0, top_mv_l0, None);

            let left_mv_l1 = if mb_x > 0 {
                Some(mb_mvs_l1[mb_idx - 1])
            } else {
                None
            };
            let top_mv_l1 = if mb_y > 0 {
                Some(mb_mvs_l1[mb_idx - mb_width as usize])
            } else {
                None
            };
            let pred_mv_l1 = me::predict_mv(left_mv_l1, top_mv_l1, None);

            // Try I16x16 intra
            let neighbors = extract_luma_neighbors(frame, mb_x, mb_y);
            let (intra_mode, intra_pred) = intra::choose_best_i16x16_n(&mb, &neighbors);
            let intra_cost: u32 = (0..16)
                .flat_map(|row| (0..16).map(move |col| (row, col)))
                .map(|(row, col)| {
                    (mb.y[row][col] as i32 - intra_pred[row][col] as i32).unsigned_abs()
                })
                .sum();

            // Choose best mode
            let best_inter_cost = match b_result.best_mode {
                0 => b_result.cost_l0,
                1 => b_result.cost_l1,
                _ => b_result.cost_bi,
            };

            if best_inter_cost < intra_cost {
                // Inter MB
                mb_mvs_l0[mb_idx] = b_result.mv_l0;
                mb_mvs_l1[mb_idx] = b_result.mv_l1;

                // Get prediction based on mode
                let (pred_l0, pred_l1) = match b_result.best_mode {
                    0 => {
                        // B_L0_16x16
                        let ref_x = src_x as i32 + b_result.mv_l0.dx as i32;
                        let ref_y = src_y as i32 + b_result.mv_l0.dy as i32;
                        (Some((ref_l0, ref_x, ref_y)), None)
                    }
                    1 => {
                        // B_L1_16x16
                        let ref_x = src_x as i32 + b_result.mv_l1.dx as i32;
                        let ref_y = src_y as i32 + b_result.mv_l1.dy as i32;
                        (None, Some((ref_l1, ref_x, ref_y)))
                    }
                    _ => {
                        // B_Bi_16x16
                        let ref_x_l0 = src_x as i32 + b_result.mv_l0.dx as i32;
                        let ref_y_l0 = src_y as i32 + b_result.mv_l0.dy as i32;
                        let ref_x_l1 = src_x as i32 + b_result.mv_l1.dx as i32;
                        let ref_y_l1 = src_y as i32 + b_result.mv_l1.dy as i32;
                        (
                            Some((ref_l0, ref_x_l0, ref_y_l0)),
                            Some((ref_l1, ref_x_l1, ref_y_l1)),
                        )
                    }
                };

                // Compute residual
                let mut residual = [[0i16; 16]; 16];
                for row in 0..16usize {
                    for col in 0..16usize {
                        let pred_px = if let Some((ref_frame, ref_x, ref_y)) = &pred_l0 {
                            if let Some((ref_frame_l1, ref_x_l1, ref_y_l1)) = &pred_l1 {
                                // Bi-directional: average
                                let p0 =
                                    ref_frame.luma(ref_x + col as i32, ref_y + row as i32) as i32;
                                let p1 = ref_frame_l1
                                    .luma(ref_x_l1 + col as i32, ref_y_l1 + row as i32)
                                    as i32;
                                ((p0 + p1 + 1) >> 1) as u8
                            } else {
                                ref_frame.luma(ref_x + col as i32, ref_y + row as i32)
                            }
                        } else if let Some((ref_frame, ref_x, ref_y)) = &pred_l1 {
                            ref_frame.luma(ref_x + col as i32, ref_y + row as i32)
                        } else {
                            128
                        };
                        residual[row][col] = mb.y[row][col] as i16 - pred_px as i16;
                    }
                }

                // DCT + quantize residual
                let mut luma_ac = Vec::new();
                let mut cbp_luma = 0u32;
                for by in 0..4 {
                    for bx in 0..4 {
                        let mut block = [[0i16; 4]; 4];
                        for r in 0..4 {
                            for c in 0..4 {
                                block[r][c] = residual[by * 4 + r][bx * 4 + c];
                            }
                        }
                        let transformed = transform::forward_4x4(block);
                        let quantized = quantize::quantize_4x4(transformed, config.qp);
                        if !quantized.iter().all(|row| row.iter().all(|&c| c == 0)) {
                            let group = (by / 2) * 2 + (bx / 2);
                            cbp_luma |= 1 << group;
                        }
                        luma_ac.push(quantized);
                    }
                }

                // Chroma residual: compute DC prediction and residual
                let mv_l0 = b_result.mv_l0;
                let mv_l1 = b_result.mv_l1;

                let mut cb_dc = [[0i16; 2]; 2];
                let mut cr_dc = [[0i16; 2]; 2];
                for by in 0..2 {
                    for bx in 0..2 {
                        let mut dc_sum_cb = 0i16;
                        let mut dc_sum_cr = 0i16;
                        for r in 0..4 {
                            for c in 0..4 {
                                let ref_cx_l0 =
                                    (src_x / 2 + bx * 4 + c) as i32 + mv_l0.dx as i32 / 2;
                                let ref_cy_l0 =
                                    (src_y / 2 + by * 4 + r) as i32 + mv_l0.dy as i32 / 2;
                                let ref_cx_l1 =
                                    (src_x / 2 + bx * 4 + c) as i32 + mv_l1.dx as i32 / 2;
                                let ref_cy_l1 =
                                    (src_y / 2 + by * 4 + r) as i32 + mv_l1.dy as i32 / 2;

                                let cb_src = mb.cb[by * 4 + r][bx * 4 + c] as i16;
                                let cr_src = mb.cr[by * 4 + r][bx * 4 + c] as i16;

                                // Bi-directional chroma prediction
                                let cb_ref_l0 = ref_l0.chroma(0, ref_cx_l0, ref_cy_l0) as i16;
                                let cr_ref_l0 = ref_l0.chroma(1, ref_cx_l0, ref_cy_l0) as i16;
                                let cb_ref_l1 = ref_l1.chroma(0, ref_cx_l1, ref_cy_l1) as i16;
                                let cr_ref_l1 = ref_l1.chroma(1, ref_cx_l1, ref_cy_l1) as i16;

                                let (cb_pred_px, cr_pred_px) = match b_result.best_mode {
                                    0 => (cb_ref_l0, cr_ref_l0),
                                    1 => (cb_ref_l1, cr_ref_l1),
                                    _ => (
                                        ((cb_ref_l0 + cb_ref_l1 + 1) >> 1),
                                        ((cr_ref_l0 + cr_ref_l1 + 1) >> 1),
                                    ),
                                };

                                dc_sum_cb += cb_src - cb_pred_px;
                                dc_sum_cr += cr_src - cr_pred_px;
                            }
                        }
                        cb_dc[by][bx] = dc_sum_cb / 4;
                        cr_dc[by][bx] = dc_sum_cr / 4;
                    }
                }

                let (cb_dc_t, cr_dc_t) = (
                    transform::hadamard_2x2(cb_dc),
                    transform::hadamard_2x2(cr_dc),
                );
                let has_chroma_dc = !cb_dc_t.iter().all(|row| row.iter().all(|&c| c == 0))
                    || !cr_dc_t.iter().all(|row| row.iter().all(|&c| c == 0));
                let cbp_chroma = if has_chroma_dc { 1 } else { 0 };

                // Write B-frame MB
                // mb_type depends on prediction mode:
                // B_L0_16x16 = 0, B_L1_16x16 = 1, B_Bi_16x16 = 2
                let mb_type = b_result.best_mode as u32;
                slice_data.write_ue(mb_type);

                // ref_idx_l0 and ref_idx_l1 are only present when num_ref_idx_active > 1
                // For single reference (default), they're not written.

                // mvd_l0 (motion vector difference)
                if b_result.best_mode == 0 || b_result.best_mode == 2 {
                    let mvd_x = b_result.mv_l0.dx as i32 - pred_mv_l0.dx as i32;
                    let mvd_y = b_result.mv_l0.dy as i32 - pred_mv_l0.dy as i32;
                    slice_data.write_se(mvd_x);
                    slice_data.write_se(mvd_y);
                }

                // mvd_l1
                if b_result.best_mode == 1 || b_result.best_mode == 2 {
                    let mvd_x = b_result.mv_l1.dx as i32 - pred_mv_l1.dx as i32;
                    let mvd_y = b_result.mv_l1.dy as i32 - pred_mv_l1.dy as i32;
                    slice_data.write_se(mvd_x);
                    slice_data.write_se(mvd_y);
                }

                // coded_block_pattern
                let cbp_code = cavlc::find_cbp_code(cbp_luma, cbp_chroma, false);
                slice_data.write_ue(cbp_code);

                // mb_qp_delta
                if cbp_luma > 0 {
                    slice_data.write_se(0);
                }

                // Write luma residual
                if cbp_luma > 0 {
                    for scan_blk in 0..16 {
                        if cbp_luma & (1 << (scan_blk / 4)) == 0 {
                            continue;
                        }
                        let coeffs = &luma_ac[SCAN_TO_RASTER[scan_blk]];
                        let zigzag_4x4: [(usize, usize); 16] = [
                            (0, 0),
                            (0, 1),
                            (1, 0),
                            (2, 0),
                            (1, 1),
                            (0, 2),
                            (0, 3),
                            (1, 2),
                            (2, 1),
                            (3, 0),
                            (3, 1),
                            (2, 2),
                            (1, 3),
                            (2, 3),
                            (3, 2),
                            (3, 3),
                        ];
                        let mut zigzagged = [0i16; 16];
                        for i in 0..16 {
                            let (r, c) = zigzag_4x4[i];
                            zigzagged[i] = coeffs[r][c];
                        }
                        let nc = compute_nc_luma(&nc_luma, mb_idx, mb_width as usize, scan_blk);
                        let tc = cavlc::write_residual_block(
                            &mut slice_data,
                            &zigzagged,
                            cavlc::BlockType::Luma4x4,
                            16,
                            nc,
                        );
                        nc_luma[mb_idx * 16 + scan_blk] = tc as u8;
                    }
                }
            } else {
                // I16x16 intra MB within B-slice
                // mb_type for intra in B-slice: starts at 23
                let mode_idx = match intra_mode {
                    Intra16x16Mode::Vertical => 0u32,
                    Intra16x16Mode::Horizontal => 1,
                    Intra16x16Mode::Dc => 2,
                    Intra16x16Mode::Plane => 3,
                };

                // Compute residual
                let mut residual = [[0i16; 16]; 16];
                for row in 0..16 {
                    for col in 0..16 {
                        residual[row][col] = mb.y[row][col] as i16 - intra_pred[row][col] as i16;
                    }
                }

                // DCT + quantize (simplified: just check if any non-zero)
                let mut cbp_luma = 0u32;
                for by in 0..4 {
                    for bx in 0..4 {
                        let mut block = [[0i16; 4]; 4];
                        for r in 0..4 {
                            for c in 0..4 {
                                block[r][c] = residual[by * 4 + r][bx * 4 + c];
                            }
                        }
                        let transformed = transform::forward_4x4(block);
                        let quantized = quantize::quantize_4x4(transformed, config.qp);
                        if !quantized.iter().all(|row| row.iter().all(|&c| c == 0)) {
                            let group = (by / 2) * 2 + (bx / 2);
                            cbp_luma |= 1 << group;
                        }
                    }
                }

                let cbp_chroma = 0u32;

                // I16x16 mb_type in B-slice: starts at 23
                let mb_type = 23 + mode_idx + cbp_chroma * 4 + if cbp_luma > 0 { 12 } else { 0 };
                slice_data.write_ue(mb_type);
                slice_data.write_ue(0); // intra_chroma_pred_mode = DC
                slice_data.write_se(0); // mb_qp_delta

                // Write luma residual (simplified: skip for now)
                // In a full implementation, we'd write the full I16x16 residual

                mb_mvs_l0[mb_idx] = me::MotionVector::ZERO;
                mb_mvs_l1[mb_idx] = me::MotionVector::ZERO;
            }
        }
    }

    let slice_bytes = slice_data.take_rbsp_bytes();
    nal::write_nal_with_emulation_prevention(w, nal::NAL_TYPE_SLICE, 0, &slice_bytes);
    Ok(())
}
