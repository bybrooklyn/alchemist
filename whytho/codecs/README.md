# WhyTho Codec & Container References

Generated: 2026-06-22

This directory contains detailed specification references for every codec and container format WhyTho targets. Each entry includes canonical spec URLs, PDF links, technical summaries, and WhyTho implementation status.

## Video Codecs

| Codec | Entry | WhyTho Status |
|---|---|---|
| H.264/AVC | [video/h264-avc.md](video/h264-avc.md) | Decode: vendored (rust_h264), Encode: planned |
| H.265/HEVC | [video/h265-hevc.md](video/h265-hevc.md) | Decode: planned, Encode: planned |
| AV1 | [video/av1.md](video/av1.md) | Decode: planned, Encode: vendored (rav1e) |
| AV2 | [video/av2.md](video/av2.md) | Encode: merging (rav2e) |

## Audio Codecs

| Codec | Entry | WhyTho Status |
|---|---|---|
| Opus | [audio/opus.md](audio/opus.md) | Encode: vendored (opus-rs) |
| AAC | [audio/aac.md](audio/aac.md) | Decode: planned |
| FLAC | [audio/flac.md](audio/flac.md) | Encode/Decode: planned |
| Vorbis | [audio/vorbis.md](audio/vorbis.md) | Decode: planned |
| MP3 | [audio/mp3.md](audio/mp3.md) | Decode: planned |

## Containers

| Format | Entry | WhyTho Status |
|---|---|---|
| Matroska/MKV | [containers/matroska-mkv.md](containers/matroska-mkv.md) | Read: vendored (matroska-demuxer), Write: vendored (mkv-element) |
| WebM | [containers/webm.md](containers/webm.md) | Subset of Matroska |
| MP4/ISOBMFF | [containers/mp4-isobmff.md](containers/mp4-isobmff.md) | Planned |
| IVF | [containers/ivf.md](containers/ivf.md) | Planned |

## Strategy

See also: [../codecs.md](../codecs.md) for the master implementation strategy document.

**In-house goal**: All major codecs built in pure Rust. Vendored crates serve as working interim while in-house implementations mature.
