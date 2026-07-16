# IVF (Indeo Video Format)

## Specification

- **Standard**: IVF Container Format
- **Organization**: Informal (originally Intel Indeo, adopted by VP8/VP9/AV1)
- **Spec URL**: https://wiki.multimedia.cx/index.php/IVF
- **Spec HTML**: https://wiki.multimedia.cx/index.php/IVF ✅ (freely available — wiki)
- **Latest Version**: N/A (informal specification)

## Technical Summary

IVF is an extremely simple container format originally designed for Intel's Indeo video codec. It was later adopted by the VP8, VP9, and AV1 codec projects as a minimal raw-stream container. IVF has a 32-byte file header followed by length-prefixed frames. No audio support, no metadata, no seeking information.

## Key Technical Details

- **Binary format**: 32-byte header + frames (12-byte frame header + data)
- **File extensions**: .ivf
- **Video codecs**: VP8, VP9, AV1 (historically: Indeo 3/4/5)
- **Audio**: Not supported
- **Metadata**: Not supported
- **Header structure**: `DKIF` magic, version (0), header length (32), codec FourCC, width, height, timebase numerator, timebase denominator, number of frames, unused
- **Frame structure**: 4-byte frame size (LE) + 8-byte timestamp (LE) + frame data
- **Typical use**: Raw video stream storage, codec testing, intermediate files

## WhyTho Relevance

- **Priority**: Low — minimal container, useful for testing
- **Read**: Planned (trivial to implement)
- **Write**: Planned (trivial to implement)
- **Notes**: IVF is the simplest container to implement. Useful for raw AV1/VP9 stream output during codec development and testing. The rav1e encoder already outputs IVF optionally.

## Reference Implementations

- **libvpx**: VP8/VP9 includes IVF reading/writing
- **rav1e**: AV1 encoder includes IVF output support
- **dav1d**: AV1 decoder includes IVF reading

## Related Vendored Crates

- `rav1e` (whytho/vendored/rav1e): Includes IVF output as optional feature (`ivf` crate in rav1e workspace).
