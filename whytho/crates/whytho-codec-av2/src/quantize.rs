//! Quantization facade over `whytho-dsp::quantize` with dequant tables from `whytho-tables`.
//!
//! Reference: `avm/av2/encoder/av2_quantize.c`, `avm/av2/common/quant_common.h`.

/// Quantized transform block for the fixed zero-residual skeleton.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedBlock {
    /// Base quantization index signalled in the frame header.
    pub base_qindex: u8,
    /// Whether every coefficient is zero.
    pub all_zero: bool,
}

/// Produce an all-zero quantized block.
pub fn quantize_zero(base_qindex: u8) -> QuantizedBlock {
    QuantizedBlock {
        base_qindex,
        all_zero: true,
    }
}
