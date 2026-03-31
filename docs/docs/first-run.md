---
title: First Run & Setup Wizard
description: Getting through the setup wizard and starting your first scan.
---

When you first open Alchemist at `http://localhost:3000`
the setup wizard runs automatically. It takes about two
minutes.

## Wizard steps

**1. Admin account** — Set a username and password. Telemetry
is opt-in and off by default.

**2. Library selection** — Add the server folders Alchemist
should scan. In Docker these are the container-side paths
(right side of your volume mount). If you mounted
`/mnt/media` as `/media`, enter `/media` here.

Alchemist auto-discovers likely media roots and shows them
as suggestions. Add any path manually or browse the server
filesystem.

**3. Processing settings** — Target codec (AV1 default),
quality profile, output rules. Defaults are sensible.
Everything is changeable later.

**4. Hardware, notifications & schedule** — GPU is detected
automatically. You can pin a vendor, configure Discord or
webhook notifications, and restrict encoding to schedule
windows.

**5. Review & complete** — Summary of all choices. Click
**Complete Setup** to write the config and start the first
library scan.

## After setup

The engine starts **paused** after setup. Click **Start**
in the header bar to begin processing.

The initial scan runs automatically in the background. Watch
files enter the queue in the **Jobs** tab.

## Resetting

To fully reset and re-run the wizard:

```bash
just db-reset-all
```
