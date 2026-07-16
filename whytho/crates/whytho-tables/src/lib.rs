//! Constant data tables for AV2.
//!
//! Generated from `av2-spec/v1.0.0/attachments/*.h` by `cargo run -p xtask -- gen-tables`.
//! Sources: default CDF tables, forward transform kernels (`*_kernel*.h`), coefficient
//! scan orders, transform-size LUTs, and quantizer matrices.
#![forbid(unsafe_code)]
#![allow(clippy::all)]

// Populated by `cargo run -p xtask -- gen-tables`; starts empty so the workspace builds
// (and `cargo fmt`/clippy work) before the tables are generated.
pub mod generated;

/// Selects the AV2 coefficient-CDF quantizer context for a base qindex.
pub const fn coefficient_q_context(qindex: u16) -> usize {
    if qindex <= 90 {
        0
    } else if qindex <= 140 {
        1
    } else if qindex <= 190 {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::{coefficient_q_context, generated};

    #[test]
    fn generated_dct_kernel_matches_spec_attachment() {
        assert_eq!(generated::dct_kernel4::DCT_KERNEL4[1], [83, 35, -35, -83]);
    }

    #[test]
    fn generated_cdf_matches_spec_attachment() {
        assert_eq!(
            generated::default_skip_cdf::DEFAULT_SKIP_CDF[0],
            [25865, 25, 0]
        );
        assert_eq!(
            generated::default_skip_cdf::DEFAULT_SKIP_CDF[5],
            [3320, 90, 0]
        );
    }

    #[test]
    fn generated_symbolic_values_are_resolved() {
        assert_eq!(generated::adjusted_tx_size::ADJUSTED_TX_SIZE[0], 0);
        assert_eq!(generated::adjusted_tx_size::ADJUSTED_TX_SIZE[24], 20);
        assert_eq!(generated::partition_subsize::PARTITION_SUBSIZE[1][0], 255);
        assert_eq!(generated::mode_to_txfm::MODE_TO_TXFM[4], 3);
    }

    #[test]
    fn coefficient_q_context_boundaries_match_reference() {
        assert_eq!(coefficient_q_context(0), 0);
        assert_eq!(coefficient_q_context(90), 0);
        assert_eq!(coefficient_q_context(91), 1);
        assert_eq!(coefficient_q_context(140), 1);
        assert_eq!(coefficient_q_context(141), 2);
        assert_eq!(coefficient_q_context(190), 2);
        assert_eq!(coefficient_q_context(191), 3);
        assert_eq!(coefficient_q_context(u16::MAX), 3);
    }
}
