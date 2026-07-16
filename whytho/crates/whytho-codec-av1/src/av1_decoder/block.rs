//! AV1 block/partition tree decoder.
//!
//! Implements basic block partition parsing and prediction mode decoding.
//! Reference: AV1 spec section 6.10.1 (Decode partition syntax)

use super::sequence::BitReader;
use super::{BlockSize, PredictionMode, TxSize};

/// A decoded block with its prediction mode and transform info.
#[derive(Debug, Clone)]
pub struct DecodedBlock {
    pub block_size: BlockSize,
    pub prediction_mode: PredictionMode,
    pub tx_size: TxSize,
    pub row: u32,
    pub col: u32,
    pub width: u32,
    pub height: u32,
}

/// Block partition types (AV1 spec Table 6-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionType {
    None,       // single block (no split)
    Horizontal, // split horizontally into two
    Vertical,   // split vertically into two
    Split,      // split into four sub-blocks
}

/// Parse the partition tree for a superblock.
///
/// `sb_size` - superblock size (64 or 128)
/// `min_block_size` - minimum block size (4)
/// Returns a list of decoded blocks.
pub fn parse_partition_tree(
    r: &mut BitReader,
    sb_size: u32,
    row: u32,
    col: u32,
    min_block_size: u32,
) -> Result<Vec<DecodedBlock>, String> {
    let mut blocks = Vec::new();
    parse_partition_recursive(r, sb_size, row, col, sb_size, min_block_size, &mut blocks)?;
    Ok(blocks)
}

fn parse_partition_recursive(
    r: &mut BitReader,
    block_size: u32,
    row: u32,
    col: u32,
    sb_size: u32,
    min_block_size: u32,
    blocks: &mut Vec<DecodedBlock>,
) -> Result<(), String> {
    if block_size <= min_block_size {
        // Minimum size reached: this is a leaf block
        let bs = block_size_to_enum(block_size);
        let pred_mode = parse_prediction_mode(r, bs)?;
        let tx_size = parse_tx_size(r, bs);

        blocks.push(DecodedBlock {
            block_size: bs,
            prediction_mode: pred_mode,
            tx_size,
            row,
            col,
            width: block_size,
            height: block_size,
        });
        return Ok(());
    }

    // Read partition type
    let partition = parse_partition_type(r, block_size, row, col, sb_size)?;

    match partition {
        PartitionType::None => {
            // Single block at this size
            let bs = block_size_to_enum(block_size);
            let pred_mode = parse_prediction_mode(r, bs)?;
            let tx_size = parse_tx_size(r, bs);

            blocks.push(DecodedBlock {
                block_size: bs,
                prediction_mode: pred_mode,
                tx_size,
                row,
                col,
                width: block_size,
                height: block_size,
            });
        }
        PartitionType::Horizontal => {
            let half = block_size / 2;
            parse_partition_recursive(r, half, row, col, sb_size, min_block_size, blocks)?;
            parse_partition_recursive(r, half, row + half, col, sb_size, min_block_size, blocks)?;
        }
        PartitionType::Vertical => {
            let half = block_size / 2;
            parse_partition_recursive(r, half, row, col, sb_size, min_block_size, blocks)?;
            parse_partition_recursive(r, half, row, col + half, sb_size, min_block_size, blocks)?;
        }
        PartitionType::Split => {
            let half = block_size / 2;
            parse_partition_recursive(r, half, row, col, sb_size, min_block_size, blocks)?;
            parse_partition_recursive(r, half, row, col + half, sb_size, min_block_size, blocks)?;
            parse_partition_recursive(r, half, row + half, col, sb_size, min_block_size, blocks)?;
            parse_partition_recursive(
                r,
                half,
                row + half,
                col + half,
                sb_size,
                min_block_size,
                blocks,
            )?;
        }
    }

    Ok(())
}

/// Parse partition type from the bitstream.
///
/// The partition probability depends on block size and position.
fn parse_partition_type(
    r: &mut BitReader,
    block_size: u32,
    row: u32,
    col: u32,
    sb_size: u32,
) -> Result<PartitionType, String> {
    // Simplified: use fixed probabilities
    // In a full implementation, this would use CDF-based arithmetic coding
    let split = r.read_bit()? != 0;

    if split {
        Ok(PartitionType::Split)
    } else if block_size > 8 {
        // For larger blocks, also allow H/V splits
        let hv = r.read_bit()? != 0;
        if hv {
            Ok(PartitionType::Horizontal)
        } else {
            Ok(PartitionType::Vertical)
        }
    } else {
        Ok(PartitionType::None)
    }
}

/// Parse intra prediction mode for a block.
fn parse_prediction_mode(r: &mut BitReader, bs: BlockSize) -> Result<PredictionMode, String> {
    // Simplified: use a fixed probability distribution
    // In a full implementation, this would use CDF-based arithmetic coding
    let mode_idx = r.read_bits(4)? as u8;

    Ok(match mode_idx {
        0 => PredictionMode::DcPred,
        1 => PredictionMode::VPred,
        2 => PredictionMode::HPred,
        3 => PredictionMode::D45Pred,
        4 => PredictionMode::D135Pred,
        5 => PredictionMode::D113Pred,
        6 => PredictionMode::D157Pred,
        7 => PredictionMode::D203Pred,
        8 => PredictionMode::D67Pred,
        9 => PredictionMode::SmoothPred,
        10 => PredictionMode::SmoothVPred,
        11 => PredictionMode::SmoothHPred,
        12 => PredictionMode::PaethPred,
        _ => PredictionMode::DcPred,
    })
}

/// Parse transform size for a block.
fn parse_tx_size(r: &mut BitReader, bs: BlockSize) -> TxSize {
    // Simplified: use block size to determine transform size
    match bs {
        BlockSize::Bs4x4 | BlockSize::Bs4x8 | BlockSize::Bs8x4 => TxSize::Tx4x4,
        BlockSize::Bs8x8 | BlockSize::Bs8x16 | BlockSize::Bs16x8 => TxSize::Tx8x8,
        BlockSize::Bs16x16 | BlockSize::Bs16x32 | BlockSize::Bs32x16 => TxSize::Tx16x16,
        _ => TxSize::Tx32x32,
    }
}

/// Convert pixel size to BlockSize enum.
fn block_size_to_enum(size: u32) -> BlockSize {
    match size {
        4 => BlockSize::Bs4x4,
        8 => BlockSize::Bs8x8,
        16 => BlockSize::Bs16x16,
        32 => BlockSize::Bs32x32,
        64 => BlockSize::Bs64x64,
        128 => BlockSize::Bs128x128,
        _ => BlockSize::Bs64x64,
    }
}

/// Get block dimensions from BlockSize enum.
pub fn block_dimensions(bs: BlockSize) -> (u32, u32) {
    match bs {
        BlockSize::Bs4x4 => (4, 4),
        BlockSize::Bs4x8 => (4, 8),
        BlockSize::Bs8x4 => (8, 4),
        BlockSize::Bs8x8 => (8, 8),
        BlockSize::Bs8x16 => (8, 16),
        BlockSize::Bs16x8 => (16, 8),
        BlockSize::Bs16x16 => (16, 16),
        BlockSize::Bs16x32 => (16, 32),
        BlockSize::Bs32x16 => (32, 16),
        BlockSize::Bs32x32 => (32, 32),
        BlockSize::Bs32x64 => (32, 64),
        BlockSize::Bs64x32 => (64, 32),
        BlockSize::Bs64x64 => (64, 64),
        BlockSize::Bs128x128 => (128, 128),
        BlockSize::Bs128x64 => (128, 64),
        BlockSize::Bs64x128 => (64, 128),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_size_to_enum_and_back() {
        for size in [4, 8, 16, 32, 64, 128] {
            let bs = block_size_to_enum(size);
            let (w, h) = block_dimensions(bs);
            assert_eq!(w, size);
            assert_eq!(h, size);
        }
    }

    #[test]
    fn block_dimensions_asymmetric() {
        let (w, h) = block_dimensions(BlockSize::Bs4x8);
        assert_eq!(w, 4);
        assert_eq!(h, 8);

        let (w, h) = block_dimensions(BlockSize::Bs32x16);
        assert_eq!(w, 32);
        assert_eq!(h, 16);
    }
}
