# Vorbis

## Specification

- **Standard**: Vorbis I Specification
- **Organization**: Xiph.Org Foundation
- **Spec URL**: https://xiph.org/vorbis/doc/
- **Spec HTML**: https://xiph.org/vorbis/doc/Vorbis_I_spec.html ✅ (freely available)
- **Latest Version**: Vorbis I (2004-02-03)

## Technical Summary

Vorbis is a free, open-source, lossy audio codec. It uses MDCT with vector quantization of audio spectra. Vorbis was designed as a free alternative to MP3 and AAC. It uses a two-phase approach: floor functions (spectral envelope) and residue functions (spectral detail). Vorbis is commonly encapsulated in Ogg containers.

## Key Technical Details

- **Bitstream format**: Packed audio packets with codebook-based VQ. Ogg encapsulation (RFC 7845).
- **Sample rates**: Typically 8000-96000 Hz
- **Channels**: 1-255
- **Bitrate range**: 16 kbit/s to 500 kbit/s
- **Typical use**: Spotify (legacy), Wikipedia audio, games, open-source projects

## WhyTho Relevance

- **Priority**: Medium — declining usage but still found in legacy collections
- **Decode**: Planned (in-house pure-Rust)
- **Encode**: Not planned for v1
- **Notes**: Vorbis decode is needed for legacy Ogg Vorbis files. Lower priority than Opus (its successor) or AAC/MP3.

## Reference Implementations

- **libvorbis**: Reference implementation — https://xiph.org/vorbis/
- **aoTuV**: Improved encoder — https://ao-yumi.github.io/aotuv_web/

## Related Vendored Crates

- None yet.
