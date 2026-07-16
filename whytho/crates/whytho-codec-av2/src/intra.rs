//! Intra prediction mode RD search.
//!
//! Reference: `avm/av2/encoder/intra_mode_search.c`. Starts with DC_PRED and grows to the
//! full directional/smooth/paeth/IBP/DIP set.

use crate::common::enums::PredictionMode;

/// Fixed intra prediction decision for the first skeleton frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntraPlan {
    /// Luma/chroma prediction mode.
    pub mode: PredictionMode,
}

/// Select DC prediction for every plane.
pub fn pick_intra_mode() -> IntraPlan {
    IntraPlan {
        mode: PredictionMode::DC_PRED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_uses_dc_prediction() {
        assert_eq!(pick_intra_mode().mode, PredictionMode::DC_PRED);
    }
}
