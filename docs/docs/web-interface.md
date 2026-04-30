---
title: Web Interface
description: What each section of the Alchemist dashboard does.
---

Served by the same binary as the backend. Default:
`http://localhost:3000`.

## Header bar

Visible on every page. Shows engine state and provides
**Start** and **Stop** controls plus About and Logout.

- **Start** — begins processing
- **Stop** — drain mode: active jobs finish, no new jobs start
- **About** — version info, environment info, and update-check status

## Dashboard

- Engine state and stat row (active, completed, failed, total)
- Recent Activity — last five jobs with status and timestamps
- Resource Monitor — CPU, memory, GPU (updated via SSE)

## Jobs

Tabs: Active / Queued / Completed / Failed / Cancelled /
Skipped / Archived

Click any job to open the detail panel:
- Input metadata (codec, resolution, bitrate, duration, HDR)
- Output stats (size, compression ratio, speed, VMAF)
- Skip or failure reason in plain English
- Per-file attempt history for retries and reruns
- Queue position and blocked reason for queued jobs
- Full FFmpeg log

Bulk actions via checkboxes: restart, cancel, delete. Terminal
tabs can be cleared from the active table without deleting the
underlying history.

## Logs

Real-time log viewer (SSE). Entries grouped by job — click
a header to expand. System logs appear at the top.
Filterable by level, searchable.

## Statistics

Space savings area chart, per-codec breakdown, aggregate
totals. Fills in as jobs complete.

## Intelligence

- Duplicate groups by basename
- Remux-only opportunities
- Wasteful audio layout recommendations
- Commentary / descriptive-track cleanup candidates

## Convert

Experimental single-file utility:

- Upload a file (bounded by `conversion_upload_limit_gb`, default 8 GiB)
- Probe streams and metadata
- Configure transcode or remux settings
- Preview the generated FFmpeg command plus source/output summary and estimated savings
- Queue the job and download the result when complete

Uploads and generated outputs are removed automatically by a cleanup sweep that runs on every upload. The retention window after a successful download is governed by `conversion_download_retention_hours` (default 1 hour).

Convert uses the same analyzer, planner, queue, and executor
as library jobs. Treat it as a one-off utility, not a second
core workflow.

## Settings tabs

| Tab | Controls |
|-----|---------|
| Library | Watch folders, scan trigger |
| Watch Folders | Extra monitored directories |
| Transcoding | Codec, quality, thresholds, stream rules |
| Hardware | GPU vendor, device path, fallback, probe log, cached detection state |
| File Settings | Output extension, suffix, output root, replace strategy |
| Quality | VMAF scoring, minimum score, revert on failure |
| Notifications | Discord webhook, Discord bot, Gotify, ntfy, Telegram, email, webhook targets, quiet hours, daily summary time |
| API Tokens | Named bearer tokens with `read_only`, `arr_webhook`, and `full_access` classes |
| Schedule | Time windows |
| Runtime | Engine mode, concurrent jobs override, Library Doctor, database backup |
| System | Monitoring poll interval, manual conversion upload limit and post-download retention, telemetry toggle, watch-folder switch, metrics switch |
| Appearance | Color theme (35+ themes) |
