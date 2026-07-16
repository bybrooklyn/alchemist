# Opus

## Specification

- **Standard**: Definition of the Opus Audio Codec
- **Organization**: IETF
- **Spec URL**: https://www.rfc-editor.org/rfc/rfc6716
- **RFC 6716 Text**: https://www.rfc-editor.org/rfc/rfc6716.txt ✅ (freely available)
- **Ogg Encapsulation**: RFC 7845 — https://www.rfc-editor.org/rfc/rfc7845 ✅
- **Latest Version**: RFC 6716 (September 2012), updated by RFC 8251

## Technical Summary

Opus is a versatile, royalty-free, open audio codec designed for interactive speech and music. It combines two layers: SILK (linear prediction, optimized for speech) and CELT (MDCT, optimized for music), with a hybrid mode that uses both simultaneously. It scales from 6 kbit/s narrowband speech to 510 kbit/s fullband stereo music, with algorithmic delays from 5 ms to 65.2 ms.

## Key Technical Details

- **Bitstream format**: TOC byte + frames. Self-delimiting variant in Appendix B.
- **Sample rates**: 8, 12, 16, 24, 48 kHz (internal always 48 kHz)
- **Bandwidths**: Narrowband (4 kHz), Mediumband (6 kHz), Wideband (8 kHz), Super-wideband (12 kHz), Fullband (20 kHz)
- **Channels**: Mono or stereo (can switch dynamically)
- **Frame sizes**: 2.5, 5, 10, 20, 40, 60 ms (up to 120 ms packets)
- **Bitrate range**: 6 kbit/s to 510 kbit/s
- **Modes**: SILK-only (speech), CELT-only (music/low latency), Hybrid (SWB/FB speech)
- **Typical use**: VoIP, WebRTC, streaming audio, Discord, WhatsApp, YouTube

## WhyTho Relevance

- **Priority**: Critical — primary audio encode target, royalty-free
- **Encode**: Working (vendored opus-rs). Target: in-house pure-Rust encoder.
- **Decode**: Planned (in-house)
- **Notes**: The vendored opus-rs is a pure-Rust Opus implementation. Quality and feature completeness need evaluation. Long-term: replace with in-house encoder optimized for WhyTho's pipeline.

## Reference Implementations

- **libopus**: Reference implementation — https://opus-codec.org/
- **opus-rs**: Pure-Rust implementation — https://github.com/restsend/opus-rs

## Related Vendored Crates

- `opus-rs` (whytho/vendored/opus-rs): Pure-Rust Opus encoder/decoder. Encode API: `OpusEncoder::new(rate, channels, application)`, `encode(input, frame_size, output)`.
