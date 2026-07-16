# Matroska / MKV

## Specification

- **Standard**: Matroska Media Container Format
- **Organization**: IETF (standardized as RFCs)
- **Spec URL**: https://www.matroska.org/technical/elements.html
- **EBML RFC**: https://www.rfc-editor.org/rfc/rfc8794 ✅ (freely available)
- **Matroska RFC**: https://www.rfc-editor.org/rfc/rfc9559 ✅ (freely available — 2024)
- **Element Spec**: https://www.matroska.org/technical/elements.html
- **Latest Version**: RFC 9559 (2024)

## Technical Summary

Matroska is an open, free, extensible multimedia container format. It is based on EBML (Extensible Binary Meta Language), a binary XML-like format. Matroska supports virtually unlimited numbers of video, audio, subtitle, and metadata tracks, chapters, attachments, tags, and cueing. MKV is the file extension for Matroska video files.

## Key Technical Details

- **Binary format**: EBML (RFC 8794) — variable-length element IDs and sizes
- **File extensions**: .mkv (video), .mka (audio), .mks (subtitle), .webm (WebM)
- **Supported codecs**: Any (H.264, H.265, AV1, VP9, Opus, AAC, FLAC, SRT, ASS, etc.)
- **Features**: Chapters, attachments (fonts, cover art), tags, seeking cues, segment linking, virtual segments
- **Max file size**: Virtually unlimited (64-bit addressing)
- **Typical use**: Media libraries, Plex/Jellyfin, Blu-ray rips, anime, archival

## WhyTho Relevance

- **Priority**: Critical — primary container format
- **Read**: Working (vendored matroska-demuxer)
- **Write**: Working (vendored mkv-element + custom muxer)
- **Notes**: Matroska is WhyTho's primary output container. The muxer has basic functionality (video + audio tracks, clusters). Needs expansion for chapters, attachments, tags, and proper seeking cues.

## Reference Implementations

- **libmatroska**: Reference C++ library — https://github.com/nicabueschow/libmatroska
- **mkvtoolnix**: Full Matroska tooling — https://mkvtoolnix.download/
- **matroska-specification**: IETF spec XML — https://github.com/nicabueschow/matroska-specification

## Related Vendored Crates

- `matroska-demuxer` (whytho/vendored/matroska-demuxer): Matroska/WebM demuxer. API: `MatroskaFile::open(file)`, `mkv.next_frame(&mut frame)`.
- `mkv-element` (whytho/vendored/mkv-element): Matroska/WebM element reader and writer. Low-level EBML encoding/decoding primitives.
