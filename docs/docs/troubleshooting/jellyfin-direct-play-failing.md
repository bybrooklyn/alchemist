---
title: Jellyfin Direct Play Failing — After Pre-Transcoding with Alchemist
description: Jellyfin still transcodes on the fly after Alchemist processed the file. How to diagnose direct-play failures — codec, container, audio, subtitles, bitrate, and client profile.
keywords:
  - jellyfin direct play not working
  - jellyfin still transcoding
  - jellyfin av1 not direct playing
  - jellyfin hevc transcoding
slug: /troubleshooting/jellyfin-direct-play-failing
---

Alchemist doesn't control Jellyfin's playback decision —
it only prepares files ahead of time. When a client still
forces Jellyfin to live-transcode, Jellyfin itself has the
answer: open the **Playback Info** panel on the client and
read the reason. Every step below is about narrowing down
what that panel tells you.

Direct play requires **every** stream the server sends to be
something the client can handle natively — video codec,
container, audio codec, and sometimes subtitle format all
have to line up. Pre-transcoding fixes the video side but
doesn't touch the audio, subtitle, or container story
unless you've configured that explicitly.

## 1. Ask Jellyfin why

Start here every time:

1. Start playback in a Jellyfin client.
2. Open the Playback Info overlay (key binding varies by
   client; web client: `Ctrl+Alt+I` or the info button).
3. Read the `Reason` field — Jellyfin names the exact
   stream that forced transcoding.

Typical reasons and what they mean:

| Reason Jellyfin gives | What actually failed |
|---|---|
| `VideoCodecNotSupported` | Client can't decode the video codec in this file |
| `ContainerNotSupported` | Client can't handle this container even though the codecs are fine |
| `AudioCodecNotSupported` | Audio codec (often TrueHD, DTS-HD, EAC3 Atmos) isn't decodable by the client |
| `SubtitleCodecNotSupported` | Embedded subtitle format is burn-in only on this client |
| `VideoBitrateNotSupported` | Client profile has a bitrate cap below the file |

## 2. Video codec mismatch

This is the clean case for pre-transcoding. If the Playback
Info says `VideoCodecNotSupported`, the codec Alchemist
produced isn't playable on that client.

Common patterns:

- **Targeted AV1, client can't decode AV1.** Older TVs,
  Rokus, many set-top boxes, and some browsers on older OS
  versions. Retarget that library to HEVC and let Alchemist
  transcode again. See [AV1 transcoding](/av1) and
  [Codecs](/codecs).
- **Targeted HEVC, client is stuck on H.264.** Older
  browsers or very old devices. Retarget to H.264 for that
  library.
- **Targeted HEVC Main10, client only decodes Main.**
  10-bit HEVC isn't universally supported. Switch the
  profile to 8-bit HEVC (or H.264) for the affected
  clients.

## 3. Audio codec mismatch

Alchemist's [stream rules](/stream-rules) decide what
happens to audio during transcoding. By default audio is
passed through, which means if the source had a TrueHD
Atmos track and the client doesn't decode TrueHD, Jellyfin
still has to transcode.

If Playback Info says `AudioCodecNotSupported`:

- Add a stream rule that re-encodes incompatible audio to
  AAC or EAC3, or transcodes high-bitrate audio down to a
  client-friendly format. See
  [Stream Rules](/stream-rules).
- Consider adding a compatibility audio track instead of
  replacing — Alchemist can output the original plus a
  client-safe track.

## 4. Container mismatch

If `ContainerNotSupported` is the reason:

- Check the output container setting in the profile you ran.
- If the codec is right but the container is wrong (e.g.
  HEVC in `.mp4` where the client wants `.mkv`), Alchemist
  will _remux_ rather than re-encode on the next pass —
  lossless and fast. See
  [Planner → Remux path](/planner#remux-path).

## 5. Subtitle format

Many clients direct-play the video and audio but force a
server transcode because an embedded PGS or VOBSUB subtitle
has to be burned in. The Playback Info panel names this
explicitly as `SubtitleCodecNotSupported`.

Alchemist doesn't manage subtitles beyond the stream-rule
settings you give it. If subtitles are the cause:

- Extract PGS subtitles to external `.sup` or `.srt`
  sidecars (outside Alchemist).
- Or accept the subtitle-triggered transcode — it's
  lighter than a full video transcode.

## 6. Bitrate caps

If the Playback Info says `VideoBitrateNotSupported`, the
Jellyfin client profile has a bitrate ceiling below the
file's actual bitrate. That's a Jellyfin client-settings
issue, not an Alchemist one — raise the bitrate cap on the
client, or target a lower-bitrate encode in Alchemist's
profile.

## 7. Verify Alchemist actually touched the file

A common false diagnosis: the file wasn't transcoded at all
because the [planner](/planner) correctly skipped it. Check
the **Skipped** tab in Alchemist. If the file shows up with
a reason like `already_target_codec` or `bpp_below_threshold`,
Alchemist decided no re-encode was worthwhile — the original
is what Jellyfin is playing. See
[Skip Decisions](/skip-decisions).

## Related

- [Alchemist for Jellyfin](/jellyfin) — overall
  pre-transcoding approach.
- [Codecs](/codecs) — AV1 / HEVC / H.264 client
  compatibility notes.
- [Stream Rules](/stream-rules) — per-stream audio and
  subtitle handling.
- [Planner](/planner) — the skip / remux / transcode
  decision.
- [Troubleshooting overview](/troubleshooting).
