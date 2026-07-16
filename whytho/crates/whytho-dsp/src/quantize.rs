//! Quantization kernels.
//!
//! Re-exports the shared quantize/dequantize functions from the transform module.
//! These are the H.264-compatible 4x4 quantizer used by multiple codecs.

pub use crate::transform::{dequantize_4x4, quantize_4x4};
