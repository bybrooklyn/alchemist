# Web Interface

Overview of the Alchemist dashboard and what each section does.

The Alchemist web interface is a single-page app served from
the same binary as the backend. No separate web server needed.

## Dashboard

The first page you see after setup. Shows:

- **Engine state** - whether the engine is running, paused,
  or draining, with Start/Pause/Stop controls in the header
- **Stat row** - active jobs, completed, failed, total
  processed (compact, updated in real time via SSE)
- **Recent Activity** - the last five jobs with status dots
  and timestamps
- **Configuration summary** - library roots, notification
  targets, schedule windows
- **Quick Start tips** - only shown when there's something
  actionable (failures, no library configured)
- **Resource Monitor** - CPU usage, memory, and GPU metrics

## Jobs

The main job management interface.

**Tabs:** All / Active / Queued / Completed / Failed /
Skipped / Archived

Each row shows filename, status badge, progress bar (for
active jobs), and last updated time. Click any row to open
the detail panel showing:

- Input metadata (codec, resolution, bitrate, duration)
- Output stats (result size, compression ratio, VMAF score)
- Skip or failure reason with plain-English explanation
- Full FFmpeg log for the job

**Bulk actions:** Select multiple jobs with the checkboxes
to restart, cancel, or delete in one operation.

## Logs

Real-time log viewer fed by SSE. Entries are grouped by job
- click a job header to expand its log lines. System logs
appear at the top. Filterable by level (info/warn/error)
with text search.

## Statistics

Space savings over time as an area chart, per-codec
breakdown, and aggregate totals. Data comes from completed
jobs - the chart fills in as jobs complete.

## Settings

Ten tabs covering every configurable aspect of Alchemist:

| Tab | What it controls |
|-----|-----------------|
| Library | Watch folders, scan trigger |
| Watch Folders | Extra directories to monitor |
| Transcoding | Codec, quality, thresholds, stream rules |
| Hardware | GPU vendor, device path, fallback behavior |
| File Settings | Output extension, suffix, output root, replace strategy |
| Quality | VMAF scoring, minimum score, revert on failure |
| Notifications | Discord, Gotify, webhook targets and events |
| Schedule | Time windows when encoding is allowed |
| Runtime | Engine mode (Background/Balanced/Throughput), Library Doctor |
| Appearance | Color theme selection (35+ themes) |

## Appearance

35+ color themes selectable from the Appearance settings.
Themes range from warm darks (Helios Orange, Ember, Sunset)
to cool darks (Ocean, Glacier, Nocturne) to light modes
(Ivory, Cloud, Linen). The selected theme persists across
sessions.

## Engine control header

Visible on every page. Shows the current engine state and
provides Start, Pause, and Stop buttons. Stop puts the engine
into **drain** mode - active jobs finish normally, no new
jobs start. Cancel the drain to resume normal operation.
