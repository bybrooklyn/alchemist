//! Core enums and shared types for the AV2 encoder.
//!
//! Ported from `avm/av2/common/{enums.h,blockd.h}` and `avm/av2/encoder/enc_enums.h`.
//! Pure, `unsafe`-free data plumbing shared across the pipeline. Was previously its
//! own crate (`whytho-codec-av2-common`); merged into `whytho-codec-av2` since
//! nothing else depended on it separately.

pub mod enums;
pub mod image;
