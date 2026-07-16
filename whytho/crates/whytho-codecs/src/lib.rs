#![forbid(unsafe_code)]

//! Codec facade for `whytho.`.
//!
//! This crate owns no codec logic. It re-exports the [`whytho_types`] contract and, behind cargo
//! features, the individual `whytho-codec-*` crates, plus a capability registry describing which
//! codecs the current build can encode/decode. Consumers (CLI, mux, app) depend on this crate and
//! select codecs via features rather than reaching into concrete codec crates.
//!
//! Default features build every codec; disable defaults and opt in (e.g. `--features h264`) for a
//! slimmer build.

// The full codec contract: frame/packet types, encoder/decoder traits, codec enums, capability
// records. Re-exported so `whytho_codecs::{VideoEncoder, DecodedFrame, VideoCodec, ...}` resolves.
pub use whytho_types::*;

// ---------------------------------------------------------------------------
// Codec crates (feature-gated), exposed both as modules and as flattened types.
// ---------------------------------------------------------------------------

#[cfg(feature = "h264")]
pub use whytho_codec_h264 as h264;
#[cfg(feature = "h264")]
pub use whytho_codec_h264::{H264Decoder, H264Encoder, H264EncoderConfig};

#[cfg(feature = "av1")]
pub use whytho_codec_av1 as av1;
#[cfg(feature = "av1")]
pub use whytho_codec_av1::{Av1Decoder, Av1Encoder};

#[cfg(feature = "opus")]
pub use whytho_codec_opus as opus;
#[cfg(feature = "opus")]
pub use whytho_codec_opus::WhythoOpusEncoder;

#[cfg(feature = "av2")]
pub use whytho_codec_av2 as av2;
#[cfg(feature = "av2")]
pub use whytho_codec_av2::Av2Encoder;

// ---------------------------------------------------------------------------
// Capability registry
// ---------------------------------------------------------------------------

/// Video codecs this build can encode or decode, in registration order.
///
/// Each entry is gated on the codec's feature, so the registry reflects exactly what was compiled
/// in. AV2 is encode-only today.
pub fn video_capabilities() -> Vec<VideoCodecCapability> {
    #[allow(unused_mut)]
    let mut caps = Vec::new();

    #[cfg(feature = "h264")]
    {
        caps.push(VideoCodecCapability::new(
            "h264",
            VideoCodec::H264,
            CodecDirection::Encode,
        ));
        caps.push(VideoCodecCapability::new(
            "h264",
            VideoCodec::H264,
            CodecDirection::Decode,
        ));
    }

    #[cfg(feature = "av1")]
    {
        caps.push(VideoCodecCapability::new(
            "rav1e",
            VideoCodec::Av1,
            CodecDirection::Encode,
        ));
        caps.push(VideoCodecCapability::new(
            "av1",
            VideoCodec::Av1,
            CodecDirection::Decode,
        ));
    }

    #[cfg(feature = "av2")]
    {
        caps.push(VideoCodecCapability::new(
            "av2",
            VideoCodec::Av2,
            CodecDirection::Encode,
        ));
    }

    caps
}

/// Audio codecs this build can encode or decode, in registration order.
pub fn audio_capabilities() -> Vec<AudioCodecCapability> {
    #[allow(unused_mut)]
    let mut caps = Vec::new();

    #[cfg(feature = "opus")]
    caps.push(AudioCodecCapability::new(
        "opus",
        AudioCodec::Opus,
        CodecDirection::Encode,
    ));

    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_capability_records_codec_direction() {
        let capability =
            VideoCodecCapability::new("rav1e", VideoCodec::Av1, CodecDirection::Encode);

        assert_eq!(capability.name, "rav1e");
        assert_eq!(capability.codec, VideoCodec::Av1);
        assert_eq!(capability.direction, CodecDirection::Encode);
    }

    #[cfg(feature = "av2")]
    #[test]
    fn registry_includes_av2_encoder() {
        let caps = video_capabilities();
        assert!(
            caps.iter()
                .any(|c| c.codec == VideoCodec::Av2 && c.direction == CodecDirection::Encode),
            "AV2 encoder should be wired into the capability registry"
        );
    }

    #[cfg(all(feature = "h264", feature = "av1"))]
    #[test]
    fn registry_lists_h264_and_av1_both_directions() {
        let caps = video_capabilities();
        for codec in [VideoCodec::H264, VideoCodec::Av1] {
            let has = |dir| caps.iter().any(|c| c.codec == codec && c.direction == dir);
            assert!(has(CodecDirection::Encode), "{codec} encode missing");
            assert!(has(CodecDirection::Decode), "{codec} decode missing");
        }
    }
}
