//! Transform-block coefficient entropy coding.
//!
//! Reference: `avm/av2/encoder/encodetxb.c`.

use crate::tokenize::TokenizedBlock;

/// Entropy payload summary for the fixed skeleton tile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodedTxBlock {
    /// Whether no coefficient symbols are needed beyond skip/eob signalling.
    pub all_zero: bool,
}

/// Encode the tokenized all-zero block summary.
pub fn encode_txb(block: TokenizedBlock) -> EncodedTxBlock {
    EncodedTxBlock {
        all_zero: block.all_zero,
    }
}
