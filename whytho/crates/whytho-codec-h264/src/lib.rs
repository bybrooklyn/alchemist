#![forbid(unsafe_code)]

//! H.264 encoder and decoder for `whytho.`.
//!
//! Depends only on the `whytho-types` contract (and the `rust_h264` backend), never on the
//! `whytho-core` app policy. SIMD/`unsafe` DSP belongs in `whytho-dsp`, not here.

// Re-export the codec contract so the `use crate::{...}` paths inside the modules resolve.
pub use whytho_types::*;

pub mod h264_decoder;
pub mod h264_encoder;

pub use h264_decoder::H264Decoder;
pub use h264_encoder::{H264Encoder, H264EncoderConfig};
