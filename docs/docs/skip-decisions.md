---
title: Skip Decisions
description: Why Alchemist skipped a file and what each reason means.
---

Every skipped file now has a structured explanation object
as the primary source of truth. The legacy machine-readable
reason string is still retained for compatibility and
debugging during rollout.

Structured explanation fields:

- `category`
- `code`
- `summary`
- `detail`
- `operator_guidance`
- `measured`
- `legacy_reason`

## Skip reasons

### already_target_codec

The file is already in the target codec at 10-bit depth.
Re-encoding would not save meaningful space.

**Action:** None. Correct behavior.

### bpp_below_threshold

Bits-per-pixel is below the configured minimum. The file is
already efficiently compressed.

The threshold is resolution-adjusted (4K gets a lower
effective threshold) and confidence-adjusted based on
bitrate measurement reliability.

**Action:** Lower `min_bpp_threshold` in Settings →
Transcoding (default: 0.10).

### below_min_file_size

The file is smaller than `min_file_size_mb` (default: 50 MB).

**Action:** Lower `min_file_size_mb` if you want small files
processed.

### size_reduction_insufficient

The predicted output would not be meaningfully smaller
(below `size_reduction_threshold`, default: 30%).

**Action:** Lower `size_reduction_threshold` in Settings →
Transcoding.

### no_suitable_encoder

No encoder available for the target codec. Usually means
hardware detection failed with CPU fallback disabled.

**Action:** Check Settings → Hardware. Enable CPU fallback,
or fix hardware detection.

### incomplete_metadata

FFprobe could not determine resolution, duration, or
bitrate. Cannot make a valid transcoding decision.

**Action:** Check if the file is corrupt using Library Doctor.

## Why a high skip rate is fine

A high skip rate means files are already efficiently
compressed. An 80% skip rate on a mixed library is normal.
