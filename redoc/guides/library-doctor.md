# Library Doctor

Scan your library for corrupt, unreadable, and broken media files.

Library Doctor scans your configured media directories for
files that are corrupt, truncated, or unreadable by FFprobe.
It surfaces issues before they surprise you during playback
or transcoding.

## What it checks

| Check | What it detects |
|-------|-----------------|
| **Probe failure** | Files FFprobe cannot read at all - fully corrupt or wrong format |
| **No video stream** | Files with no detectable video track |
| **Zero duration** | Files reporting 0 seconds of content |
| **Truncated file** | Files that appear to end prematurely |
| **Missing codec data** | Files missing required metadata Alchemist needs to plan a transcode |

## Running a scan

1. Go to **Settings -> Runtime**
2. Scroll to the **Library Doctor** section
3. Click **Run Scan**
4. Alchemist scans in the background - the dashboard
   shows progress

Results appear in the Library Doctor section after the scan
completes. Each issue shows the file path, the issue type,
and a brief explanation.

## What to do with results

Library Doctor reports issues but does not automatically
delete or repair files - that decision is yours.

**Common actions:**

- **Re-download the file** - if it came from a torrent or
  download that was interrupted
- **Re-rip the disc** - if it came from a Blu-ray or DVD
  that had errors
- **Delete it** - if it's a duplicate or you no longer need it
- **Ignore it** - if you know it's a weird format that
  FFprobe can't handle but your player can

## Scheduled scans

Library Doctor runs on-demand only. There is no automatic
scheduled scan - run it manually after large library changes
or when you suspect drive issues.

## Relationship to transcoding

Files that fail Library Doctor checks will typically also
fail the **Analyzing** stage of a transcode job and appear
as `Failed` in the Jobs tab with an error message. Running
Library Doctor beforehand helps you identify and fix these
files before queueing them.
