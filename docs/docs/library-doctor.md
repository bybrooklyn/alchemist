---
title: Library Doctor
description: Scan for corrupt, truncated, and unreadable media files.
---

Library Doctor scans your configured directories for files
that are corrupt, truncated, or unreadable by FFprobe.

Run from **Settings → Runtime → Library Doctor → Run Scan**.

## What it checks

| Check | What it detects |
|-------|-----------------|
| Probe failure | Files FFprobe cannot read at all |
| No video stream | Files with no detectable video track |
| Zero duration | Files reporting 0 seconds of content |
| Truncated file | Files that appear to end prematurely |
| Missing codec data | Files missing metadata needed to plan a transcode |

## What to do with results

Library Doctor reports issues — it does not repair or delete
files automatically.

- **Re-download** — interrupted download
- **Re-rip** — disc read errors
- **Delete** — duplicate or unrecoverable
- **Ignore** — player handles it despite FFprobe failing

Files that fail Library Doctor also fail the Analyzing
stage of a transcode job and appear as Failed in Jobs.
