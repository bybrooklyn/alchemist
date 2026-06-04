# Alchemist Jellyfin Plugin

This plugin listens for Jellyfin library item additions/updates and forwards
eligible local media file paths to Alchemist. It can also listen to Alchemist
job events and ask Jellyfin to refresh the containing directory after a job
completes.

Target Jellyfin version: `10.11.10`.
Plugin catalog version: `0.3.4.0`.
Canonical plugin/package ID: `dev.bybrooklyn.alchemist`.

Jellyfin requires plugin IDs to be GUIDs, so the canonical ID is represented by
the deterministic UUID `c3637bc0-04ad-58b6-b11c-e840af0b1f6e`.

## Build

```bash
just jellyfin-check
```

## Package

Create the release zip plus MD5 and SHA-256 checksum assets with:

```bash
just jellyfin-package
```

Release candidates attach a manually installable
`dev.bybrooklyn.alchemist_0.3.4.0.zip` without changing the stable catalog. Stable
releases update:

```text
https://raw.githubusercontent.com/bybrooklyn/alchemist/jellyfin-plugin-repo/manifest.json
```

Add that URL under **Dashboard → Plugins → Repositories** to install stable
plugin releases from Jellyfin's catalog. For a manual install, extract the
release zip into a dedicated Jellyfin plugin folder and restart Jellyfin.

## Configure

Install the built plugin into Jellyfin, open the Alchemist plugin settings, and set:

- Alchemist URL, for example `http://localhost:3000`
- A Jellyfin-scoped Alchemist API token
- Whether auto-enqueue is enabled for Jellyfin add/update events
- Whether the Alchemist event listener is enabled
- Whether completed Alchemist jobs should refresh Jellyfin
- Whether dry-run mode is enabled
- Optional path translations, one per line: `/jellyfin/path=/alchemist/path`
- Optional reverse path translations, one per line: `/alchemist/path=/jellyfin/path`

Dry-run mode is enabled by default so library events can be observed before
Alchemist receives enqueue requests. Reverse path translations can be left empty
when they are the exact inverse of the forward translations.

## Token scope

Create the token in Alchemist with `access_level: "jellyfin"`. The plugin uses:

- `GET /api/v1/system/info` for connection checks
- `GET /api/v1/events` for completed-job notifications
- `GET /api/v1/jobs/:id/details` to resolve completed job paths
- `POST /api/v1/jobs/enqueue` for Jellyfin add/update hooks

## Refresh behavior

When Alchemist emits a completed job status event, the plugin fetches job details,
prefers `output_path` over `input_path`, translates the path back to a
Jellyfin-visible path, and reports the containing directory to Jellyfin's library
monitor. That keeps refreshes narrow instead of queueing a whole-library scan.

## Troubleshooting

- Use **Test Connection** to verify URL and token access to Alchemist.
- Use **Test Event Access** to verify the token can open the SSE stream.
- Check the runtime status line for the last event-listener state and refresh result.
- If enqueue works but refresh does not, check reverse path translations first.
- If nothing is sent to Alchemist, disable dry-run mode after confirming the logged paths.
