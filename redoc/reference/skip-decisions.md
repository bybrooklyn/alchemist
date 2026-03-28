# Skip Decisions

Why Alchemist skipped a file and what each skip reason means.

When Alchemist skips a file it always records a machine-readable
reason string. In the web interface this appears as a
plain-English explanation in the job detail panel. This page
documents every possible skip reason and what to do about it.

## Skip reasons

### `already_target_codec`

The file is already encoded in the target codec (e.g. AV1)
and already in 10-bit color depth. Re-encoding it would not
save meaningful space and could reduce quality.

**What to do:** Nothing. This is the correct outcome.

### `already_target_codec_wrong_container`

The file is already in the target codec but is wrapped in an
MP4/M4V container and you've configured MKV as the output
extension. Alchemist will **remux** it (fast, lossless
container conversion) rather than skip it.

**What to do:** Nothing - Alchemist handles this automatically.

### `bpp_below_threshold`

Bits-per-pixel is below the configured minimum threshold.
The file is already efficiently compressed; transcoding it
would consume significant CPU/GPU time for minimal savings
and could introduce quality loss.

Technical detail: `bpp={value},threshold={value}` in the
raw reason string. The threshold is resolution-adjusted
(4K content has a lower effective threshold) and
confidence-adjusted based on how reliable the bitrate
measurement is.

**What to do:** If you believe the file should be transcoded,
lower `min_bpp_threshold` in Settings -> Transcoding. The
default is 0.10.

### `below_min_file_size`

The file is smaller than `min_file_size_mb`. Transcoding
very small files is usually not worth the time.

**What to do:** Lower `min_file_size_mb` in Settings ->
Transcoding if you want small files processed. Default is 50 MB.

### `size_reduction_insufficient`

The predicted output size is not meaningfully smaller than
the input. Alchemist estimated the output would be less than
`size_reduction_threshold` smaller.

**What to do:** Lower `size_reduction_threshold` in Settings
-> Transcoding if you want more aggressive transcoding.
Default is 0.30 (30% reduction required).

### `no_suitable_encoder`

No encoder is available for the target codec on this machine.
This usually means hardware detection failed and CPU fallback
is disabled.

**What to do:** Check Settings -> Hardware. Enable CPU fallback,
or fix hardware detection. See the Hardware guides for your
GPU vendor.

### `incomplete_metadata`

FFprobe could not determine resolution, duration, or bitrate
for this file. Without reliable metadata Alchemist cannot
make a valid transcoding decision.

**What to do:** Check if the file is corrupt using Library
Doctor. Try playing it in a media player to confirm it works.

## Why so many skips is a good sign

A high skip rate means Alchemist is protecting you from
pointless re-encodes. Files get skipped when they are already
efficiently compressed - the work has already been done,
either by whoever encoded them originally or by a previous
Alchemist run.

A library where 80% of files are skipped and 20% are transcoded
is normal for a library that already has a mix of well-encoded
and poorly-encoded content.
