---
title: Scheduling
description: Restrict encoding to specific time windows.
---

The scheduler restricts when the engine may start new jobs.

## Creating a window

Go to **Settings → Schedule** and add a window with:

- **Start time** — when encoding may begin (e.g. `22:00`)
- **End time** — when it must stop (e.g. `07:00`)
- **Days of week** — which days apply

Multiple windows are supported. Windows spanning midnight
are handled correctly.

## At the window boundary

When a window ends mid-encode, the engine pauses. The
in-progress job is suspended and resumes at the next window
start. No data is lost.

## Manual override

With a schedule active, you can still force-start from the
header bar. The override lasts until the next boundary.

## No schedule

If no windows are configured, the engine runs whenever it
is in Running state.
