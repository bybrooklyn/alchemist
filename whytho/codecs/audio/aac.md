# AAC (Advanced Audio Coding)

## Specification

- **Standard**: ISO/IEC 14496-3 (MPEG-4 Part 3 — Audio)
- **Organization**: ISO/IEC JTC 1/SC 29/WG 11
- **Spec URL**: https://www.iso.org/standard/43345.html
- **PDF**: Paid (ISO). Technical content freely available via reference implementations.
- **Latest Version**: ISO/IEC 14496-3:2019

## Technical Summary

AAC is the successor to MP3 and the standard audio codec for MPEG-4. It uses MDCT with window switching, temporal noise shaping (TNS), intensity stereo, mid-side stereo, and Huffman coding. AAC-LC (Low Complexity) is the most common profile. HE-AAC v1 (with SBR) and HE-AAC v2 (with SBR+PS) extend it for low bitrates.

## Key Technical Details

- **Bitstream format**: Raw AAC frames with ADTS (Audio Data Transport Stream) or LATM/LOAS headers
- **Sample rates**: 8, 11.025, 12, 16, 22.05, 24, 32, 44.1, 48, 64, 88.2, 96 kHz
- **Channels**: Up to 48 channels (mono, stereo, 5.1, 7.1, etc.)
- **Profiles**: AAC-LC, HE-AAC v1 (SBR), HE-AAC v2 (SBR+PS), AAC-LD (Low Delay), AAC-ELD (Enhanced Low Delay)
- **Bitrate range**: 8 kbit/s to 529 kbit/s (stereo)
- **Typical use**: Apple ecosystem, YouTube, broadcast (DVB, ATSC), streaming

## WhyTho Relevance

- **Priority**: High — most common audio codec in existing media libraries
- **Decode**: Planned (in-house pure-Rust)
- **Encode**: Planned (in-house pure-Rust)
- **Notes**: AAC decode is essential for reading existing media. AAC-LC is the primary target. HE-AAC v1/v2 support for low-bitrate content.

## Reference Implementations

- **fdk-aac**: Fraunhofer FDK AAC library — https://github.com/mstorsjo/fdk-aac
- **FAAD2**: Open-source decoder — https://github.com/knik0/faad2
- **qaac**: Apple AAC encoder wrapper — https://github.com/nu774/qaac
- **libfdk-aac**: Part of FFmpeg's non-free build

## Related Vendored Crates

- None yet. Pure-Rust AAC decoder is a planned in-house implementation.
