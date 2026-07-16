# WebM

## Specification

- **Standard**: WebM Container Format
- **Organization**: Google / WebM Project
- **Spec URL**: https://www.webmproject.org/docs/container/
- **Spec HTML**: https://www.webmproject.org/docs/container/ ✅ (freely available)
- **Latest Version**: Based on Matroska (EBML)

## Technical Summary

WebM is a subset of the Matroska container format, designed for use with VP8/VP9/AV1 video and Vorbis/Opus audio in web browsers. It uses the same EBML structure as Matroska but with a restricted set of features and codec IDs. WebM was designed for HTML5 video and is natively supported by all major browsers.

## Key Technical Details

- **Binary format**: EBML (same as Matroska)
- **File extension**: .webm
- **Video codecs**: VP8, VP9, AV1
- **Audio codecs**: Vorbis, Opus
- **Subtitles**: WebVTT (via Matroska subtitle tracks)
- **Features**: Chapters, cues (seeking), tags
- **Restrictions vs Matroska**: No attachment support (fonts), limited codec set, no segment linking
- **Typical use**: Web video, YouTube, HTML5 video elements

## WhyTho Relevance

- **Priority**: Medium — subset of Matroska, handled by the same code
- **Read**: Supported via matroska-demuxer (Matroska subset)
- **Write**: Supported via mkv-element (Matroska subset)
- **Notes**: WebM support comes for free with Matroska support. Just need to ensure WebM codec IDs are recognized.

## Reference Implementations

- **libwebm**: Google's WebM library — https://github.com/nicabueschow/libwebm
- **libvpx**: VP8/VP9 encoder/decoder — https://chromium.googlesource.com/webm/libvpx/

## Related Vendored Crates

- Same as Matroska (matroska-demuxer, mkv-element)
