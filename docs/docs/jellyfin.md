---
title: Alchemist for Jellyfin — Pre-Transcode Your Library
description: Use Alchemist to pre-transcode a Jellyfin media library to AV1, HEVC, or H.264. Reduces on-the-fly transcoding load, keeps originals safe, and runs entirely self-hosted.
keywords:
  - jellyfin transcoding
  - jellyfin transcoding automation
  - pre transcode jellyfin
  - jellyfin av1
  - jellyfin hevc
---

Alchemist is a self-hosted transcoding automation tool. It
doesn't replace Jellyfin — it prepares files **before** they
hit Jellyfin, so the Jellyfin server spends less time
transcoding on the fly.

## Why pre-transcode for Jellyfin

Jellyfin transcodes live whenever a client requests a codec,
container, or bitrate it can't direct-play. That's expensive
on the server and often noticeable on the client. Two things
make pre-transcoding worthwhile:

- **Fewer live transcodes.** If the whole library already
  sits in a codec your clients direct-play, Jellyfin's own
  transcoder rarely runs.
- **Predictable storage.** Re-encoding an older H.264 library
  to HEVC or AV1 can recover substantial space — how much
  depends on source bitrate and target quality settings.

Alchemist automates the decision: per file, it checks whether
transcoding would actually save meaningful space at the
quality target you set, and skips files that are already
efficiently compressed. Every skip records a plain-English
reason.

## What Alchemist does for a Jellyfin library

- Points at the same directories Jellyfin scans.
- Uses your GPU (NVENC, Intel Quick Sync, VAAPI, AMD AMF, or
  Apple VideoToolbox) when one is available — CPU fallback
  otherwise. See [Hardware Acceleration](/hardware).
- Writes output alongside the source by default (with a
  configurable suffix), or mirrors into a separate output
  root. Originals are **never** overwritten unless you
  explicitly enable `delete_source`. See
  [Configuration Reference](/configuration-reference).
- Optionally scores output with VMAF and reverts the encode
  if quality falls below your threshold. See
  [Quality settings](/configuration-reference#quality).
- Runs inside your off-peak schedule windows if you configure
  them. See [Scheduling](/scheduling).

## Setting it up with Jellyfin

1. Install Alchemist — [Docker](/docker) is the usual path
   for a Jellyfin-style homelab.
2. In the setup wizard, add the same library roots you've
   given Jellyfin. Use the container-side paths when running
   in Docker. See [First Run](/first-run).
3. Pick a target codec. AV1 gives the biggest savings on
   modern clients but requires AV1 decode support on the
   device; HEVC is the safer default if you have older
   clients in the mix. See [Codecs](/codecs).
4. Choose a hardware vendor if you want to pin one, or leave
   on auto-detect. See
   [Hardware Acceleration](/hardware).
5. Start the engine and watch the queue.

Jellyfin picks up the new files automatically on its next
scan. By default, Alchemist does not move or rename your
originals — Jellyfin keeps seeing both until you decide what
to do with the sources.

## Codec notes for Jellyfin clients

Decoding support varies by client, OS, and browser. A short
rule of thumb:

- **H.264**: universal. Safe fallback if compatibility
  matters more than space.
- **HEVC (H.265)**: wide support on TVs and mobile; browser
  support is mixed and depends on OS and hardware.
- **AV1**: best compression, supported by recent Chromium
  browsers, newer Apple devices, and many 2023+ TVs. Older
  clients will force Jellyfin to transcode back.

Check the Jellyfin docs for the latest compatibility matrix
before committing a whole library to a codec.

## FAQ

**Does Alchemist replace Jellyfin's built-in transcoder?**
No. Jellyfin still handles live playback. Alchemist's job
is to reduce how often that live transcoder has to run by
preparing files in a codec your clients can direct-play.

**Does Alchemist need access to Jellyfin's API?**
No. Alchemist operates on the filesystem — point it at the
same directories Jellyfin scans. Jellyfin picks up changes
on its own library scan.

**Will Alchemist break Jellyfin's existing media?**
Not by default. Originals are kept until you explicitly
enable `delete_source`. If you enable VMAF gating, encodes
below your quality threshold are rolled back instead of
promoted. See
[Configuration Reference](/configuration-reference).

**What happens to metadata, subtitles, and chapter markers
during transcoding?**
They are preserved on the transcoded output by default.
Alchemist re-encodes the video stream and passes other
streams through unless your [stream rules](/stream-rules)
say otherwise.

**Is there a way to try it without touching my library?**
Yes. Run `alchemist plan /path/to/library` — it scans and
analyses every file and reports the per-file decision
without enqueueing jobs. See [Installation](/installation).

**Should I target AV1 for Jellyfin?**
Only if your clients direct-play AV1. Otherwise Jellyfin
live-transcodes AV1 back to a compatible codec on the
server, which defeats the point of pre-transcoding. See
[AV1](/av1) and [Codecs](/codecs).

## See also

- [Hardware Acceleration](/hardware) — set up NVENC, Quick
  Sync, VAAPI, AMF, or VideoToolbox.
- [Codecs](/codecs) — target codec tradeoffs.
- [AV1](/av1) — deeper dive into AV1 hardware / software
  paths and when AV1 is the right target.
- [Profiles](/profiles) — different settings per library
  (e.g. Movies vs TV).
- [Skip Decisions](/skip-decisions) — why Alchemist skips
  files that are already efficient.
- [Jellyfin direct-play failing](/troubleshooting/jellyfin-direct-play-failing) —
  why Jellyfin still transcodes after Alchemist processed
  the file.
- [Alchemist vs Tdarr](/alternatives/tdarr) · [Alchemist vs
  FileFlows](/alternatives/fileflows).
