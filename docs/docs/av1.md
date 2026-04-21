---
title: AV1 Transcoding — Hardware and Software Encoding in Alchemist
description: How Alchemist encodes AV1. Hardware paths (av1_nvenc, av1_qsv, av1_vaapi, av1_amf, av1_videotoolbox), CPU fallback (SVT-AV1, libaom), and when AV1 is the right target codec.
keywords:
  - av1 transcoding
  - av1 nvenc
  - av1 qsv
  - svt-av1
  - av1 hardware encoding
slug: /av1
---

AV1 is Alchemist's most aggressive target codec. It produces
the smallest files per unit of perceived quality, at the
cost of slower encoding and narrower client compatibility
than HEVC or H.264.

Alchemist treats AV1 as a first-class output: the
[planner](/planner) knows AV1 has a better compression
ceiling than HEVC (applies a 0.70× target multiplier when
deciding whether a file is "already efficient enough"), and
the encoder selection path prefers hardware AV1 when it's
available.

## Encoder paths

Alchemist will use the first available AV1 encoder that
passes its startup probe. In approximate preference order:

| Encoder | Hardware | Notes |
|---|---|---|
| `av1_nvenc` | NVIDIA RTX 30 / 40 series | Fast, GPU-accelerated |
| `av1_qsv` | Intel 12th gen+ | Fast, power-efficient |
| `av1_vaapi` | AMD RDNA 2+ (Linux) on compatible driver/FFmpeg | Driver/FFmpeg stack sensitive |
| `av1_amf` | AMD RDNA 2+ (Windows) | The AMD Windows path |
| `av1_videotoolbox` | Apple Silicon M3+ | On macOS hosts only |
| `libsvtav1` (CPU) | Any CPU | Alchemist's default CPU AV1 path — fastest of the CPU AV1 encoders |
| `libaom-av1` (CPU) | Any CPU | Higher quality per bit, much slower |

Hardware support is detected at startup using a short
FFmpeg probe. If you want to see what Alchemist found on
your host, open **Settings → Hardware → Probe Log** — it
records exactly which encoders passed and which failed, with
the FFmpeg stderr captured for failures. See
[Hardware Acceleration](/hardware) for the selection policy.

## Choosing AV1 as a target

AV1 makes sense when **storage savings matter more than
compatibility** and you're confident your playback devices
handle AV1 decode without kicking the server back into
live-transcode mode.

A short decision prompt:

- **Is your library mostly consumed on modern clients?**
  (Recent Chromium browsers, 2023+ TVs, newer Apple devices,
  newer Android phones). AV1 is a reasonable target.
- **Do you have older clients in the mix?** (2019-era
  TVs, Roku devices that lack AV1 decode, older browsers).
  HEVC is safer; AV1 will force Jellyfin / Plex to transcode
  back to something compatible, wiping out the savings.
- **Are you CPU-only?** Encoding AV1 in software is slow
  even with SVT-AV1. That's fine for a library you're
  transcoding overnight in an off-peak window; less fine if
  you want results in hours.

For more on codec tradeoffs see [Codecs](/codecs).

## When AV1 is not the right target

AV1 isn't automatically the best answer. Pick HEVC or H.264
when:

- Your clients don't reliably decode AV1 and you don't want
  live transcoding on the server.
- You're running CPU-only on older hardware — SVT-AV1 is
  competitive, but HEVC on x265 is still noticeably faster.
- Your library is already mostly HEVC 10-bit — Alchemist
  will skip those files by default
  (see [already_target_codec](/skip-decisions#already_target_codec)).
  Re-targeting to AV1 and running again is possible, but
  you're trading real CPU/GPU time for marginal savings;
  watch the planner output before committing.

## AV1 on Jellyfin

See [Alchemist for Jellyfin](/jellyfin) for the full
context. The short version: AV1 gives the biggest
space savings for libraries served to Jellyfin, but only if
your clients direct-play it. When a client can't decode AV1,
Jellyfin's own transcoder runs and the server does the work
anyway. Pre-transcoding to a codec that every client in your
house can direct-play is usually more useful than
pre-transcoding to the smallest codec on paper.

## Troubleshooting AV1

**AV1 encode requested but Alchemist fell back to CPU.**
Most likely the hardware AV1 probe failed — open
**Settings → Hardware → Probe Log**. For NVIDIA, check that
the card is RTX 30 or 40 series; older cards have NVENC but
not AV1 NVENC. For Intel, AV1 encode starts at 12th gen.
See [CPU fallback despite GPU](/troubleshooting#cpu-fallback-despite-gpu).

**VAAPI AV1 errors on Linux.**
`av1_vaapi` is sensitive to driver and FFmpeg versions. If
HEVC / H.264 VAAPI work but AV1 fails in the probe log, the
driver stack is the usual cause. See the
[AMD hardware guide](/hardware/amd) and
[Intel hardware guide](/hardware/intel).

**AV1 file plays, but the UI shows a live transcode on the
server.** The client can't direct-play the resulting AV1
stream. Confirm with Jellyfin / Plex playback info and
consider targeting HEVC for that library instead.

## See also

- [Hardware Acceleration](/hardware) — probe and selection
  policy.
- [Codecs](/codecs) — AV1 vs HEVC vs H.264 tradeoffs.
- [Planner](/planner) — how AV1's target multiplier affects
  skip decisions.
- [Alchemist for Jellyfin](/jellyfin) — client-compatibility
  context for picking a target codec.
