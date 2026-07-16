# AV2 (AOMedia Video 2)

## Specification

- **Standard**: AV2 Bitstream & Decoding Process Specification
- **Organization**: Alliance for Open Media (AOMedia)
- **Spec URL**: https://aomediacodec.github.io/av2-spec/
- **PDF**: https://aomediacodec.github.io/av2-spec/v1.0.0/20260528_38f28e7_AV2_Spec_v1.0.0.pdf ✅ (freely available)
- **Additional Tables**: https://aomediacodec.github.io/av2-spec/v1.0.0/attachments/all_tables.h
- **Syntax Browser**: https://aomediacodec.github.io/av2-spec/v1.0.0/syntax_browser.html
- **Reference Software (AVM)**: https://github.com/AOMediaCodec/avm/tree/v1.0.0
- **Latest Version**: v1.0.0 (28 May 2026)

## Technical Summary

AV2 is the next-generation royalty-free video codec from AOMedia, successor to AV1. It provides superior compression efficiency for streaming, broadcasting, real-time conferencing, AR/VR, split-screen delivery, and screen content. Building on AV1's foundation, AV2 adds enhanced prediction tools, improved entropy coding, better loop filtering, and new coding tools for emerging use cases.

## Key Technical Details

- **Bitstream format**: OBU (Open Bitstream Unit) — evolution of AV1's OBU framing
- **Profiles**: Not yet public in v1.0.0 summary
- **Key improvements over AV1**: Enhanced AR/VR support, split-screen delivery, improved screen content handling, wider quality range
- **Reference software**: AVM (AOMedia Video Model)

## WhyTho Relevance

- **Priority**: Critical — flagship in-house codec, royalty-free, next-generation
- **Encode**: In-house (rav2e) — being merged into whytho-codecs. Currently pre-alpha (128×128 stills).
- **Decode**: Planned (in-house)
- **Notes**: rav2e is WhyTho's own AV2 encoder. Currently at pre-alpha stage — encodes single 128×128 8-bit 4:2:0 still-picture keyframes as minimal .obu streams, validated by AVM reference decoder.

## Reference Implementations

- **AVM**: AOMedia reference encoder/decoder — https://github.com/AOMediaCodec/avm
- **rav2e**: WhyTho's pure-Rust AV2 encoder (now lives in whytho workspace)

## In-House Implementation (rav2e)

| Crate | Purpose |
|---|---|
| `whytho-tables` | Generated const tables from AV2 spec attachments |
| `whytho-entropy` | AVM-compatible range entropy coding, bit writing, CDF updates, OBU framing |
| `whytho-dsp` | Scalar DSP baselines + runtime-dispatched arch kernels (ARM/x86 ASM) |
| `whytho-codec-av2` | Constrained encoder pipeline and public Encoder API, plus core enums and shared types (block/transform sizes, modes, partitions, frame buffers) in its `common` module — formerly the separate `whytho-codec-av2-common` crate |

**License**: Apache-2.0 + AOM Patent License 1.0 (see PATENTS)

**Not yet implemented**: Non-zero residual coding, real rate-distortion search, arbitrary geometry, partition trees, inter prediction, loop filters, rate control, multiple frames/tiles, 10-bit output.
