# First Run & Setup Wizard

Getting through the setup wizard and starting your first scan.

When you first open Alchemist at
[http://localhost:3000](http://localhost:3000), the setup
wizard runs automatically. It takes about two minutes.

## The wizard steps

1. **Create your admin account**

   Set a username and password. These are the credentials
   you'll use to log in to the web interface. Telemetry is
   opt-in and off by default.

2. **Library selection**

   Add the server folders Alchemist should scan. If you're
   running in Docker, these are the paths as the container
   sees them - so if you mounted `/path/to/media` as
   `/media`, you enter `/media` here.

   Alchemist auto-discovers likely media folders and shows
   them as suggestions. You can add any path manually or
   use the server browser to navigate the filesystem.

3. **Processing settings**

   Choose your target codec (AV1 is the default - best
   compression, growing hardware support), quality profile,
   and output rules. The defaults are sensible for most
   libraries. You can change everything later.

4. **Hardware, notifications & schedule**

   Alchemist detects your GPU automatically. You can pin a
   specific vendor, set CPU fallback behavior, configure
   Discord or webhook notifications, and define schedule
   windows so encoding only runs during off-peak hours.

5. **Review & complete**

   A summary of all your choices. Click **Complete Setup**
   to write the config and start the first library scan.

## After setup

The engine starts paused after setup completes. You'll see
an "Engine Paused" banner on the dashboard. Click **Start**
in the header to begin processing.

The initial scan runs automatically in the background.
Depending on your library size it may take a few minutes.
Check the **Jobs** tab to watch files enter the queue.

> Note: The engine starts paused intentionally - this gives
> you a chance to review what was queued before any encoding
> begins.

## Changing settings later

Everything configured in the wizard is accessible in
**Settings** at any time. To fully reset and re-run the
wizard:

```bash
just db-reset-all
```
