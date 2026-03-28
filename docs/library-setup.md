---
title: Library Setup
description: Adding watch directories and organizing your media.
---

## Adding directories

Go to **Settings → Library** and add your media directory
paths. In Docker, use the container-side path (right side
of your volume mount).

Assign a **Profile** to each directory (Space Saver,
Balanced, Quality First, Streaming, or custom) to control
how files in that folder are transcoded.

## Watch folders

Enable **Watch Folders** to monitor directories in real
time. New files are queued automatically within a few
seconds.

Extra watch directories can be added in
**Settings → Watch Folders**.

## Recommended structure

```text
/media/
├── movies/       → Quality First profile
├── tv/           → Balanced profile
└── home-videos/  → Space Saver profile
```

## Triggering a manual scan

**Settings → Library → Trigger Scan** picks up newly added
files without waiting for the file watcher.

## Library Doctor

Run a health scan from **Settings → Runtime → Library Doctor**
to find corrupt or unreadable files before they fail as
encode jobs. See [Library Doctor](/library-doctor).
