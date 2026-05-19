---
title: Media Analyzer
description: How Alchemist extracts media facts with FFprobe before planning a job.
keywords:
  - media analyzer
  - ffprobe
  - analyzer labels
  - media metadata
  - library intelligence
---

The media analyzer is the first decision-making stage after a file is discovered.
It runs FFprobe once, normalizes the raw stream metadata, and returns a typed
analysis object for the planner and future intelligence features.

The analyzer does **not** decide whether to transcode. It records facts. The
planner remains responsible for skip, remux, and transcode decisions.

## What the analyzer extracts

For the selected video stream and container, Alchemist records:

- codec, container, resolution, frame rate, duration, and file size
- video bitrate and container bitrate when FFprobe reports them
- bit depth and pixel-format warnings
- HDR/color metadata such as transfer, primaries, color space, and range
- audio stream metadata, channel counts, default/forced flags, and heavy/lossless audio facts
- subtitle stream metadata, including image-based and styled subtitle facts
- cheap structure metadata such as interlacing flags and Dolby Vision side data
- source chapter count for non-fatal preservation checks after finalization

This is intentionally limited to metadata already available from the normal
probe path. Alchemist does not run expensive sampled probes such as cropdetect,
VMAF pre-flight, OCR, decode spot checks, or grain/complexity analysis during
this pass.

## Analyzer report

Each `MediaAnalysis` includes an `analysis_report` alongside the existing
metadata, warnings, and confidence fields. The report has two parts:

```text
analysis_report:
  labels:  factual classifications
  metrics: measured optional values
```

### Labels

Labels are factual and deterministic. They are not product-policy conclusions.
Examples include:

| Label | Meaning |
|-------|---------|
| `high_bpp_density` | The measured normalized video bits-per-pixel is high. |
| `low_bpp_density` | The measured normalized video bits-per-pixel is low. |
| `remux_like_density` | Container bitrate is very close to video bitrate, suggesting low mux overhead. |
| `heavy_audio` | At least one audio stream uses a codec Alchemist treats as heavy/lossless for optimization evidence. |
| `lossless_audio` | At least one audio stream is a lossless codec such as TrueHD, FLAC, ALAC, or PCM. |
| `image_subtitle` | At least one subtitle stream is image-based, such as PGS/DVD subtitles. |
| `styled_subtitle` | At least one subtitle stream uses styled text, such as ASS/SSA. |
| `hdr_metadata` | HDR metadata is present. |
| `bt2020_without_transfer` | BT.2020 primaries are present but transfer metadata is missing. |
| `dolby_vision_metadata` | Dolby Vision side-data metadata is present. |
| `interlaced_metadata` | FFprobe reports an interlaced field order. |
| `variable_frame_rate_hint` | Average-rate and frame-count-derived FPS disagree enough to suggest VFR. |

Warnings such as `missing_video_bitrate`, `missing_duration`, and
`unrecognized_pixel_format` are mirrored as labels so future UI surfaces can
show the same evidence without reverse-engineering warning enums.

### Metrics

Metrics are optional because FFprobe does not report every field for every file.
Current metrics include:

- `raw_bpp` and `normalized_bpp`
- `estimated_container_bitrate_bps`
- aggregate `audio_bitrate_share`
- video/audio/subtitle stream counts
- image/text subtitle counts
- HDR and BT.2020 booleans
- FPS values derived from average rate and frame count

If video bitrate is missing, Alchemist may still estimate container bitrate from
file size and duration, but BPP density labels remain absent because they require
a measured video bitrate.

## Cache behavior

Analyzer output is cached in the media probe cache keyed by path, mtime, size,
file identity when available, and FFprobe/cache schema version. The analyzer
report bumps the probe cache schema marker so older cached probe payloads are
refreshed before these new facts are used.

Legacy cached `MediaAnalysis` JSON without `analysis_report` still decodes
safely; the report defaults to an empty label set and empty metrics.

## Relationship to the planner

The planner still uses the stable `MediaMetadata` fields and existing
confidence/warning behavior. Analyzer labels and metrics are stored for later
Library Intelligence and explanation work, but this pass does not change job
queueing, skip/remux/transcode decisions, output promotion, or replacement
policy.

See [Planner](/planner) and [Skip Decisions](/skip-decisions) for the current
policy layer.
