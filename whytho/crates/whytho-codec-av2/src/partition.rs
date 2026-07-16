//! Superblock partition RD search.
//!
//! Reference: `avm/av2/encoder/partition_search.c` (`av2_rd_pick_partition`). Starts with
//! PARTITION_NONE only and grows to the full extended-split set.

use crate::common::enums::{BlockSize, PartitionType};

/// Fixed partition decision for the first decodable keyframe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PartitionPlan {
    /// Superblock-sized luma block.
    pub block: BlockSize,
    /// The only supported partition in the skeleton.
    pub partition: PartitionType,
}

/// Select the fixed one-superblock partition.
pub fn pick_partition() -> PartitionPlan {
    PartitionPlan {
        block: BlockSize::BLOCK_128X128,
        partition: PartitionType::PARTITION_NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_uses_one_unsplit_superblock() {
        let plan = pick_partition();
        assert_eq!(plan.block, BlockSize::BLOCK_128X128);
        assert_eq!(plan.partition, PartitionType::PARTITION_NONE);
    }
}
