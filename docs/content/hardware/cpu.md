---
title: CPU Encoding
description: Software encoding with SVT-AV1, x265, and x264.
---

CPU encoding is the fallback path when no supported GPU is
available, and it is also the right choice when you want the
best software quality and do not care about throughput.

## Encoders

| Codec | Encoder |
|------|---------|
| AV1 | SVT-AV1 |
| HEVC | x265 |
| H.264 | x264 |

## Presets

`cpu_preset` controls the software speed/quality tradeoff.

| Preset | Effect |
|-------|--------|
| `slow` | Best compression, lowest throughput |
| `medium` | Balanced default |
| `fast` | Lower CPU time, larger output |

## Thread configuration

`threads = 0` means automatic. Alchemist lets FFmpeg choose
the thread count per job. Set a manual value only if you are
tuning around a busy shared server or a known NUMA/core
layout.

## Performance expectations

These are reasonable 1080p expectations on modern CPUs:

| Codec | Preset | Expected speed |
|------|--------|----------------|
| AV1 | `medium` | ~0.5–1.5x realtime |
| HEVC | `medium` | ~1–3x realtime |
| H.264 | `medium` | ~3–8x realtime |

## When to use CPU

- No supported GPU is present
- Maximum software quality matters more than speed
- The batch is small enough that wall-clock time does not matter

## Thread allocation

| CPU cores | Suggested starting point |
|----------|--------------------------|
| 4 | 1 job, auto threads |
| 8 | 1 job, auto threads or 2 jobs with care |
| 16 | 2 jobs, auto threads |
| 32+ | 2-4 jobs, benchmark before going wider |

Use **Settings → Hardware** to allow or disable CPU encoding.
