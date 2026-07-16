# H.264 / AVC (Advanced Video Coding)

## Specification

- **Standard**: ITU-T H.264 / ISO/IEC 14496-10 (MPEG-4 Part 10)
- **Organization**: ITU-T / ISO/IEC JTC 1/SC 29/WG 11
- **Spec URL**: https://www.itu.int/rec/T-REC-H.264
- **PDF**: Paid (ITU). Draft freely available at https://www.itu.int/wftp3/av-arch/jvt-site/
- **Latest Version**: H.264 (V14) — 08/2024

## Technical Summary

H.264/AVC is the most widely deployed video codec in the world. It uses block-based motion-compensated prediction with integer DCT transforms, CABAC/CAVLC entropy coding, and in-loop deblocking filtering. It supports resolutions from QCIF to 8K and bitrates from kilobits to hundreds of megabits per second.

## Key Technical Details

- **Bitstream format**: NAL (Network Abstraction Layer) units. Two packetization modes: Annex B (start-code delimited) and AVCC (length-prefixed, used in MP4/MKV).
- **Max resolution**: 8192×8192 (Level 5.2+)
- **Profiles**: Baseline, Main, High, High 10, High 4:2:2, High 4:4:4 Predictive, Scalable Baseline, Scalable High, Multiview High, Stereo High
- **Levels**: 1.0 through 6.2
- **Color spaces**: YUV 4:2:0 (8-bit all profiles), YUV 4:2:2 (High 4:2:2+), YUV 4:4:4 (High 4:4:4+)
- **Entropy coding**: CAVLC (Baseline) or CABAC (Main+)
- **Typical use**: Streaming, broadcast, Blu-ray, video conferencing, surveillance

## WhyTho Relevance

- **Priority**: Critical — most common source format
- **Decode**: Working (vendored rust_h264). Target: in-house pure-Rust decoder.
- **Encode**: Planned (in-house). No working implementation yet.
- **Notes**: The vendored rust_h264 supports Baseline/Main/High profiles (8-bit 4:2:0 only). High 10, 4:2:2, and 4:4:4 are not yet supported.

## Reference Implementations

- **JM** (Joint Model): ITU-T reference software — https://iphome.hhi.de/suehring/
- **x264**: Open-source encoder — https://www.videolan.org/developers/x264.html
- **OpenH264**: Cisco's open-source encoder/decoder — https://github.com/cisco/openh264

## Related Vendored Crates

- `rust_h264` (whytho/vendored/rust_h264): Pure-Rust H.264 decoder. Supports Baseline/Main/High, 8-bit 4:2:0, CABAC/CAVLC, B-frames, multi-reference.
