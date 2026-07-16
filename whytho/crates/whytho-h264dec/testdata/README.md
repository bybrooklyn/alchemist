# Test Data

## single_frame.h264

A minimal H.264 Annex B bitstream containing a single 16x16 pixel red frame.

Properties:
- Profile: Constrained Baseline (profile_idc=66)
- Level: 1.0
- Pixel format: YUV 4:2:0, 8-bit
- Entropy coding: CAVLC (no CABAC)
- 1 macroblock (I16x16, DC prediction)
- No B-frames, 1 reference frame

NAL unit structure (each prefixed with `00 00 00 01` start code):
1. **SPS** (NAL type 7) - Sequence Parameter Set
2. **PPS** (NAL type 8) - Picture Parameter Set
3. **SEI** (NAL type 6) - x264 encoder metadata
4. **IDR slice** (NAL type 5) - The single I-frame

Generated with:
```
ffmpeg -f lavfi -i "color=c=red:s=16x16:d=0.04:r=25" -vframes 1 \
  -c:v libx264 -profile:v baseline -level 1 \
  -pix_fmt yuv420p -x264-params "keyint=1:bframes=0:ref=1:no-cabac=1" \
  -f h264 single_frame.h264
```

## single_frame.yuv

Raw YUV420p decode of `single_frame.h264`. 384 bytes total for a 16x16 frame:

| Plane | Size | Offset | Value | Description |
|-------|------|--------|-------|-------------|
| Y     | 16x16 = 256 bytes | 0x000 | 0x51 (81)  | Luma |
| U (Cb)| 8x8 = 64 bytes    | 0x100 | 0x5A (90)  | Chroma blue |
| V (Cr)| 8x8 = 64 bytes    | 0x140 | 0xF0 (240) | Chroma red |

These values correspond to the color red in BT.601 YCbCr space.

Generated with:
```
ffmpeg -i single_frame.h264 -pix_fmt yuv420p -f rawvideo single_frame.yuv
```
