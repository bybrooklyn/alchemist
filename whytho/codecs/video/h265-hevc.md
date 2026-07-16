# H.265 / HEVC (High Efficiency Video Coding)

## Specification

- **Standard**: ITU-T H.265 / ISO/IEC 23008-2 (MPEG-H Part 2)
- **Organization**: ITU-T / ISO/IEC JTC 1/SC 29/WG 11
- **Spec URL**: https://www.itu.int/rec/T-REC-H.265
- **PDF**: Paid (ITU). Draft freely available at https://www.itu.int/wftp3/av-arch/jvet-site/
- **Latest Version**: H.265 (V6) — 02/2024

## Technical Summary

HEVC is the successor to H.264, offering roughly 50% better compression at the same quality. It uses larger coding tree units (CTUs up to 64×64), more flexible partitioning (quad-tree + binary + ternary), 35 angular intra prediction modes, advanced motion vector prediction, sample adaptive offset (SAO) filtering, and improved entropy coding (CABAC with context modeling improvements).

## Key Technical Details

- **Bitstream format**: NAL units (similar structure to H.264 but different NAL unit types). HVCC configuration record in MP4/MKV.
- **Max resolution**: 8192×8192 (Level 6.0+)
- **Profiles**: Main, Main 10, Main Still Picture, Range Extensions (Main 12, Main 4:2:2, Main 4:4:4, etc.)
- **Levels**: 1.0 through 6.2
- **Color spaces**: YUV 4:2:0 (Main/Main 10), YUV 4:2:2, YUV 4:4:4 (Range Extensions)
- **Bit depth**: 8-bit (Main), 10-bit (Main 10), 12-bit (Range Extensions)
- **CTU size**: 64×64, 32×32, or 16×16
- **Typical use**: 4K streaming, UHD Blu-ray, broadcast, OTT services

## WhyTho Relevance

- **Priority**: High — increasingly common, especially for 4K content
- **Decode**: Planned (in-house pure-Rust)
- **Encode**: Planned (in-house pure-Rust)
- **Notes**: Patent-encumbered. WhyTho commits to pure-Rust implementation despite patent landscape. Decoder is the first target.

## Reference Implementations

- **HM** (HEVC Test Model): Reference software — https://hevc.hhi.fraunhofer.de/
- **x265**: Open-source encoder — https://www.x265.org/
- **Kvazaar**: Open-source encoder — https://github.com/ultravideo/kvazaar
- **libde265**: Open-source decoder — https://github.com/nicabueschow/libde265

## Related Vendored Crates

- None yet. Pure-Rust HEVC decoder/encoder is a planned in-house implementation.
