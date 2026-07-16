//! Forward transform facade over `whytho-dsp::transform`.
//!
//! Reference: `avm/av2/encoder/{av2_fwd_txfm2d.c,hybrid_fwd_txfm.c}`.

use crate::common::enums::{TxSize, TxType};

/// Transform decision for the zero-residual skeleton path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransformPlan {
    /// Transform size.
    pub tx_size: TxSize,
    /// Transform type.
    pub tx_type: TxType,
}

/// Pick the largest square transform. Coefficients are all zero in this skeleton.
pub fn pick_transform() -> TransformPlan {
    TransformPlan {
        tx_size: TxSize::TX_64X64,
        tx_type: TxType::DCT_DCT,
    }
}
