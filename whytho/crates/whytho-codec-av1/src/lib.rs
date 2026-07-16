#![forbid(unsafe_code)]

//! AV1 codec for `whytho.`: a from-scratch decoder and a `rav1e`-backed encoder.
//!
//! Depends only on the `whytho-types` contract (and the `rav1e`/`v_frame` backend), never on
//! the `whytho-core` app policy.

// Re-export the codec contract so the `use crate::{...}` / `use super::{...}` paths inside the
// modules resolve.
pub use whytho_types::*;

pub mod av1_decoder;
pub mod av1_encoder;

pub use av1_decoder::Av1Decoder;
pub use av1_encoder::Av1Encoder;
