---
title: Quick Start
description: The essentials for getting Alchemist processing a media library — start the engine, monitor jobs, confirm hardware selection, and read skip reasons.
keywords:
  - alchemist quick start
  - automatic transcoding
---

Assumes you've completed the setup wizard. If not, see
[First Run](/first-run).

## Start the engine

The engine starts paused after setup. Click **Start** in
the header bar. If the header shows **Stopping**, the engine
is draining; wait for active jobs to finish or use the
Runtime controls to stop drain mode.

## Watch the queue

Go to **Jobs**. Files move through:
`Queued → Analyzing → Encoding → Completed`

Skipped files appear in the **Skipped** tab with a
plain-English reason. A high skip rate is normal — it means
files are already efficiently compressed. See
[Skip Decisions](/skip-decisions).

## Check hardware detection

Go to **Settings → Hardware**. Confirm your GPU is the
active backend. If you see `CPU (Software)` with a supported
GPU, open the probe log first, then see
[GPU Passthrough](/gpu-passthrough).

On repeat boots, a valid cached hardware result may appear
immediately while the full probe refreshes in the background.

## See your savings

Once jobs complete, **Statistics** shows total space
recovered, compression ratios, and a savings chart.

## Key controls

| Action | Where |
|--------|-------|
| Pause new job claims | Settings → Runtime → Pause |
| Drain (finish active, stop new) | Header → Stop |
| Cancel a job | Jobs → ⋯ → Cancel |
| Boost priority | Jobs → ⋯ → Boost |
| Trigger manual scan | Settings → Library → Scan |
| Change engine mode | Settings → Runtime |

## See also

- [Alchemist for Jellyfin](/jellyfin) — pre-transcoding a
  Jellyfin library.
- [Profiles](/profiles) — per-library targets.
- [Scheduling](/scheduling) — restrict encoding to off-peak
  windows.
- [Troubleshooting](/troubleshooting) — queue stuck, GPU
  not detected, unexpected skips.
