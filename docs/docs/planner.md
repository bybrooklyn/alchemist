---
title: Planner
description: The planner evaluates every file deterministically — BPP thresholds, resolution and confidence multipliers, codec and target multipliers — and produces a transcode, remux, or skip decision.
keywords:
  - transcoding planner
  - bpp threshold
  - skip decision logic
  - remux vs transcode
---

The planner runs once per job during the analysis phase and produces one of three decisions:

- **Transcode** — re-encode the video stream.
- **Remux** — copy streams into a different container (lossless, fast).
- **Skip** — mark the file as not worth processing.

Decisions are deterministic and based solely on file metadata and settings.

---

## Decision flow

Each condition is evaluated in order. The first match wins.

```
1. already_target_codec          → Skip (or Remux if container mismatch)
2. no_available_encoders         → Skip
3. preferred_codec_unavailable   → Skip (if fallback disabled)
4. no_suitable_encoder           → Skip (no encoder selected)
5. incomplete_metadata           → Skip (missing resolution)
6. bpp_below_threshold           → Skip (already efficient)
7. below_min_file_size           → Skip (too small)
8. h264 source                   → Transcode (priority path)
9. everything else               → Transcode (transcode_recommended)
```

---

## Skip conditions

### already_target_codec

The video stream is already in the target codec at the required bit depth.

- **AV1 / HEVC target:** skip if codec matches AND bit depth is 10-bit.
- **H.264 target:** skip if codec is h264 AND bit depth is 8-bit or lower.

If the codec matches but the container does not (e.g. AV1 in an MP4, target MKV), the decision is **Remux** instead.

```
skip if: codec == target AND bit_depth == required_depth
remux if: above AND container != target_container
```

---

### bpp_below_threshold

**Bits-per-pixel** measures how efficiently a file is already compressed relative to its resolution and frame rate.

#### Formula

```
raw_bpp = video_bitrate_bps / (width × height × fps)
normalized_bpp = raw_bpp × resolution_multiplier
effective_threshold = min_bpp_threshold × confidence_multiplier × codec_multiplier × target_multiplier

skip if: normalized_bpp < effective_threshold
```

#### Resolution multipliers

| Resolution | Multiplier | Reason |
|------------|-----------|--------|
| ≥ 3840px wide (4K) | 0.60× | 4K compression is naturally denser |
| ≥ 1920px wide (1080p) | 0.80× | HD has moderate density premium |
| < 1920px (SD) | 1.00× | No adjustment |

#### Confidence multipliers

Applied to the threshold when Alchemist is uncertain about bitrate accuracy:

| Confidence | Multiplier | When |
|-----------|-----------|------|
| High | 1.00× | Video bitrate directly reported by FFprobe |
| Medium | 0.70× | Bitrate estimated from container/file size |
| Low | 0.50× | Bitrate estimated with low reliability |

Lower confidence → lower threshold → harder to skip → safer.

#### Codec multipliers

| Source codec | Multiplier | Reason |
|-------------|-----------|--------|
| h264 (AVC) | 0.60× | H.264 needs more bits to match HEVC/AV1 quality |

#### Target multipliers

| Target codec | Multiplier | Reason |
|-------------|-----------|--------|
| AV1 | 0.70× | AV1 is more efficient; skip more aggressively |
| HEVC/H.264 | 1.00× | No adjustment |

#### Worked example

Settings: `min_bpp_threshold = 0.10`, target AV1, source HEVC 10-bit 4K.

```
raw_bpp = 15_000_000 / (3840 × 2160 × 24) = 0.0756
normalized_bpp = 0.0756 × 0.60 = 0.0454          (4K multiplier)

threshold = 0.10 × 1.00 × 1.00 × 0.70 = 0.070   (AV1 multiplier, HEVC source)

0.0454 < 0.070 → SKIP (bpp_below_threshold)
```

---

### below_min_file_size

Files smaller than `min_file_size_mb` (default: 50 MB) are skipped. Small files have minimal savings potential relative to overhead.

**Adjust:** Settings → Transcoding → Minimum file size.

---

### incomplete_metadata

FFprobe could not determine resolution (width or height is zero). Without resolution, BPP cannot be computed and no valid decision can be made.

**Diagnose:** run Library Doctor on the file.

---

### no_available_encoders

No encoder is available for the target codec at all. Either:
- CPU encoding is disabled (`allow_cpu_encoding = false`)
- Hardware detection failed and CPU fallback is off

**Fix:** Settings → Hardware → Enable CPU fallback.

---

### preferred_codec_unavailable_fallback_disabled

The requested codec encoder is not available, and `allow_fallback = false` prevents using any substitute.

**Fix:** Enable CPU fallback in Settings → Hardware, or check GPU detection.

---

## Transcode paths

### transcode_h264_source

H.264 files are unconditionally transcoded (if not skipped by BPP or size filters above). H.264 is the largest space-saving opportunity in most libraries.

### transcode_recommended

Everything else that passes the skip filters. Alchemist transcodes it because it is a plausible candidate based on the current codec and measured efficiency.

---

## Remux path

### already_target_codec_wrong_container

The video is already in the correct codec but wrapped in the wrong container (e.g. AV1 in `.mp4`, target is `.mkv`). Alchemist remuxes using stream copy — fast and lossless.

---

## Tuning

| Setting | Effect |
|---------|--------|
| `min_bpp_threshold` | Higher = skip more files. Default: 0.10. |
| `min_file_size_mb` | Higher = skip more small files. Default: 50. |
| `size_reduction_threshold` | Minimum predicted savings. Default: 30%. |
| `allow_fallback` | Allow CPU encoding when hardware is unavailable. |
| `allow_cpu_encoding` | Allow CPU to encode (not just fall back). |
