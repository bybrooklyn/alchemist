//! The AV2 encode pipeline.
//!
//! Module layout mirrors the AVM reference encoder (`avm/av2/encoder/*`). The bitstream
//! and entropy substrate are real; the prediction/transform/RD stages are wired stubs
//! that are filled in stage by stage. `unsafe` is confined to `whytho-dsp`.
#![forbid(unsafe_code)]

pub mod adapter; // implements the whytho-types `VideoEncoder` contract over `Encoder`
pub mod bitstream; // <- avm/av2/encoder/bitstream.c (OBU + headers; implemented for real)
pub mod common; // <- avm/av2/common/{enums.h,blockd.h} (was its own crate, now merged in)
pub mod encoder; // <- avm/av2/encoder/{encoder.c,encode_strategy.c}
pub mod encodetxb; // <- avm/av2/encoder/encodetxb.c
pub mod frame; // <- avm/av2/encoder/encodeframe.c
pub mod inter; // <- avm/av2/encoder/{rdopt.c,mcomp.c}
pub mod intra; // <- avm/av2/encoder/intra_mode_search.c
pub mod loopfilter; // <- avm/av2/encoder/{picklpf,pickcdef,pickrst,pickccso}.c
pub mod partition; // <- avm/av2/encoder/partition_search.c
pub mod quantize; // <- avm/av2/encoder/av2_quantize.c
pub mod ratectrl; // <- fixed-QP passthrough first
pub mod tokenize; // <- avm/av2/encoder/tokenize.c
pub mod transform; // <- avm/av2/encoder/{av2_fwd_txfm2d,hybrid_fwd_txfm}.c

pub use adapter::Av2Encoder;
pub use encoder::{Config, EncodeError, Encoder};
