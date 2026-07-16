# MP3 (MPEG-1 Audio Layer III)

## Specification

- **Standard**: ISO/IEC 11172-3 (MPEG-1 Part 3) / ISO/IEC 13818-3 (MPEG-2 Part 3)
- **Organization**: ISO/IEC JTC 1/SC 29/WG 11
- **Spec URL**: https://www.iso.org/standard/22412.html
- **PDF**: Paid (ISO). The format is extremely well-documented in public literature.
- **Latest Version**: ISO/IEC 13818-3:1998 (MPEG-2 Layer III)

## Technical Summary

MP3 is the most widely recognized audio codec in history. It uses MDCT with psychoacoustic masking, Huffman coding, and joint stereo. It was the first widely adopted perceptual audio codec and remains ubiquitous despite being superseded by AAC and Opus. Patents expired in 2017.

## Key Technical Details

- **Bitstream format**: Sync word (0xFFE0/0xFFF0), header (bitrate, sample rate, channel mode, etc.), side information, Huffman-coded frequency data. Optionally wrapped in ID3v1/v2 tags.
- **Sample rates**: 32, 44.1, 48 kHz (MPEG-1); 16, 22.05, 24 kHz (MPEG-2); 8, 11.025, 12 kHz (MPEG-2.5)
- **Channels**: Mono, Stereo, Joint Stereo, Dual Channel
- **Bitrate range**: 8-320 kbit/s (CBR/VBR)
- **Typical use**: Legacy music collections, podcasts, web audio, portable players

## WhyTho Relevance

- **Priority**: High — essential for reading existing media libraries
- **Decode**: Planned (in-house pure-Rust)
- **Encode**: Not planned (AAC/Opus preferred)
- **Notes**: MP3 decode is mandatory for any media server. Patents fully expired. Focus on decode only.

## Reference Implementations

- **lame**: Reference encoder — https://lame.sourceforge.io/
- **mpg123**: High-performance decoder — https://www.mpg123.de/
- **libmad**: Open-source decoder — https://www.underbit.com/products/mad/

## Related Vendored Crates

- None yet.
