# Quick Start

The essentials for getting Alchemist processing your library.

This guide assumes you've completed the setup wizard. If not,
start with [First Run](first-run.md).

1. **Start the engine**

   The engine starts paused after setup. Click **Start** in
   the header bar to begin processing. The button shows the
   current state - Paused, Running, or Draining.

2. **Watch the queue**

   Go to **Jobs**. Files move through these states:
   `Queued -> Analyzing -> Encoding -> Completed`.

   Skipped files appear in the **Skipped** tab with a
   plain-English explanation of why Alchemist decided not to
   transcode them. This is normal - it means the files are
   already efficiently compressed.

3. **Check hardware detection**

   Go to **Settings -> Hardware** to confirm your GPU is
   detected. If you see `CPU (Software)` and you have a
   supported GPU, check the
   [GPU Passthrough guide](../guides/gpu-passthrough.md).

4. **See your savings**

   Once jobs start completing, the **Statistics** page shows
   total space recovered, average compression ratio, and a
   chart of activity over time.

## Useful controls

| What | Where | Notes |
|------|-------|-------|
| Pause encoding | Header -> Pause | Active jobs freeze mid-encode |
| Stop after current jobs | Header -> Stop | Drains the active jobs, starts no new ones |
| Cancel a job | Jobs -> ... -> Cancel | Stops the job immediately |
| Boost a job's priority | Jobs -> ... -> Boost | Moves it to the front of the queue |
| Trigger a manual scan | Settings -> Library -> Scan | Picks up newly added files |
| Change engine mode | Settings -> Runtime | Background / Balanced / Throughput |

> Tip: Use **Background** mode when you want Alchemist to run
> without impacting your server's responsiveness. Use
> **Throughput** when you want to clear a large backlog as
> fast as possible.
