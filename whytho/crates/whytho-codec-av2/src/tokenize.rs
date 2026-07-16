//! Coefficient tokenization.
//!
//! Reference: `avm/av2/encoder/tokenize.c`.

use crate::quantize::QuantizedBlock;

/// Token stream summary for one transform block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenizedBlock {
    /// Whether the block has no non-zero coefficient tokens.
    pub all_zero: bool,
}

/// Tokenize the skeleton all-zero block.
pub fn tokenize(block: QuantizedBlock) -> TokenizedBlock {
    TokenizedBlock {
        all_zero: block.all_zero,
    }
}
