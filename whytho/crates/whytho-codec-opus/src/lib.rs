#![forbid(unsafe_code)]

//! Opus audio encoder for `whytho.`.
//!
//! Depends only on the `whytho-types` contract (and the `opus-rs` backend), never on the
//! `whytho-core` app policy.

// Re-export the codec contract so the `use super::{...}` paths inside the module resolve.
pub use whytho_types::*;

pub mod opus_encoder;

pub use opus_encoder::WhythoOpusEncoder;
