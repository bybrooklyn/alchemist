//! Frame/tile/superblock-row iteration.
//!
//! Reference: `avm/av2/encoder/encodeframe.c` (`encode_sb_row`, `av2_encode_tile`).

use crate::encodetxb::EncodedTxBlock;
use crate::intra::IntraPlan;
use crate::partition::PartitionPlan;
use crate::transform::TransformPlan;

/// One-tile frame plan used by the first skeleton encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FramePlan {
    /// Fixed partition decision.
    pub partition: PartitionPlan,
    /// Fixed intra decision.
    pub intra: IntraPlan,
    /// Fixed transform decision.
    pub transform: TransformPlan,
    /// Encoded transform block summary.
    pub txb: EncodedTxBlock,
}

/// Build the one-superblock frame plan.
pub fn build_frame_plan(
    partition: PartitionPlan,
    intra: IntraPlan,
    transform: TransformPlan,
    txb: EncodedTxBlock,
) -> FramePlan {
    FramePlan {
        partition,
        intra,
        transform,
        txb,
    }
}
