---
title: Library Doctor
description: Identifying corrupt, truncated, and unreadable media files in your library.
---

Library Doctor is a specialized diagnostic tool that scans your library for media files that are corrupt, truncated, or otherwise unreadable by the Alchemist analyzer. 

Run a scan manually from **Settings → Runtime → Library Doctor**.

## Core Checks

Library Doctor runs an intensive probe on every file in your watch directories to identify the following issues:

| Check | Technical Detection | Action Recommended |
|-------|-----------------|--------------------|
| **Probe Failure** | `ffprobe` returns a non-zero exit code or cannot parse headers. | Re-download or Re-rip. |
| **No Video Stream** | File container is valid but contains no detectable video tracks. | Verify source; delete if unintended. |
| **Zero Duration** | File metadata reports a duration of 0 seconds. | Check for interrupted transfers. |
| **Truncated File** | File size is significantly smaller than expected for the reported bitrate/duration. | Check filesystem integrity. |
| **Missing Metadata** | Missing critical codec data (e.g., pixel format, profile) needed for planning. | Possible unsupported codec variant. |

---

## Relationship to Jobs

Files that fail Library Doctor checks will also fail the **Analyzing** stage of a standard transcode job. 

- **Pre-emptive detection**: Running Library Doctor helps you clear "broken" files from your library before they enter the processing queue.
- **Reporting**: Issues identified by the Doctor appear in the **Health** tab of the dashboard, separate from active transcode jobs.

## Handling Results

Library Doctor is read-only; it will **never delete or modify** your files automatically. 

If a file is flagged, you should manually verify it using a media player. If the file is indeed unplayable, we recommend replacing it from the source. Flags can be cleared by deleting the file or moving it out of a watched directory.
