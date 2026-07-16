# AV1 (AOMedia Video 1)

## Specification

- **Standard**: AV1 Bitstream & Decoding Process Specification
- **Organization**: Alliance for Open Media (AOMedia)
- **Spec URL**: https://aomediacodec.github.io/av1-spec/
- **PDF**: https://aomediacodec.github.io/av1-spec/av1-spec.pdf ✅ (freely available)
- **Latest Version**: Version 1.0.0 with Errata 1

## Technical Summary

AV1 is a royalty-free, open video codec developed by AOMedia (Google, Mozilla, Cisco, Microsoft, Netflix, Amazon, Intel, etc.). It builds on VP9 with significant improvements: larger superblocks (128×128), more intra prediction modes (56+), compound inter prediction, warped motion, filter-intra, palette mode, recursive partitioning, CDEF filtering, loop restoration, and film grain synthesis. It achieves roughly 30-50% better compression than H.264 and 20-30% better than HEVC.

## Key Technical Details

- **Bitstream format**: OBU (Open Bitstream Unit) framing. Low-overhead byte stream (Annex B equivalent) and OBU sequence format. AV1CodecConfigurationRecord in MP4/MKV.
- **Max resolution**: 65536×65536 (theoretical), practical up to 8K
- **Profiles**: Main (8-bit 4:2:0), High (8-bit 4:4:4), Professional (10/12-bit 4:2:2/4:4:4)
- **Levels**: 2.0 through 7.0
- **Color spaces**: YUV 4:2:0, 4:2:2, 4:4:4. BT.709, BT.2020, DCI-P3. HDR10, HLG, PQ.
- **Bit depth**: 8, 10, 12 bits
- **Entropy coding**: Multi-symbol arithmetic coder
- **Typical use**: YouTube, Netflix, streaming, WebRTC, AVIF images

## WhyTho Relevance

- **Priority**: Critical — primary encode target, royalty-free
- **Decode**: Planned (in-house)
- **Encode**: Working (vendored rav1e). Target: replace with in-house encoder.
- **Notes**: AV1 is the current primary encoding target for WhyTho. rav1e is the vendored encoder. Long-term goal is an in-house encoder built on WhyTho-specific optimizations.

## Reference Implementations

- **libaom** (AVM): AOMedia reference encoder/decoder — https://aomedia.googlesource.com/aom/
- **rav1e**: Pure-Rust encoder (Xiph.org) — https://github.com/xiph/rav1e
- **dav1d**: High-performance decoder (VideoLAN) — https://code.videolan.org/videolan/dav1d
- **SVT-AV1**: Scalable encoder (Intel/Netflix) — https://gitlab.com/AOMediaCodec/SVT-AV1

## Related Vendored Crates

- `rav1e` (whytho/vendored/rav1e): Pure-Rust AV1 encoder. Production-quality, used by Firefox.
