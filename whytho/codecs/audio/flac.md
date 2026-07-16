# FLAC (Free Lossless Audio Codec)

## Specification

- **Standard**: FLAC Format Specification
- **Organization**: Xiph.Org Foundation / IETF
- **Spec URL**: https://xiph.org/flac/format.html
- **RFC**: https://www.rfc-editor.org/rfc/rfc9639 ✅ (freely available — IETF Proposed Standard, 2024)
- **Latest Version**: FLAC 1.4.x / RFC 9639 (2024)

## Technical Summary

FLAC is a lossless audio codec that typically compresses audio to 50-60% of its original size. It uses linear prediction (LPC) with residual coding via Rice/Golomb codes, with optional MDCT-based blocking. FLAC is widely supported, fast to decode, and has no patent issues.

## Key Technical Details

- **Bitstream format**: Frames with sync code, header, subframes (one per channel), and residual. Metadata blocks at stream start.
- **Sample rates**: 1 Hz to 65535 Hz (common: 44100, 48000, 96000, 192000)
- **Channels**: 1-8 (mono, stereo, 5.1, 7.1)
- **Bit depth**: 4, 8, 12, 16, 20, 24, 32 bits
- **Compression levels**: 0-8 (faster decode at lower levels, better compression at higher)
- **Typical use**: Music archival, CD rips, audiophile collections, Spotify source

## WhyTho Relevance

- **Priority**: High — dominant lossless audio format
- **Decode**: Planned (in-house pure-Rust)
- **Encode**: Planned (in-house pure-Rust)
- **Notes**: FLAC encode/decode is relatively straightforward compared to lossy codecs. Good candidate for early in-house implementation.

## Reference Implementations

- **flac**: Reference encoder/decoder — https://xiph.org/flac/
- **libFLAC**: Reference library — https://github.com/xiph/flac

## Related Vendored Crates

- None yet. Pure-Rust FLAC encoder/decoder is a planned in-house implementation.
