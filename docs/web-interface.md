---
title: Web Interface
description: What each section of the Alchemist dashboard does.
---

Served by the same binary as the backend. Default:
`http://localhost:3000`.

## Header bar

Visible on every page. Shows engine state and provides
**Start**, **Pause**, and **Stop** controls.

- **Start** — begins processing
- **Pause** — freezes active jobs mid-encode, stops new jobs
- **Stop** — drain mode: active jobs finish, no new jobs start

## Dashboard

- Engine state and stat row (active, completed, failed, total)
- Recent Activity — last five jobs with status and timestamps
- Resource Monitor — CPU, memory, GPU (updated via SSE)

## Jobs

Tabs: Active / Queued / Completed / Failed / Skipped / Archived

Click any job to open the detail panel:
- Input metadata (codec, resolution, bitrate, duration, HDR)
- Output stats (size, compression ratio, speed, VMAF)
- Skip or failure reason in plain English
- Full FFmpeg log

Bulk actions via checkboxes: restart, cancel, delete.

## Logs

Real-time log viewer (SSE). Entries grouped by job — click
a header to expand. System logs appear at the top.
Filterable by level, searchable.

## Statistics

Space savings area chart, per-codec breakdown, aggregate
totals. Fills in as jobs complete.

## Settings tabs

| Tab | Controls |
|-----|---------|
| Library | Watch folders, scan trigger |
| Watch Folders | Extra monitored directories |
| Transcoding | Codec, quality, thresholds, stream rules |
| Hardware | GPU vendor, device path, fallback |
| File Settings | Output extension, suffix, output root, replace strategy |
| Quality | VMAF scoring, minimum score, revert on failure |
| Notifications | Discord, Gotify, webhook targets |
| Schedule | Time windows |
| Runtime | Engine mode, concurrent jobs override, Library Doctor |
| Appearance | Color theme (35+ themes) |
