//! AV1 tile group header decoder.
//!
//! Parses tile group OBU headers to determine tile start/end positions.
//! Reference: AV1 spec section 6.10.1 (Tile group OBU syntax)

use super::FrameHeader;
use super::sequence::BitReader;

/// Parsed tile group information.
#[derive(Debug, Clone)]
pub struct TileGroupInfo {
    /// Number of tiles in this group.
    pub num_tiles: u32,
    /// Start tile index (inclusive).
    pub tile_start: u32,
    /// End tile index (inclusive).
    pub tile_end: u32,
    /// Byte offset to tile data (after header).
    pub tile_data_offset: usize,
}

/// Parse tile group OBU header.
///
/// `data` - OBU payload (after OBU header)
/// `fh` - frame header (needed for tile info)
/// `num_tiles` - total number of tiles in the frame
pub fn decode_tile_group_header(
    data: &[u8],
    fh: &FrameHeader,
    num_tiles: u32,
) -> Result<TileGroupInfo, String> {
    if data.is_empty() {
        return Err("empty tile group data".into());
    }

    let mut r = BitReader::new(data);

    // tg_start and tg_end are present when num_tiles > 1
    let (tile_start, tile_end) = if num_tiles > 1 {
        let tile_bits = (num_tiles as f64).log2().ceil() as u32;
        let start = r.read_bits(tile_bits)?;
        let end = r.read_bits(tile_bits)?;
        (start, end)
    } else {
        (0, 0)
    };

    // Tile group size (number of bytes in tile data)
    // This is encoded as a variable-length integer
    let tile_data_size = if num_tiles > 1 {
        r.read_leb128()? as usize
    } else {
        // For single tile, the rest of the data is the tile
        data.len() - (r.bits_consumed() as usize / 8)
    };

    let tile_data_offset = r.bits_consumed() as usize / 8;

    Ok(TileGroupInfo {
        num_tiles,
        tile_start,
        tile_end,
        tile_data_offset,
    })
}

/// Calculate the number of tiles from frame dimensions and tile info.
pub fn calculate_num_tiles(fh: &FrameHeader) -> u32 {
    let tile_cols = 1 << fh.tile_info.tile_cols_log2;
    let tile_rows = 1 << fh.tile_info.tile_rows_log2;
    tile_cols * tile_rows
}

/// Calculate tile dimensions in superblocks.
pub fn tile_dimensions(
    fh: &FrameHeader,
    tile_col: u32,
    tile_row: u32,
    sb_size: u32,
) -> (u32, u32, u32, u32) {
    let frame_width_sb = (fh.width + sb_size - 1) / sb_size;
    let frame_height_sb = (fh.height + sb_size - 1) / sb_size;

    let tile_cols = 1 << fh.tile_info.tile_cols_log2;
    let tile_rows = 1 << fh.tile_info.tile_rows_log2;

    // Tile start positions in superblocks
    let tile_start_sb_x = (tile_col * frame_width_sb) / tile_cols;
    let tile_start_sb_y = (tile_row * frame_height_sb) / tile_rows;
    let tile_end_sb_x = ((tile_col + 1) * frame_width_sb) / tile_cols;
    let tile_end_sb_y = ((tile_row + 1) * frame_height_sb) / tile_rows;

    let tile_width_sb = tile_end_sb_x - tile_start_sb_x;
    let tile_height_sb = tile_end_sb_y - tile_start_sb_y;

    (
        tile_start_sb_x,
        tile_start_sb_y,
        tile_width_sb,
        tile_height_sb,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_tile() {
        let fh = FrameHeader {
            frame_type: super::super::FrameType::KeyFrame,
            show_existing_frame: false,
            frame_to_show: 0,
            show_frame: true,
            showable_frame: false,
            error_resilient_mode: false,
            width: 640,
            height: 480,
            render_width: 640,
            render_height: 480,
            superres_denom: 8,
            upscaled_width: 640,
            use_superres: false,
            frame_offset: 0,
            quantization_params: Default::default(),
            segmentation_params: Default::default(),
            delta_q_params: Default::default(),
            loop_filter_params: Default::default(),
            cdef_params: Default::default(),
            lr_params: Default::default(),
            tile_info: super::super::TileInfo {
                tile_cols_log2: 0,
                tile_rows_log2: 0,
            },
        };

        let num_tiles = calculate_num_tiles(&fh);
        assert_eq!(num_tiles, 1);

        let (x, y, w, h) = tile_dimensions(&fh, 0, 0, 64);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
        // 640/64 = 10 SBs wide, 480/64 = 7.5 → 8 SBs high
        assert_eq!(w, 10);
        assert_eq!(h, 8);
    }

    #[test]
    fn multiple_tiles() {
        let fh = FrameHeader {
            frame_type: super::super::FrameType::KeyFrame,
            show_existing_frame: false,
            frame_to_show: 0,
            show_frame: true,
            showable_frame: false,
            error_resilient_mode: false,
            width: 1920,
            height: 1080,
            render_width: 1920,
            render_height: 1080,
            superres_denom: 8,
            upscaled_width: 1920,
            use_superres: false,
            frame_offset: 0,
            quantization_params: Default::default(),
            segmentation_params: Default::default(),
            delta_q_params: Default::default(),
            loop_filter_params: Default::default(),
            cdef_params: Default::default(),
            lr_params: Default::default(),
            tile_info: super::super::TileInfo {
                tile_cols_log2: 1, // 2 columns
                tile_rows_log2: 1, // 2 rows
            },
        };

        let num_tiles = calculate_num_tiles(&fh);
        assert_eq!(num_tiles, 4);

        // Check tile 0 (top-left)
        let (x, y, w, h) = tile_dimensions(&fh, 0, 0, 64);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
        // 1920/64 = 30 SBs, 30/2 = 15 SBs per tile column
        assert_eq!(w, 15);
        // 1080/64 = 16.875 → 17 SBs, 17/2 ≈ 8-9
        assert!(h >= 8 && h <= 9);
    }

    #[test]
    fn tile_group_info_single() {
        let data = [0u8; 10]; // dummy tile data
        let fh = FrameHeader {
            frame_type: super::super::FrameType::KeyFrame,
            show_existing_frame: false,
            frame_to_show: 0,
            show_frame: true,
            showable_frame: false,
            error_resilient_mode: false,
            width: 640,
            height: 480,
            render_width: 640,
            render_height: 480,
            superres_denom: 8,
            upscaled_width: 640,
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

        let info = decode_tile_group_header(&data, &fh, 1).unwrap();
        assert_eq!(info.num_tiles, 1);
        assert_eq!(info.tile_start, 0);
        assert_eq!(info.tile_end, 0);
    }
}
