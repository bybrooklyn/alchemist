# MP4 / ISOBMFF (ISO Base Media File Format)

## Specification

- **Standard**: ISO/IEC 14496-12 (MPEG-4 Part 12 — ISO Base Media File Format)
- **Organization**: ISO/IEC JTC 1/SC 29/WG 11
- **Spec URL**: https://www.iso.org/standard/83458.html
- **PDF**: Paid (ISO). Draft versions available at https://mpeg.chiariglione.org/
- **Latest Version**: ISO/IEC 14496-12:2022 (8th edition)

## Technical Summary

ISOBMFF is the base container format for MP4, MOV, M4A, M4V, 3GP, and many other media file types. It uses a box/atom structure where each element is a length-prefixed box containing data or other boxes. It supports interleaved audio/video, streaming (fragmented MP4), metadata, chapters, subtitles, and DRM.

## Key Technical Details

- **Binary format**: Box (atom) structure — 4-byte size + 4-byte type + payload. Extended size for large boxes.
- **File extensions**: .mp4, .m4v, .m4a, .mov, .3gp, .f4v, .ismv
- **Video codecs**: H.264 (AVC), H.265 (HEVC), AV1, VP9, MPEG-2, MPEG-4 Part 2
- **Audio codecs**: AAC, MP3, Opus, FLAC, ALAC, AC-3, E-AC-3
- **Subtitles**: WebVTT, TTMP, 3GPP Timed Text
- **Features**: Fragmented MP4 (fMP4) for streaming, edit lists, sample tables, DRM (Widevine, FairPlay, PlayReady)
- **Typical use**: Apple ecosystem, YouTube, streaming (HLS/DASH), Blu-ray, digital distribution

## WhyTho Relevance

- **Priority**: High — second most common container after MKV
- **Read**: Planned
- **Write**: Planned
- **Notes**: MP4 read support is essential for existing media libraries. Fragmented MP4 (fMP4) support needed for streaming workflows. The box structure is well-documented and relatively straightforward to parse.

## Reference Implementations

- **mp4parse** (Mozilla): Pure-Rust MP4 parser — https://github.com/mozilla/mp4parse-rust
- **GPAC**: Full MP4 tooling — https://gpac.io/
- **L-SMASH**: MP4/MOV library — https://github.com/nicabueschow/L-SMASH

## Related Vendored Crates

- None yet. MP4 read/write is a planned in-house implementation.
